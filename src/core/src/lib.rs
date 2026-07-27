//! `rpg_core` -- deterministic, engine-agnostic turn-based battle simulation.
//!
//! Invariants that the rest of the project depends on:
//!
//! 1. **No engine types.** This crate compiles and tests without Godot.
//! 2. **No ambient state.** The RNG lives inside [`BattleState`], never in a
//!    global. There is no clock access, no threading, and no float math.
//! 3. **No content in code.** Characters, stands, and skills come from
//!    `data/*.json` via [`Database`].
//!
//! Together those give the central property: the same seed plus the same
//! sequence of [`Command`]s always produces the same sequence of [`Event`]s.
//! That is what makes battles replayable, testable, and cheap to serialize.

pub mod ai;
pub mod battle;
pub mod command;
pub mod data;
pub mod event;
pub mod resolve;
pub mod rng;
pub mod state;

pub use battle::{Battle, BattleConfig, BattleOutcome, Phase};
pub use command::Command;
pub use data::{
    AiProfile, CombatantDef, DataError, Database, Effect, Element, Id, SkillDef, StandDef, Stats,
    StatusKind, TargetKind, Team,
};
pub use event::Event;
pub use rng::Rng;
pub use state::{BattleState, Combatant, Status, TEMPO_THRESHOLD};
