//! Player and AI *intent*.
//!
//! A command is a request, not a result. It is validated before resolution and
//! may be rejected with a reason; the presentation layer can surface that reason
//! instead of silently swallowing an illegal input.
//!
//! `target` is an index into [`crate::state::BattleState::combatants`]. Indices
//! rather than ids because indices are stable for the lifetime of a battle,
//! cheap to compare, and avoid any hash-order dependence that would threaten
//! determinism.

use serde::{Deserialize, Serialize};

use crate::data::Id;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    /// Basic attack. Resolves through the data-defined basic attack skill so
    /// that its numbers stay tunable without a recompile.
    Attack { target: usize },
    Skill { skill: Id, target: usize },
    Guard,
    /// Yield the turn at half the usual tempo cost. Always legal, which
    /// guarantees the scheduler can never deadlock on an actor with no options.
    Wait,
}
