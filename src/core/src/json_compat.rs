//! JSON compatibility helpers for hosts whose JSON has no integer type.
//!
//! Godot is one of them. GDScript's `JSON.parse_string` represents every number
//! as a `float`, and `JSON.stringify` then writes it back as `95.0`. Content
//! that leaves `data/*.json` as `95`, is parsed by GDScript and forwarded to the
//! simulation therefore arrives with a decimal point, and `serde` correctly
//! refuses it:
//!
//! ```text
//! invalid type: floating point `95.0`, expected i32
//! ```
//!
//! Three ways to respond, and only one of them is right:
//!
//! 1. Change the schema to `f32`. Rejected outright. Float arithmetic is not
//!    bit-reproducible across compilers and platforms, so this would destroy
//!    determinism -- the single property the whole architecture is built on.
//! 2. Annotate every integer field with `deserialize_with`. Dozens of fields
//!    across [`Stats`](crate::Stats), [`SkillDef`](crate::SkillDef),
//!    [`Effect`](crate::Effect) and [`CombatantDef`](crate::CombatantDef), and
//!    every field added later is a fresh opportunity to forget.
//! 3. Normalize once, at the boundary, before deserialization. One function,
//!    total coverage, no schema compromise. This module.
//!
//! Note on the crate's "no float math" invariant: nothing here computes with
//! floats. The only inspection is whether a number that already exists in the
//! input has a zero fractional part. No simulation value is ever a float, and
//! this code runs strictly before any state exists.

use serde_json::Value;

/// Recursively demotes every float with a zero fractional part to an integer.
///
/// Left untouched: numbers that are already integers, numbers with a real
/// fractional part, values outside `i64` range, and every non-numeric value --
/// notably strings, so `"95.0"` stays a string and a malformed id is still
/// reported as a type error rather than being silently coerced.
///
/// # Examples
///
/// ```
/// use rpg_core::json_compat::normalize_integral_floats;
/// use serde_json::{json, Value};
///
/// let mut value = json!({ "hp": 95.0, "rate": 1.5 });
/// normalize_integral_floats(&mut value);
///
/// assert_eq!(value["hp"], Value::from(95_i64));
/// assert_eq!(value["rate"], Value::from(1.5_f64));
/// ```
pub fn normalize_integral_floats(value: &mut Value) {
    match value {
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                return;
            }
            let Some(float) = number.as_f64() else {
                return;
            };
            let representable = float >= (i64::MIN as f64) && float <= (i64::MAX as f64);
            if float.fract() == 0.0 && representable {
                *value = Value::Number((float as i64).into());
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalize_integral_floats),
        Value::Object(entries) => entries.values_mut().for_each(normalize_integral_floats),
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

/// Parses `json`, normalizes it with [`normalize_integral_floats`], and returns
/// it serialized again.
///
/// Intended for FFI entry points: normalize the incoming text, then deserialize
/// the result into the real type so that schema errors are still reported
/// against the schema.
///
/// # Errors
///
/// Returns the underlying [`serde_json::Error`] if `json` is not valid JSON.
pub fn normalize_json_str(json: &str) -> Result<String, serde_json::Error> {
    let mut value: Value = serde_json::from_str(json)?;
    normalize_integral_floats(&mut value);
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[test]
    fn integral_floats_become_integers_at_every_depth() {
        let mut value = json!({
            "hp": 95.0,
            "stats": { "spd": 12.0 },
            "list": [{ "potency": 50.0 }, 7.0],
        });

        normalize_integral_floats(&mut value);

        assert_eq!(value["hp"], Value::from(95_i64));
        assert_eq!(value["stats"]["spd"], Value::from(12_i64));
        assert_eq!(value["list"][0]["potency"], Value::from(50_i64));
        assert_eq!(value["list"][1], Value::from(7_i64));
    }

    #[test]
    fn fractional_numbers_are_preserved() {
        let mut value = json!({ "multiplier": 1.5, "tiny": 0.25 });

        normalize_integral_floats(&mut value);

        assert_eq!(value["multiplier"], Value::from(1.5_f64));
        assert_eq!(value["tiny"], Value::from(0.25_f64));
    }

    #[test]
    fn non_numeric_values_are_untouched() {
        let mut value = json!({
            "id": "skill.strike",
            "looks_numeric": "95.0",
            "flag": true,
            "nothing": Value::Null,
        });
        let before = value.clone();

        normalize_integral_floats(&mut value);

        assert_eq!(value, before);
    }

    #[test]
    fn normalized_payload_deserializes_into_an_integer_schema() {
        #[derive(Deserialize)]
        struct Probe {
            hp: i32,
            spd: i32,
        }

        // Exactly what Godot hands the bridge: every number a float.
        let godot_payload = r#"{"hp": 95.0, "spd": 12.0}"#;

        assert!(
            serde_json::from_str::<Probe>(godot_payload).is_err(),
            "guard: without normalization this must fail, or the test proves nothing"
        );

        let normalized = normalize_json_str(godot_payload).expect("valid json");
        let probe: Probe = serde_json::from_str(&normalized).expect("normalized json");

        assert_eq!(probe.hp, 95);
        assert_eq!(probe.spd, 12);
    }

    #[test]
    fn invalid_json_is_reported_not_swallowed() {
        assert!(normalize_json_str("{ not json").is_err());
    }
}
