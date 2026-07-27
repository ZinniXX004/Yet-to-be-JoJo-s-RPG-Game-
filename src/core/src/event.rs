//! Immutable facts produced by resolution.
//!
//! The presentation layer animates events; it never recomputes outcomes. That
//! separation is what allows the same event log to be replayed, written to a bug
//! report, or diffed in a regression test.
//!
//! Never add a field here that only makes sense visually (screen shake, sprite
//! offsets, timings). Those belong in the Godot layer.
//!
//! Corollary that cost us a real bug: an event must never carry a value that is
//! merely plausible. Damage over time has no attacker, so it does not get an
//! `actor` field -- it gets its own variant. Filling a field with the victim's
//! own index to satisfy the type produced a log claiming a character attacked
//! itself, and a UI that faithfully animates events would have shown exactly
//! that.

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
    /// One combatant hurt another. `actor` is always a real attacker, which is
    /// what lets an animator bind this event to an attack animation with no
    /// special cases. Damage without an attacker is [`Event::StatusDamaged`].
    Damaged {
        actor: usize,
        target: usize,
        amount: i32,
        crit: bool,
        element: Element,
    },
    /// Damage from a status the target is carrying, such as bleed.
    ///
    /// No `actor`: by the time it ticks, the combatant that applied the status
    /// may be dead, and crediting it would be misleading anyway -- the damage is
    /// caused by the condition, not by an action. No `element` either, for the
    /// same reason the old code was wrong to claim `physical`: the status is the
    /// cause, and it names itself. No `crit`, because there is no roll.
    StatusDamaged {
        target: usize,
        status: StatusKind,
        amount: i32,
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
