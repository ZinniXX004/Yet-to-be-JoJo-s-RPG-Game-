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
//!
//! It is also what makes [`sim`] possible: replaying a fixed list of seeds is
//! only a measurement if the same list always produces the same battles. The
//! balance harness lives here, in the library, rather than in the binary that
//! prints it, so that its output can be asserted on in a test.

pub mod ai;
pub mod battle;
pub mod command;
pub mod data;
pub mod event;
pub mod json_compat;
pub mod report;
pub mod resolve;
pub mod rng;
pub mod sim;
pub mod state;

pub use battle::{Battle, BattleConfig, BattleOutcome, Phase};
pub use command::Command;
pub use data::{
    AiProfile, CombatantDef, DataError, Database, Effect, Element, Id, SkillDef, StandDef, Stats,
    StatusKind, TargetKind, Team,
};
pub use event::Event;
pub use json_compat::{normalize_integral_floats, normalize_json_str};
pub use report::{BatchReport, CombatantStats, MatchupReport, WinRateBand};
pub use rng::Rng;
pub use sim::{parse_matchups, run_batch, run_matchup, Matchup};
pub use state::{BattleState, Combatant, Status, TEMPO_THRESHOLD};
