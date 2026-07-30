//! Turns a [`Command`] into [`Event`]s.
//!
//! Contract, in order:
//!
//! 1. Validate everything (actor alive, skill known, SP available, target legal)
//!    **before** mutating anything. A rejected command must leave the state
//!    untouched and the turn unspent, so the UI can explain and re-prompt.
//! 2. Mutate, emitting one event per observable fact.
//! 3. Return the tempo cost actually consumed.
//!
//! There is no `match` on a specific skill id anywhere below. Basic attack and
//! guard are ordinary data entries; the only thing the code knows is their id.

use crate::command::Command;
use crate::data::{
    Database, Effect, Element, SkillDef, StatusKind, TargetKind, MAX_RESISTANCE, MIN_RESISTANCE,
};
use crate::event::Event;
use crate::state::{BattleState, TEMPO_THRESHOLD};

/// Content ids the engine looks up by name. These two are the only ids the code
/// knows about, and both have code fallbacks so a missing entry degrades into a
/// sane default instead of a panic.
pub const BASIC_ATTACK_ID: &str = "skill.strike";
pub const GUARD_ID: &str = "skill.guard_stance";

/// Used only if `data/skills.json` is missing the basic attack. The validator
/// should catch that first; this exists so that a content mistake cannot make
/// the game unplayable.
fn fallback_attack() -> SkillDef {
    SkillDef {
        id: BASIC_ATTACK_ID.to_string(),
        name: "Strike".to_string(),
        description: "Fallback basic attack.".to_string(),
        sp_cost: 0,
        target: TargetKind::OneEnemy,
        accuracy: 95,
        tempo_cost: TEMPO_THRESHOLD,
        effects: vec![Effect::Damage {
            power: 100,
            element: Element::Physical,
            variance: 10,
        }],
    }
}

fn fallback_guard() -> SkillDef {
    SkillDef {
        id: GUARD_ID.to_string(),
        name: "Guard".to_string(),
        description: "Fallback defensive stance.".to_string(),
        sp_cost: 0,
        target: TargetKind::SelfOnly,
        accuracy: 100,
        tempo_cost: TEMPO_THRESHOLD * 3 / 5,
        effects: vec![Effect::Status {
            status: StatusKind::DefUp,
            potency: 50,
            duration: 2,
            chance: 100,
        }],
    }
}

/// Resolves one command and returns the tempo cost to charge the actor.
pub fn resolve_command(
    db: &Database,
    st: &mut BattleState,
    actor: usize,
    command: &Command,
    log: &mut Vec<Event>,
) -> Result<u32, String> {
    match st.get(actor) {
        Some(combatant) if combatant.alive() => {}
        Some(combatant) => return Err(format!("{} is down and cannot act", combatant.name)),
        None => return Err(format!("no combatant at index {actor}")),
    }

    // Waiting is deliberately free of validation: it is the escape hatch that
    // guarantees an actor always has a legal move, so the scheduler cannot
    // deadlock.
    let (skill, requested): (SkillDef, Option<usize>) = match command {
        Command::Wait => {
            log.push(Event::Waited { actor });
            return Ok(TEMPO_THRESHOLD / 2);
        }
        Command::Attack { target } => (
            db.skill(BASIC_ATTACK_ID)
                .cloned()
                .unwrap_or_else(fallback_attack),
            Some(*target),
        ),
        Command::Guard => (
            db.skill(GUARD_ID).cloned().unwrap_or_else(fallback_guard),
            None,
        ),
        Command::Skill { skill, target } => {
            let def = db
                .skill(skill)
                .ok_or_else(|| format!("unknown skill '{skill}'"))?;
            if !st.combatants[actor].skills.iter().any(|id| id == skill) {
                return Err(format!(
                    "{} does not know '{skill}'",
                    st.combatants[actor].name
                ));
            }
            (def.clone(), Some(*target))
        }
    };

    if st.combatants[actor].sp < skill.sp_cost {
        return Err(format!(
            "not enough SP for '{}': {} required, {} available",
            skill.name, skill.sp_cost, st.combatants[actor].sp
        ));
    }

    // Target resolution is the last thing that can fail. After this point the
    // action is committed.
    let targets = resolve_targets(st, actor, skill.target, requested)?;

    log.push(Event::ActionUsed {
        actor,
        skill: skill.id.clone(),
        name: skill.name.clone(),
    });

    if skill.sp_cost > 0 {
        st.combatants[actor].sp -= skill.sp_cost;
        log.push(Event::SpConsumed {
            actor,
            amount: skill.sp_cost,
        });
    }

    for target in targets {
        // Accuracy is rolled per target, so a multi-target skill can partially
        // miss. Friendly effects never roll; see TargetKind::is_hostile.
        if skill.target.is_hostile() && !st.rng.chance(skill.accuracy) {
            log.push(Event::Missed { actor, target });
            continue;
        }
        for effect in &skill.effects {
            apply_effect(st, actor, target, effect, log);
        }
    }

    Ok(skill.tempo_cost.max(1))
}

/// Expands a target kind into concrete indices.
///
/// A `None` request on a single-target skill falls back to the first legal
/// target instead of erroring: AI and scripted actions should not be able to
/// waste a turn on an omitted field.
fn resolve_targets(
    st: &BattleState,
    actor: usize,
    kind: TargetKind,
    requested: Option<usize>,
) -> Result<Vec<usize>, String> {
    let team = st.combatants[actor].team;
    let single_team = match kind {
        TargetKind::SelfOnly => return Ok(vec![actor]),
        TargetKind::AllEnemies => return Ok(st.alive_on(team.opposite())),
        TargetKind::AllAllies => return Ok(st.alive_on(team)),
        TargetKind::OneEnemy => team.opposite(),
        TargetKind::OneAlly => team,
    };

    let candidates = st.alive_on(single_team);
    if candidates.is_empty() {
        return Err("no legal target remains".to_string());
    }
    match requested {
        Some(index) if candidates.contains(&index) => Ok(vec![index]),
        Some(index) => Err(format!(
            "index {index} is not a legal target for this action"
        )),
        None => Ok(vec![candidates[0]]),
    }
}

fn apply_effect(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    effect: &Effect,
    log: &mut Vec<Event>,
) {
    match *effect {
        Effect::Damage {
            power,
            element,
            variance,
        } => {
            let (amount, crit) = compute_damage(st, actor, target, power, element, variance);
            deal_damage(st, actor, target, amount, crit, element, log);
        }
        Effect::Drain { power, element } => {
            let (amount, crit) = compute_damage(st, actor, target, power, element, 0);
            let dealt = deal_damage(st, actor, target, amount, crit, element, log);
            // Recovery is based on damage *actually dealt*, not on the rolled
            // amount, so overkill does not become a healing exploit.
            let recovered = heal(st, actor, dealt / 2);
            if recovered > 0 {
                log.push(Event::Healed {
                    target: actor,
                    amount: recovered,
                });
            }
        }
        Effect::Heal { power } => {
            // Healing scales on the healer's WILL so that support characters
            // have a stat to invest in.
            let will = st.combatants[actor].will();
            let amount = (power * (100 + will) / 100).max(1);
            let healed = heal(st, target, amount);
            if healed > 0 {
                log.push(Event::Healed {
                    target,
                    amount: healed,
                });
            }
        }
        Effect::Status {
            status,
            potency,
            duration,
            chance,
        } => {
            if !st.combatants[target].alive() {
                return;
            }
            // Hostile statuses are resisted by WILL; buffs on your own side are
            // not, because a buff that randomly fails is only frustrating.
            let hostile = st.combatants[target].team != st.combatants[actor].team;
            let effective = if hostile {
                (chance - st.combatants[target].will() / 2).clamp(5, 100)
            } else {
                chance
            };
            if st.rng.chance(effective) {
                st.combatants[target].apply_status(status, potency, duration);
                log.push(Event::StatusApplied {
                    target,
                    status,
                    potency,
                    duration,
                });
            } else {
                log.push(Event::StatusResisted { target, status });
            }
        }
        Effect::TempoLock { ticks } => {
            if !st.combatants[target].alive() {
                return;
            }
            let combatant = &mut st.combatants[target];
            // Take the longer lock rather than summing, so chaining the ability
            // cannot produce an unbounded freeze.
            combatant.tempo_lock = combatant.tempo_lock.max(ticks);
            log.push(Event::TempoLocked { target, ticks });
        }
    }
}

/// Integer damage model. See docs/ARCHITECTURE.md for the rationale.
///
/// Order of operations, and it is load-bearing: power against attack, minus half
/// of defence, then the target's resistance to the element, then variance, then
/// crit. Resistance sits after defence so that it scales the damage that got
/// through armour rather than the number the skill declared, and before variance
/// so that the jitter is jitter on what the target really takes.
///
/// Stats are copied into locals before touching the RNG: the RNG lives inside
/// `BattleState`, so holding a borrow on a combatant across the roll would not
/// borrow-check.
fn compute_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    power: i32,
    element: Element,
    variance: i32,
) -> (i32, bool) {
    let atk = st.combatants[actor].atk();
    let will_actor = st.combatants[actor].will();
    let def = st.combatants[target].def();
    let will_target = st.combatants[target].will();
    // Clamped here as well as in the validator: content can reach this function
    // through the bridge without passing the Python validator at all.
    let resist = st.combatants[target]
        .resistance(element)
        .clamp(MIN_RESISTANCE, MAX_RESISTANCE);

    let base = power * atk / 100;
    // Floor at 1: no amount of defence makes a target immune, which keeps a
    // fight from stalling into an unwinnable state.
    let mut damage = (base - def / 2).max(1);

    if resist != 0 {
        // The floor is reapplied rather than skipped: at 100 the modifier is
        // gone, the hit is not.
        damage = (damage * (100 - resist) / 100).max(1);
    }

    if variance > 0 {
        let variance = variance.min(99);
        let jitter = st.rng.range_i32(100 - variance, 100 + variance);
        damage = (damage * jitter / 100).max(1);
    }

    let crit_chance = (5 + (will_actor - will_target) / 2).clamp(1, 50);
    let crit = st.rng.chance(crit_chance);
    if crit {
        damage = (damage * 3 / 2).max(1);
    }

    (damage, crit)
}

/// Subtracts HP without emitting anything, returning the amount actually lost.
///
/// `None` means the target was already down. Both damage paths share this so
/// that HP can never go negative in one of them and not the other.
fn subtract_hp(st: &mut BattleState, target: usize, amount: i32) -> Option<i32> {
    if !st.combatants[target].alive() {
        return None;
    }
    let before = st.combatants[target].hp;
    let lost = amount.clamp(0, before);
    st.combatants[target].hp = before - lost;
    Some(lost)
}

/// Emits [`Event::Downed`] if the target just reached zero HP.
///
/// Shared by every damage path on purpose: a lethal bleed must end the battle
/// exactly like a lethal sword blow, and duplicating this check is how one of
/// them eventually stops doing it.
fn note_if_downed(st: &BattleState, target: usize, log: &mut Vec<Event>) {
    if st.combatants[target].hp == 0 {
        log.push(Event::Downed { target });
    }
}

/// Applies damage from an actor's action and returns the amount actually dealt
/// (never more than the target's remaining HP).
pub(crate) fn deal_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    amount: i32,
    crit: bool,
    element: Element,
    log: &mut Vec<Event>,
) -> i32 {
    let Some(dealt) = subtract_hp(st, target, amount) else {
        return 0;
    };

    log.push(Event::Damaged {
        actor,
        target,
        amount: dealt,
        crit,
        element,
    });
    note_if_downed(st, target, log);
    dealt
}

/// Applies damage caused by a status the target is carrying, such as bleed.
///
/// Deliberately separate from [`deal_damage`]: there is no attacker and no
/// element, so passing the victim's own index as the actor would emit a log
/// entry claiming the character attacked itself. See [`Event::StatusDamaged`].
pub(crate) fn status_damage(
    st: &mut BattleState,
    target: usize,
    status: StatusKind,
    amount: i32,
    log: &mut Vec<Event>,
) -> i32 {
    let Some(dealt) = subtract_hp(st, target, amount) else {
        return 0;
    };

    log.push(Event::StatusDamaged {
        target,
        status,
        amount: dealt,
    });
    note_if_downed(st, target, log);
    dealt
}

/// Restores HP and returns the amount actually restored. Healing never revives:
/// resurrection must be an explicit effect, not an accident of ordering.
pub(crate) fn heal(st: &mut BattleState, target: usize, amount: i32) -> i32 {
    let combatant = &mut st.combatants[target];
    if !combatant.alive() || amount <= 0 {
        return 0;
    }
    let before = combatant.hp;
    combatant.hp = (before + amount).min(combatant.max_hp);
    combatant.hp - before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AiProfile, CombatantDef, Resistances, Stats, Team};

    /// A zero-variance attack at perfect accuracy: the only roll left in the
    /// pipeline is the crit, and that one lands or not identically in every run
    /// below because the seed and the roll order are the same.
    fn attack(id: &str, element: Element) -> SkillDef {
        SkillDef {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            sp_cost: 0,
            target: TargetKind::OneEnemy,
            accuracy: 100,
            tempo_cost: TEMPO_THRESHOLD,
            effects: vec![Effect::Damage {
                power: 100,
                element,
                variance: 0,
            }],
        }
    }

    fn fighter(id: &str, team: Team, resist: Resistances, skills: &[&str]) -> CombatantDef {
        CombatantDef {
            id: id.to_string(),
            name: id.to_string(),
            team,
            // atk 100 and def 0 make the pre-resistance damage exactly the
            // skill's power, so the assertions below are about one variable.
            stats: Stats {
                hp: 1000,
                sp: 0,
                atk: 100,
                def: 0,
                spd: 10,
                will: 0,
            },
            stand: None,
            skills: skills.iter().map(|id| id.to_string()).collect(),
            ai: AiProfile::Aggressive,
            resist,
        }
    }

    fn table(element: Element, percent: i32) -> Resistances {
        let mut resist = Resistances::new();
        resist.insert(element, percent);
        resist
    }

    /// Resolves one hit against a defender carrying `resist` and returns the
    /// damage the emitted event reports.
    fn hit(skill: &str, resist: Resistances) -> i32 {
        let db = Database::new(
            Vec::new(),
            vec![
                attack("skill.flame", Element::Fire),
                attack("skill.jab", Element::Physical),
            ],
            vec![
                fighter(
                    "pc.attacker",
                    Team::Party,
                    Resistances::new(),
                    &["skill.flame", "skill.jab"],
                ),
                fighter("npc.target", Team::Foe, resist, &["skill.jab"]),
            ],
        )
        .expect("the test content is valid");

        let party = vec!["pc.attacker".to_string()];
        let foes = vec!["npc.target".to_string()];
        let mut st =
            BattleState::build(&db, &party, &foes, 7).expect("the test battle instantiates");
        let mut log: Vec<Event> = Vec::new();
        let command = Command::Skill {
            skill: skill.to_string(),
            target: 1,
        };
        resolve_command(&db, &mut st, 0, &command, &mut log).expect("the command is legal");

        log.iter()
            .find_map(|event| match event {
                Event::Damaged { amount, .. } => Some(*amount),
                _ => None,
            })
            .expect("an attack at 100 accuracy always lands")
    }

    #[test]
    fn a_resisted_element_lands_for_less_and_a_vulnerable_one_for_more() {
        let neutral = hit("skill.flame", Resistances::new());
        // Ratios rather than literals: the crit roll is identical across the
        // three runs, so a literal would be asserting the roll as much as the
        // resistance, and would have to change if the seed ever did.
        assert_eq!(
            hit("skill.flame", table(Element::Fire, 50)) * 2,
            neutral,
            "50 should halve the hit"
        );
        assert_eq!(
            hit("skill.flame", table(Element::Fire, -50)) * 2,
            neutral * 3,
            "-50 should make the hit half again as strong"
        );
    }

    #[test]
    fn total_resistance_removes_the_modifier_not_the_hit() {
        assert_eq!(hit("skill.flame", table(Element::Fire, 100)), 1);
    }

    #[test]
    fn resistance_applies_only_to_the_element_that_was_used() {
        let neutral = hit("skill.jab", Resistances::new());
        assert_eq!(hit("skill.jab", table(Element::Fire, 75)), neutral);
    }
}
