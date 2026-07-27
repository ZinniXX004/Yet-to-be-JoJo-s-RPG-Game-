//! GDExtension boundary. Contains **no game rules**.
//!
//! Design decision: everything crossing the boundary is a JSON string.
//!
//! - No `#[repr(C)]` structs, no pointers, no ownership transfer, so a schema
//!   mismatch between Rust and GDScript surfaces as a parse error rather than
//!   undefined behavior.
//! - The ABI never changes when content gains fields, which matters because the
//!   data schema will churn constantly during design iteration.
//! - Cost is one serialization per player action, not per frame. At this scale
//!   that is irrelevant; claiming otherwise would be premature optimization.
//!
//! Every method returns a JSON object containing at least `ok`. On failure it
//! carries `error` with a human-readable reason, which the UI is expected to
//! surface rather than swallow.
//!
//! Incoming payloads are normalized before deserialization, because Godot's
//! JSON has no integer type. See [`rpg_core::json_compat`].

use godot::prelude::*;
use rpg_core::{json_compat, Battle, Command};

struct RpgBridge;

#[gdextension]
unsafe impl ExtensionLibrary for RpgBridge {}

/// Serializes a JSON value into a Godot string.
///
/// `GString` implements `From<&str>` and `From<&String>` but deliberately not
/// `From<String>`: the conversion copies into Godot-managed memory, so accepting
/// an owned `String` would silently discard an allocation. Borrowing makes that
/// visible.
///
/// Funnelling every conversion through one function is not ceremony. godot-rust
/// is pre-1.0, and when its string API shifts again this is the only line that
/// has to change instead of one per method.
fn json_to_gstring(payload: &serde_json::Value) -> GString {
    GString::from(&payload.to_string())
}

fn error_json(message: &str) -> GString {
    let payload = serde_json::json!({ "ok": false, "error": message });
    json_to_gstring(&payload)
}

/// Converts a payload authored in GDScript into one the schema accepts.
///
/// GDScript has no JSON integer: `JSON.parse_string` yields floats and
/// `JSON.stringify` writes `95.0`, which `serde` rejects for an `i32` field.
/// Normalizing here keeps the fix in the translation layer, where it belongs,
/// instead of weakening the simulation's integer-only schema.
fn normalize_incoming(json: &GString, what: &str) -> Result<String, GString> {
    json_compat::normalize_json_str(&json.to_string())
        .map_err(|error| error_json(&format!("malformed {what} json: {error}")))
}

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct BattleSession {
    battle: Option<Battle>,
}

#[godot_api]
impl BattleSession {
    /// Creates the battle. `config_json` matches `rpg_core::BattleConfig`:
    /// `{ seed, stands, skills, combatants, party, foes }`.
    #[func]
    fn create(&mut self, config_json: GString) -> GString {
        let normalized = match normalize_incoming(&config_json, "battle config") {
            Ok(json) => json,
            Err(failure) => return failure,
        };
        match Battle::from_json(&normalized) {
            Ok(battle) => {
                self.battle = Some(battle);
                GString::from("{\"ok\":true}")
            }
            Err(error) => error_json(&error.to_string()),
        }
    }

    /// Runs the scheduler to the next decision point.
    /// Returns `{ ok, phase, events }`. Events are drained, so the caller must
    /// consume them; calling twice without animating loses the first batch.
    #[func]
    fn advance(&mut self) -> GString {
        let Some(battle) = self.battle.as_mut() else {
            return error_json("session not created");
        };
        let phase = battle.advance();
        let payload = serde_json::json!({
            "ok": true,
            "phase": phase,
            "events": battle.take_events(),
        });
        json_to_gstring(&payload)
    }

    /// Submits one command for the awaiting actor.
    /// `command_json` examples:
    ///   `{"kind":"attack","target":3}`
    ///   `{"kind":"skill","skill":"skill.tempo_halt","target":3}`
    ///   `{"kind":"guard"}` / `{"kind":"wait"}`
    ///
    /// A rejected command leaves the turn with the actor on purpose, so the UI
    /// can show the reason and re-prompt.
    #[func]
    fn submit(&mut self, command_json: GString) -> GString {
        let Some(battle) = self.battle.as_mut() else {
            return error_json("session not created");
        };
        let normalized = match normalize_incoming(&command_json, "command") {
            Ok(json) => json,
            Err(failure) => return failure,
        };
        let command: Command = match serde_json::from_str(&normalized) {
            Ok(command) => command,
            Err(error) => return error_json(&format!("malformed command: {error}")),
        };
        match battle.submit(&command) {
            Ok(()) => {
                let payload = serde_json::json!({
                    "ok": true,
                    "events": battle.take_events(),
                });
                json_to_gstring(&payload)
            }
            Err(reason) => error_json(&reason),
        }
    }

    /// Lets the AI act for the current actor. Used for enemy turns and
    /// auto-battle.
    #[func]
    fn step_with_ai(&mut self) -> GString {
        let Some(battle) = self.battle.as_mut() else {
            return error_json("session not created");
        };
        let phase = battle.step_with_ai();
        let payload = serde_json::json!({
            "ok": true,
            "phase": phase,
            "events": battle.take_events(),
        });
        json_to_gstring(&payload)
    }

    /// Full state snapshot for the HUD. Also a complete save payload: there is
    /// no hidden state outside BattleState.
    #[func]
    fn state_json(&self) -> GString {
        let Some(battle) = self.battle.as_ref() else {
            return error_json("session not created");
        };
        match serde_json::to_string(battle.state()) {
            Ok(json) => GString::from(&json),
            Err(error) => error_json(&error.to_string()),
        }
    }
}
