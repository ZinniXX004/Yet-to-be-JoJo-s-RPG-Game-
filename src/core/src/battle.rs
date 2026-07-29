//! The scheduler and the public entry point of the simulation.
//!
//! Turn order is a tempo (ATB) model, not round-robin: each tick every living
//! combatant accumulates tempo equal to its effective speed, and acts once it
//! crosses [`TEMPO_THRESHOLD`]. Acting subtracts the action's own cost, so a slow
//! heavy skill genuinely delays the next turn. That single rule is what makes
//! speed a resource and makes tempo denial a real strategy.
//!
//! Resources recover with time as well: each turn returns
//! [`SP_REGEN_PER_TURN`] to the combatant that took it, capped at that
//! combatant's own pool. Without a trickle, SP is a one-shot budget, the
//! correct play is to empty it in the opening exchange, and every turn after
//! that is a basic attack -- which is how `matchup.dio_boss` came to be decided
//! by whose hit points were larger rather than by whose skills were better.
//!
//! The loop is driven by the caller. [`Battle::advance`] runs until a decision
//! is required and returns; it never blocks and never calls back into the
//! presentation layer.

use serde::{Deserialize, Serialize};

use crate::ai;
use crate::command::Command;
use crate::data::{CombatantDef, DataError, Database, Id, SkillDef, StandDef, StatusKind, Team};
use crate::event::Event;
use crate::resolve;
use crate::state::{BattleState, SP_REGEN_PER_TURN, TEMPO_THRESHOLD};

/// Hard stop on scheduler ticks. Without it, a content bug (every combatant
/// tempo-locked forever, zero-damage stalemate) would hang the game instead of
/// reporting a stalemate.
const MAX_TICKS: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleOutcome {
    PartyWins,
    PartyWipes,
    Stalemate,
}

/// What the simulation needs from the caller next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Phase {
    AwaitingCommand { actor: usize },
    Finished { outcome: BattleOutcome },
}

/// Everything needed to start a battle from JSON, so the Godot layer never has
/// to construct Rust types.
#[derive(Clone, Debug, Deserialize)]
pub struct BattleConfig {
    #[serde(default)]
    pub seed: u64,
    pub stands: Vec<StandDef>,
    pub skills: Vec<SkillDef>,
    pub combatants: Vec<CombatantDef>,
    pub party: Vec<Id>,
    pub foes: Vec<Id>,
}

pub struct Battle {
    db: Database,
    state: BattleState,
    log: Vec<Event>,
    /// Set while a command is expected. Kept on failure so a rejected command
    /// can be retried without losing the turn.
    awaiting: Option<usize>,
    finished: Option<BattleOutcome>,
}

impl Battle {
    pub fn new(db: Database, party: &[Id], foes: &[Id], seed: u64) -> Result<Self, DataError> {
        let state = BattleState::build(&db, party, foes, seed)?;
        Ok(Battle {
            db,
            state,
            log: vec![Event::BattleStarted { seed }],
            awaiting: None,
            finished: None,
        })
    }

    pub fn from_config(config: BattleConfig) -> Result<Self, DataError> {
        let db = Database::new(config.stands, config.skills, config.combatants)?;
        Battle::new(db, &config.party, &config.foes, config.seed)
    }

    pub fn from_json(config_json: &str) -> Result<Self, DataError> {
        let config: BattleConfig = serde_json::from_str(config_json)
            .map_err(|e| DataError::Parse(format!("battle config: {e}")))?;
        Battle::from_config(config)
    }

    pub fn state(&self) -> &BattleState {
        &self.state
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    pub fn log(&self) -> &[Event] {
        &self.log
    }

    /// Drains the event log. The caller is expected to animate what it takes;
    /// calling twice without consuming the first batch loses it.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.log)
    }

    pub fn outcome(&self) -> Option<BattleOutcome> {
        self.finished
    }

    /// Runs the scheduler until a command is required or the battle ends.
    pub fn advance(&mut self) -> Phase {
        if let Some(outcome) = self.finished {
            return Phase::Finished { outcome };
        }
        if let Some(actor) = self.awaiting {
            return Phase::AwaitingCommand { actor };
        }

        loop {
            if let Some(outcome) = self.check_end() {
                return self.finish(outcome);
            }
            if self.state.tick >= MAX_TICKS {
                return self.finish(BattleOutcome::Stalemate);
            }

            match self.ready_actor() {
                None => self.tick(),
                Some(actor) => {
                    if self.state.combatants[actor].has(StatusKind::Stun) {
                        // A stunned actor still pays a full turn of tempo, so
                        // stun costs time rather than merely delaying an action.
                        self.log.push(Event::TurnSkipped {
                            actor,
                            reason: "stun".to_string(),
                        });
                        self.end_turn(actor, TEMPO_THRESHOLD);
                        continue;
                    }
                    self.log.push(Event::TurnStarted {
                        actor,
                        tick: self.state.tick,
                    });
                    self.awaiting = Some(actor);
                    return Phase::AwaitingCommand { actor };
                }
            }
        }
    }

    /// Submits a command for the awaiting actor.
    ///
    /// On `Err` the actor keeps the turn on purpose: illegal input should produce
    /// a message and a re-prompt, never a silently wasted turn.
    pub fn submit(&mut self, command: &Command) -> Result<(), String> {
        let actor = self
            .awaiting
            .ok_or_else(|| "no actor is awaiting a command".to_string())?;
        let cost =
            resolve::resolve_command(&self.db, &mut self.state, actor, command, &mut self.log)?;
        self.awaiting = None;
        self.end_turn(actor, cost);
        Ok(())
    }

    /// Advances one decision point, letting the AI act for whoever is up.
    pub fn step_with_ai(&mut self) -> Phase {
        let phase = self.advance();
        let actor = match phase {
            Phase::AwaitingCommand { actor } => actor,
            Phase::Finished { .. } => return phase,
        };

        let command = ai::choose(&self.db, &mut self.state, actor);
        if let Err(reason) = self.submit(&command) {
            // An AI that cannot produce a legal move must not stall the
            // scheduler; burn the turn and record why.
            self.log.push(Event::TurnSkipped { actor, reason });
            self.awaiting = None;
            self.end_turn(actor, TEMPO_THRESHOLD);
        }
        self.advance()
    }

    /// Runs an unattended battle to its end. Used by tests and by the balance
    /// harness. `max_turns` bounds the run independently of `MAX_TICKS`.
    pub fn run_to_completion(&mut self, max_turns: u64) -> BattleOutcome {
        while self.finished.is_none() && self.state.turn < max_turns {
            self.step_with_ai();
        }
        if self.finished.is_none() {
            self.finish(BattleOutcome::Stalemate);
        }
        self.finished.unwrap_or(BattleOutcome::Stalemate)
    }

    fn check_end(&self) -> Option<BattleOutcome> {
        match (
            self.state.team_alive(Team::Party),
            self.state.team_alive(Team::Foe),
        ) {
            (false, _) => Some(BattleOutcome::PartyWipes),
            (true, false) => Some(BattleOutcome::PartyWins),
            _ => None,
        }
    }

    fn finish(&mut self, outcome: BattleOutcome) -> Phase {
        if self.finished.is_none() {
            self.finished = Some(outcome);
            self.awaiting = None;
            self.log.push(Event::BattleEnded { outcome });
        }
        Phase::Finished {
            outcome: self.finished.unwrap_or(outcome),
        }
    }

    /// Charges tempo, returns a turn's worth of SP, then ticks the actor's own
    /// statuses. Status duration and SP recovery are both counted per *turn of
    /// the combatant*, not per global tick, so a fast character burns through
    /// its buffs faster and recovers its reserve faster. That is intentional:
    /// the engine has one notion of a turn, not two.
    ///
    /// A turn burned to a stun recovers as well. The fighter did nothing, and
    /// standing still is exactly when a reserve comes back.
    fn end_turn(&mut self, actor: usize, cost: u32) {
        {
            let combatant = &mut self.state.combatants[actor];
            combatant.tempo = combatant.tempo.saturating_sub(cost.max(1));
            combatant.recover_sp(SP_REGEN_PER_TURN);
        }
        self.tick_statuses(actor);
        self.state.turn += 1;
    }

    fn tick_statuses(&mut self, actor: usize) {
        let mut bleed = 0;
        let mut regen = 0;
        let mut expired: Vec<StatusKind> = Vec::new();

        for status in &mut self.state.combatants[actor].statuses {
            match status.kind {
                StatusKind::Bleed => bleed += status.potency.max(1),
                StatusKind::Regen => regen += status.potency.max(1),
                _ => {}
            }
            status.remaining = status.remaining.saturating_sub(1);
            if status.remaining == 0 {
                expired.push(status.kind);
            }
        }
        self.state.combatants[actor]
            .statuses
            .retain(|status| status.remaining > 0);

        for status in expired {
            self.log.push(Event::StatusExpired {
                target: actor,
                status,
            });
        }

        if bleed > 0 {
            // Damage over time has no attacker, so it gets its own event rather
            // than a Damaged with the victim named as its own assailant.
            resolve::status_damage(
                &mut self.state,
                actor,
                StatusKind::Bleed,
                bleed,
                &mut self.log,
            );
        }
        if regen > 0 {
            let healed = resolve::heal(&mut self.state, actor, regen);
            if healed > 0 {
                self.log.push(Event::Healed {
                    target: actor,
                    amount: healed,
                });
            }
        }
    }

    fn tick(&mut self) {
        self.state.tick += 1;
        for combatant in &mut self.state.combatants {
            if !combatant.alive() {
                continue;
            }
            if combatant.tempo_lock > 0 {
                combatant.tempo_lock -= 1;
            } else {
                let speed = combatant.spd().max(1) as u32;
                combatant.tempo = combatant.tempo.saturating_add(speed);
            }
        }
    }

    /// Highest tempo above the threshold acts first; ties break on the lower
    /// index. Deterministic tie-breaking is not a detail -- without it, replay
    /// would depend on iteration order.
    fn ready_actor(&self) -> Option<usize> {
        self.state
            .combatants
            .iter()
            .enumerate()
            .filter(|(_, combatant)| {
                combatant.alive() && combatant.tempo_lock == 0 && combatant.tempo >= TEMPO_THRESHOLD
            })
            .max_by_key(|(index, combatant)| (combatant.tempo, std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AiProfile, Effect, Element, Stats, TargetKind};

    fn stats(hp: i32, atk: i32, spd: i32) -> Stats {
        Stats {
            hp,
            sp: 50,
            atk,
            def: 10,
            spd,
            will: 10,
        }
    }

    fn strike() -> SkillDef {
        SkillDef {
            id: resolve::BASIC_ATTACK_ID.to_string(),
            name: "Strike".to_string(),
            description: String::new(),
            sp_cost: 0,
            target: TargetKind::OneEnemy,
            accuracy: 100,
            tempo_cost: TEMPO_THRESHOLD,
            effects: vec![Effect::Damage {
                power: 100,
                element: Element::Physical,
                variance: 0,
            }],
        }
    }

    fn combatant(id: &str, team: Team, hp: i32, atk: i32, spd: i32) -> CombatantDef {
        CombatantDef {
            id: id.to_string(),
            name: id.to_string(),
            team,
            stats: stats(hp, atk, spd),
            stand: None,
            skills: vec![resolve::BASIC_ATTACK_ID.to_string()],
            ai: AiProfile::Aggressive,
        }
    }

    fn battle(hero_spd: i32, foe_spd: i32) -> Battle {
        let db = Database::new(
            vec![],
            vec![strike()],
            vec![
                combatant("hero", Team::Party, 200, 100, hero_spd),
                combatant("foe", Team::Foe, 60, 20, foe_spd),
            ],
        )
        .expect("fixture content must be valid");
        Battle::new(db, &["hero".to_string()], &["foe".to_string()], 42)
            .expect("fixture battle must build")
    }

    #[test]
    fn faster_combatant_acts_first() {
        let mut battle = battle(120, 40);
        match battle.advance() {
            Phase::AwaitingCommand { actor } => assert_eq!(actor, 0),
            other => panic!("expected a command request, got {other:?}"),
        }
    }

    #[test]
    fn illegal_command_does_not_consume_the_turn() {
        let mut battle = battle(120, 40);
        let Phase::AwaitingCommand { actor } = battle.advance() else {
            panic!("expected a command request");
        };
        assert!(battle.submit(&Command::Attack { target: 99 }).is_err());
        // Still the same actor's turn: the rejection cost nothing.
        match battle.advance() {
            Phase::AwaitingCommand { actor: again } => assert_eq!(actor, again),
            other => panic!("expected the same actor to retain the turn, got {other:?}"),
        }
    }

    #[test]
    fn battle_reaches_a_decisive_outcome() {
        let mut battle = battle(120, 40);
        assert_eq!(battle.run_to_completion(200), BattleOutcome::PartyWins);
        assert!(matches!(
            battle.log().last(),
            Some(Event::BattleEnded { .. })
        ));
    }

    /// Recovery is per turn taken, so the combatant that acted is the only one
    /// whose reserve moves. A tick-based trickle would quietly pay the whole
    /// field, including whoever is standing tempo-locked.
    #[test]
    fn a_turn_returns_sp_to_the_actor_who_took_it_and_to_nobody_else() {
        let mut battle = battle(120, 40);
        let Phase::AwaitingCommand { actor } = battle.advance() else {
            panic!("expected a command request");
        };
        battle.state.combatants[0].sp = 0;
        battle.state.combatants[1].sp = 0;

        battle
            .submit(&Command::Attack { target: 1 })
            .expect("attacking the living foe is legal");

        assert_eq!(
            battle.state.combatants[actor].sp, SP_REGEN_PER_TURN,
            "the actor should recover exactly one turn's worth of SP"
        );
        assert_eq!(
            battle.state.combatants[1].sp, 0,
            "a combatant that never took a turn recovers nothing"
        );
    }

    #[test]
    fn recovery_never_pushes_a_pool_above_where_it_started() {
        let mut battle = battle(120, 40);
        let Phase::AwaitingCommand { actor } = battle.advance() else {
            panic!("expected a command request");
        };
        let full = battle.state.combatants[actor].max_sp;
        battle.state.combatants[actor].sp = full;

        battle
            .submit(&Command::Attack { target: 1 })
            .expect("attacking the living foe is legal");

        assert_eq!(
            battle.state.combatants[actor].sp, full,
            "a full pool must stay at its maximum, not drift above it"
        );
    }

    /// Regression test for a defect found in the first end-to-end run, where a
    /// bleed tick was logged as `Damaged { actor: 0, target: 0 }` -- the victim
    /// attacking itself with a physical hit it never made.
    #[test]
    fn bleed_is_attributed_to_the_status_not_to_its_victim() {
        let mut battle = battle(120, 40);
        let Phase::AwaitingCommand { actor } = battle.advance() else {
            panic!("expected a command request");
        };
        assert_eq!(actor, 0, "the fast hero should be up first");

        battle.state.combatants[actor].apply_status(StatusKind::Bleed, 7, 3);
        // Discard the opening events so the assertions below can only see what
        // this turn produced.
        battle.take_events();

        battle
            .submit(&Command::Attack { target: 1 })
            .expect("attacking the living foe is legal");
        let events = battle.take_events();

        assert!(
            events.iter().any(|event| matches!(
                event,
                Event::StatusDamaged {
                    target: 0,
                    status: StatusKind::Bleed,
                    amount: 7,
                }
            )),
            "bleed must report itself as the cause: {events:?}"
        );
        assert!(
            !events.iter().any(|event| matches!(
                event,
                Event::Damaged {
                    actor: 0,
                    target: 0,
                    ..
                }
            )),
            "no event may claim a combatant attacked itself: {events:?}"
        );
    }
}
