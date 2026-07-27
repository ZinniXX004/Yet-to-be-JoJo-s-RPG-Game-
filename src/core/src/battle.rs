//! The battle driver: scheduler, turn lifecycle, and public API.
//!
//! Control flow is pull-based, not callback-based. The caller repeatedly asks
//! [`Battle::advance`] what should happen next; the core never calls back into
//! the presentation layer. That keeps the engine boundary one-directional and
//! makes the whole simulation trivially driveable from a test.

use serde::{Deserialize, Serialize};

use crate::ai;
use crate::command::Command;
use crate::data::{CombatantDef, DataError, Database, Element, Id, SkillDef, StandDef, StatusKind};
use crate::event::Event;
use crate::resolve;
use crate::state::{BattleState, TEMPO_THRESHOLD};

/// Safety valve. A correctly configured battle resolves in far fewer ticks; if
/// this is ever reached, the content is broken (for example, mutual immortality)
/// and reporting a stalemate is better than hanging the game loop.
const MAX_TICKS: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleOutcome {
    PartyWins,
    PartyWipes,
    Stalemate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Phase {
    AwaitingCommand { actor: usize },
    Finished { outcome: BattleOutcome },
}

/// Everything needed to start a battle, in one deserializable payload. This is
/// the shape the GDExtension bridge accepts, so the FFI signature never has to
/// change when content gains fields.
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

    pub fn events(&self) -> &[Event] {
        &self.log
    }

    /// Hands the accumulated events to the caller and clears the buffer. The
    /// presentation layer drains after each step so memory does not grow without
    /// bound during long fights.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.log)
    }

    pub fn awaiting(&self) -> Option<usize> {
        self.awaiting
    }

    /// Runs the scheduler until someone must act or the battle ends.
    pub fn advance(&mut self) -> Phase {
        if let Some(outcome) = self.finished {
            return Phase::Finished { outcome };
        }
        if let Some(actor) = self.awaiting {
            return Phase::AwaitingCommand { actor };
        }

        loop {
            if let Some(outcome) = self.outcome() {
                return self.finish(outcome);
            }
            if let Some(actor) = self.ready_actor() {
                if self.state.combatants[actor].has(StatusKind::Stun) {
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
            if self.state.tick >= MAX_TICKS {
                return self.finish(BattleOutcome::Stalemate);
            }
            self.tick();
        }
    }

    /// Submits a command for the awaiting actor.
    ///
    /// On `Err` the actor keeps the turn, so the UI can report the reason and let
    /// the player choose again. Never auto-substitute a different action here;
    /// that hides input bugs.
    pub fn submit(&mut self, cmd: &Command) -> Result<(), String> {
        let actor = self
            .awaiting
            .ok_or_else(|| "no actor is awaiting a command".to_string())?;
        let cost = resolve::resolve_command(&self.db, &mut self.state, actor, cmd, &mut self.log)?;
        self.awaiting = None;
        self.end_turn(actor, cost);
        Ok(())
    }

    /// Advances one step, letting the AI act for whoever is up. Used for enemy
    /// turns, auto-battle, and the headless test suite.
    pub fn step_with_ai(&mut self) -> Phase {
        let phase = self.advance();
        if let Phase::AwaitingCommand { actor } = phase {
            let cmd = ai::choose(&self.db, &mut self.state, actor);
            if let Err(reason) = self.submit(&cmd) {
                // An AI that proposes an illegal command is a bug, but stalling
                // the battle would be worse. Record it and pass the turn.
                debug_assert!(false, "ai produced an illegal command: {reason}");
                let _ = self.submit(&Command::Wait);
            }
        }
        phase
    }

    /// Drives both sides with the AI until the battle resolves. `max_turns`
    /// bounds the loop independently of the tick safety valve.
    pub fn run_with_ai(&mut self, max_turns: u64) -> BattleOutcome {
        for _ in 0..max_turns {
            if let Phase::Finished { outcome } = self.step_with_ai() {
                return outcome;
            }
        }
        BattleOutcome::Stalemate
    }

    fn outcome(&self) -> Option<BattleOutcome> {
        let party = self.state.team_alive(crate::data::Team::Party);
        let foes = self.state.team_alive(crate::data::Team::Foe);
        match (party, foes) {
            (true, false) => Some(BattleOutcome::PartyWins),
            (false, _) => Some(BattleOutcome::PartyWipes),
            _ => None,
        }
    }

    fn finish(&mut self, outcome: BattleOutcome) -> Phase {
        if self.finished.is_none() {
            self.finished = Some(outcome);
            self.awaiting = None;
            self.log.push(Event::BattleEnded { outcome });
        }
        Phase::Finished { outcome }
    }

    /// Highest tempo acts first; ties break to the lowest index. Explicit and
    /// stable, because "whoever the iterator happened to yield" is how replay
    /// determinism dies.
    fn ready_actor(&self) -> Option<usize> {
        self.state
            .combatants
            .iter()
            .enumerate()
            .filter(|(_, c)| c.alive() && c.tempo_lock == 0 && c.tempo >= TEMPO_THRESHOLD)
            .max_by_key(|(index, c)| (c.tempo, std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
    }

    fn tick(&mut self) {
        self.state.tick += 1;
        for combatant in self.state.combatants.iter_mut() {
            if !combatant.alive() {
                continue;
            }
            if combatant.tempo_lock > 0 {
                combatant.tempo_lock -= 1;
                continue;
            }
            let speed = combatant.spd().max(1) as u32;
            combatant.tempo = combatant.tempo.saturating_add(speed);
        }
    }

    /// Charges tempo, resolves damage/heal over time, then ages statuses.
    ///
    /// Order matters: ticking a status before it has had a chance to act would
    /// make a 1-turn buff effectively worthless.
    fn end_turn(&mut self, actor: usize, cost: u32) {
        self.state.turn += 1;
        {
            let combatant = &mut self.state.combatants[actor];
            combatant.tempo = combatant.tempo.saturating_sub(cost.max(1));
        }

        let bleed = self
            .state
            .combatants[actor]
            .status(StatusKind::Bleed)
            .map(|status| status.potency)
            .unwrap_or(0);
        let regen = self
            .state
            .combatants[actor]
            .status(StatusKind::Regen)
            .map(|status| status.potency)
            .unwrap_or(0);

        if bleed > 0 {
            resolve::deal_flat_damage(
                &mut self.state,
                actor,
                actor,
                bleed,
                Element::Fire,
                false,
                &mut self.log,
            );
        }
        if regen > 0 {
            resolve::heal(&mut self.state, actor, regen, &mut self.log);
        }

        let mut expired: Vec<StatusKind> = Vec::new();
        {
            let combatant = &mut self.state.combatants[actor];
            for status in combatant.statuses.iter_mut() {
                status.remaining = status.remaining.saturating_sub(1);
            }
            combatant.statuses.retain(|status| {
                if status.remaining == 0 {
                    expired.push(status.kind);
                    false
                } else {
                    true
                }
            });
        }
        for status in expired {
            self.log.push(Event::StatusExpired {
                target: actor,
                status,
            });
        }
    }
}
