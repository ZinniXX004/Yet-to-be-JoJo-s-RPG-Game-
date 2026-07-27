//! Immutable facts produced by resolution.
//!
//! The presentation layer animates events; it never recomputes outcomes. That
//! separation is what allows the same event log to be replayed, written to a bug
//! report, or diffed in a regression test.
//!
//! Never add a field here that only makes sense visually (screen shake, sprite
//! offsets, timings). Those belong in the Godot layer.

use serde::{Deserialize, Serialize};

use crate::battle::BattleOutcome;
use crate::data::{Element, StatusKind};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    BattleStarted {
        seed: u64,
    },
    TurnStarted {
        actor: usize,
        tick: u64,
    },
    TurnSkipped {
        actor: usize,
        reason: String,
    },
    ActionUsed {
        actor: usize,
        skill: String,
        name: String,
    },
    Waited {
        actor: usize,
    },
    SpConsumed {
        actor: usize,
        amount: i32,
    },
    Missed {
        actor: usize,
        target: usize,
    },
    Damaged {
        actor: usize,
        target: usize,
        amount: i32,
        crit: bool,
        element: Element,
    },
    Healed {
        target: usize,
        amount: i32,
    },
    StatusApplied {
        target: usize,
        status: StatusKind,
        potency: i32,
        duration: u8,
    },
    StatusResisted {
        target: usize,
        status: StatusKind,
    },
    StatusExpired {
        target: usize,
        status: StatusKind,
    },
    TempoLocked {
        target: usize,
        ticks: u32,
    },
    Downed {
        target: usize,
    },
    BattleEnded {
        outcome: BattleOutcome,
    },
}
