//! Mutable battle state.
//!
//! Everything needed to resume a battle lives in [`BattleState`], including the
//! RNG. Serializing this struct is a complete save; there is no hidden state
//! elsewhere. Keep it that way.

use serde::{Deserialize, Serialize};

use crate::data::{
    AiProfile, CombatantDef, DataError, Database, Element, Id, Resistances, Stats, StatusKind, Team,
};
use crate::rng::Rng;

/// Tempo required to act. Acting subtracts the action's cost, so a 1500-cost
/// skill pushes the actor further back in the order than a 600-cost one.
pub const TEMPO_THRESHOLD: u32 = 1000;

/// SP returned to a combatant at the end of each of its own turns.
///
/// A flat amount rather than a share of the pool, and that asymmetry is
/// deliberate: recovery is the fighter's own effort, not a dividend on power.
/// Four points is 5% of an 80 SP party member's reserve and 2% of a 200 SP
/// boss's, so the longer a fight runs the more it favours the side with the
/// smaller pool -- which is the side that had already spent everything.
pub const SP_REGEN_PER_TURN: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub kind: StatusKind,
    pub potency: i32,
    pub remaining: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Combatant {
    pub id: Id,
    pub name: String,
    pub team: Team,
    pub base: Stats,
    pub max_hp: i32,
    pub max_sp: i32,
    pub hp: i32,
    pub sp: i32,
    pub stand: Option<Id>,
    pub skills: Vec<Id>,
    pub statuses: Vec<Status>,
    pub tempo: u32,
    pub tempo_lock: u32,
    pub ai: AiProfile,
    /// Copied from the definition at instantiation. Older saves that predate
    /// resistances deserialize into an empty table, which reads as neutral.
    #[serde(default)]
    pub resist: Resistances,
}

impl Combatant {
    pub fn alive(&self) -> bool {
        self.hp > 0
    }

    pub fn has(&self, kind: StatusKind) -> bool {
        self.statuses.iter().any(|status| status.kind == kind)
    }

    pub fn status(&self, kind: StatusKind) -> Option<&Status> {
        self.statuses.iter().find(|status| status.kind == kind)
    }

    /// Resistance to `element` in percent: positive removes damage, negative
    /// adds it, zero when the table says nothing.
    ///
    /// An accessor rather than a public map read, matching [`Combatant::atk`]
    /// and friends: when a status that alters resistance is added, it is added
    /// here and every call site inherits it.
    pub fn resistance(&self, element: Element) -> i32 {
        self.resist.get(&element).copied().unwrap_or(0)
    }

    /// Returns SP to this combatant and reports how much was actually recovered,
    /// so a caller can log or account for it rather than inferring it.
    ///
    /// Never exceeds `max_sp`, and the dead recover nothing: a downed combatant
    /// that quietly refilled would come back with a full kit if a revive is ever
    /// added.
    pub fn recover_sp(&mut self, amount: i32) -> i32 {
        if !self.alive() || amount <= 0 {
            return 0;
        }
        let recovered = amount.min(self.max_sp - self.sp).max(0);
        self.sp += recovered;
        recovered
    }

    /// Net percentage modifier from opposing buff/debuff pairs. Stacking is
    /// additive rather than multiplicative so that the numbers stay legible to
    /// a designer reading the JSON.
    fn modifier(&self, up: StatusKind, down: StatusKind) -> i32 {
        self.statuses
            .iter()
            .map(|status| {
                if status.kind == up {
                    status.potency
                } else if status.kind == down {
                    -status.potency
                } else {
                    0
                }
            })
            .sum()
    }

    fn scaled(&self, base: i32, up: StatusKind, down: StatusKind) -> i32 {
        // Floor the multiplier at 10% so an unbounded debuff stack can never
        // zero out a stat and stall the fight.
        let percent = (100 + self.modifier(up, down)).max(10);
        (base * percent / 100).max(1)
    }

    pub fn atk(&self) -> i32 {
        self.scaled(self.base.atk, StatusKind::AtkUp, StatusKind::AtkDown)
    }

    pub fn def(&self) -> i32 {
        self.scaled(self.base.def, StatusKind::DefUp, StatusKind::DefDown)
    }

    pub fn spd(&self) -> i32 {
        self.scaled(self.base.spd, StatusKind::SpdUp, StatusKind::SpdDown)
    }

    /// Will drives crit rate and status resistance. Intentionally not buffable
    /// yet; add a status pair here when a design need actually appears.
    pub fn will(&self) -> i32 {
        self.base.will
    }

    /// Inserts or refreshes a status. Reapplying keeps the stronger potency and
    /// the longer remaining duration instead of stacking entries, which keeps
    /// spam-locking a target from scaling without bound.
    pub fn apply_status(&mut self, kind: StatusKind, potency: i32, duration: u8) {
        if let Some(existing) = self.statuses.iter_mut().find(|status| status.kind == kind) {
            existing.potency = existing.potency.max(potency);
            existing.remaining = existing.remaining.max(duration);
        } else {
            self.statuses.push(Status {
                kind,
                potency,
                remaining: duration,
            });
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleState {
    pub tick: u64,
    pub turn: u64,
    pub combatants: Vec<Combatant>,
    pub rng: Rng,
}

impl BattleState {
    /// Instantiates runtime combatants from content definitions.
    ///
    /// Team membership comes from which list an id appears in, not from the
    /// definition's own `team` field. That lets the same definition be reused as
    /// an ally or an enemy without duplicating content.
    pub fn build(db: &Database, party: &[Id], foes: &[Id], seed: u64) -> Result<Self, DataError> {
        let mut issues: Vec<String> = Vec::new();
        let mut combatants: Vec<Combatant> = Vec::new();

        for (ids, team) in [(party, Team::Party), (foes, Team::Foe)] {
            for id in ids {
                match db.combatant(id) {
                    Some(def) => combatants.push(instantiate(db, def, team)),
                    None => issues.push(format!("unknown combatant id '{id}'")),
                }
            }
        }

        if party.is_empty() {
            issues.push("party is empty".to_string());
        }
        if foes.is_empty() {
            issues.push("foe side is empty".to_string());
        }
        if !issues.is_empty() {
            return Err(DataError::Invalid(issues));
        }

        Ok(BattleState {
            tick: 0,
            turn: 0,
            combatants,
            rng: Rng::new(seed),
        })
    }

    pub fn alive_on(&self, team: Team) -> Vec<usize> {
        self.combatants
            .iter()
            .enumerate()
            .filter(|(_, c)| c.team == team && c.alive())
            .map(|(index, _)| index)
            .collect()
    }

    pub fn team_alive(&self, team: Team) -> bool {
        self.combatants.iter().any(|c| c.team == team && c.alive())
    }

    pub fn get(&self, index: usize) -> Option<&Combatant> {
        self.combatants.get(index)
    }
}

fn instantiate(db: &Database, def: &CombatantDef, team: Team) -> Combatant {
    let bonus = def
        .stand
        .as_ref()
        .and_then(|id| db.stand(id))
        .map(|stand| stand.bonus)
        .unwrap_or_default();
    let base = def.stats.plus(bonus);
    Combatant {
        id: def.id.clone(),
        name: def.name.clone(),
        team,
        max_hp: base.hp.max(1),
        max_sp: base.sp.max(0),
        hp: base.hp.max(1),
        sp: base.sp.max(0),
        base,
        stand: def.stand.clone(),
        skills: db.skills_for(def),
        statuses: Vec::new(),
        // Staggered opening tempo would be a fairness lever; starting everyone
        // at zero keeps the first turn order a pure function of speed.
        tempo: 0,
        tempo_lock: 0,
        ai: def.ai,
        resist: def.resist.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fighter(atk: i32, def: i32, spd: i32) -> Combatant {
        let base = Stats {
            hp: 100,
            sp: 40,
            atk,
            def,
            spd,
            will: 10,
        };
        Combatant {
            id: "pc.test".to_string(),
            name: "Test".to_string(),
            team: Team::Party,
            base,
            max_hp: 100,
            max_sp: 40,
            hp: 100,
            sp: 20,
            stand: None,
            skills: Vec::new(),
            statuses: Vec::new(),
            tempo: 0,
            tempo_lock: 0,
            ai: AiProfile::Aggressive,
            resist: Resistances::default(),
        }
    }

    #[test]
    fn a_debuff_lowers_the_stat_it_names_and_no_other() {
        let mut fighter = fighter(100, 60, 20);
        fighter.apply_status(StatusKind::AtkDown, 30, 3);
        assert_eq!(fighter.atk(), 70);
        assert_eq!(fighter.def(), 60);
        assert_eq!(fighter.spd(), 20);

        fighter.apply_status(StatusKind::SpdDown, 50, 3);
        assert_eq!(fighter.spd(), 10);
        assert_eq!(fighter.atk(), 70);
    }

    #[test]
    fn a_buff_and_a_debuff_of_one_stat_add_up() {
        let mut fighter = fighter(100, 60, 20);
        fighter.apply_status(StatusKind::AtkUp, 50, 3);
        fighter.apply_status(StatusKind::AtkDown, 20, 3);
        assert_eq!(fighter.atk(), 130);
    }

    #[test]
    fn reapplying_a_status_refreshes_that_status_and_no_other() {
        let mut fighter = fighter(100, 60, 20);
        fighter.apply_status(StatusKind::AtkUp, 20, 2);
        fighter.apply_status(StatusKind::DefUp, 50, 4);
        fighter.apply_status(StatusKind::AtkUp, 10, 5);
        assert_eq!(
            fighter.statuses,
            vec![
                Status {
                    kind: StatusKind::AtkUp,
                    potency: 20,
                    remaining: 5,
                },
                Status {
                    kind: StatusKind::DefUp,
                    potency: 50,
                    remaining: 4,
                },
            ]
        );
    }

    #[test]
    fn has_reports_exactly_the_statuses_present() {
        let mut fighter = fighter(100, 60, 20);
        assert!(!fighter.has(StatusKind::Stun));
        fighter.apply_status(StatusKind::Stun, 0, 1);
        assert!(fighter.has(StatusKind::Stun));
        assert!(!fighter.has(StatusKind::Bleed));
    }

    #[test]
    fn the_downed_recover_no_sp_and_the_living_recover_up_to_their_pool() {
        let mut fighter = fighter(100, 60, 20);
        fighter.hp = 0;
        fighter.sp = 5;
        assert_eq!(fighter.recover_sp(10), 0);
        assert_eq!(fighter.sp, 5);

        fighter.hp = 1;
        assert_eq!(fighter.recover_sp(10), 10);
        assert_eq!(fighter.recover_sp(100), 25);
        assert_eq!(fighter.sp, 40);
        assert_eq!(fighter.recover_sp(0), 0);
    }
}
