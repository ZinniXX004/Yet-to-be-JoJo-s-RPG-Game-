//! The `balance` binary as CI runs it: its exit code is the gate.
//!
//! `balance_bounds.rs` checks the shipped bands through the library. Nothing
//! checked the binary's own verdict, so a `main` that always exited 0 would
//! have kept CI's `balance` job green over an encounter outside its band. These
//! tests run the built binary on a two-seed matchup file written for the test,
//! which keeps them to a few milliseconds each; the shipped content is still
//! compiled in and loaded, only the matchup list is replaced.

use std::path::PathBuf;
use std::process::{Command, Output};

/// Jotaro against the thug on two fixed seeds. The shipped fight is won on
/// every one of its 300 seeds, so these two are wins as well.
fn matchups_file(name: &str, bands: &[(&str, i32, i32)]) -> PathBuf {
    let entries: Vec<String> = bands
        .iter()
        .map(|(id, min, max)| {
            format!(
                r#"{{"id": "{id}", "party": ["pc.jotaro"], "foes": ["npc.thug"], "seeds": [1, 2], "band": {{"min_percent": {min}, "max_percent": {max}}}, "max_turns": 200}}"#
            )
        })
        .collect();
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, format!("[{}]", entries.join(", ")))
        .expect("the test can write its matchup file");
    path
}

fn balance(args: &[&str], matchups: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_balance"))
        .arg("--matchups")
        .arg(matchups)
        .args(args)
        .output()
        .expect("the balance binary runs")
}

#[test]
fn an_encounter_inside_its_band_exits_zero() {
    let file = matchups_file("cli_in_band.json", &[("matchup.cli", 0, 100)]);
    let run = balance(&[], &file);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
}

#[test]
fn an_encounter_outside_its_band_exits_one_unless_told_not_to_judge() {
    let file = matchups_file("cli_out_of_band.json", &[("matchup.cli", 0, 50)]);

    let gate = balance(&[], &file);
    assert_eq!(gate.status.code(), Some(1), "{gate:?}");

    let json = balance(&["--json"], &file);
    assert_eq!(json.status.code(), Some(1), "{json:?}");
    let report: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("--json prints a JSON report");
    assert_eq!(report["matchups"][0]["win_rate_percent"], 100);

    let survey = balance(&["--no-fail"], &file);
    assert_eq!(survey.status.code(), Some(0), "{survey:?}");
}

#[test]
fn only_runs_the_named_matchup_and_refuses_an_unknown_one() {
    let file = matchups_file(
        "cli_only.json",
        &[("matchup.first", 0, 100), ("matchup.second", 0, 100)],
    );

    let one = balance(&["--json", "--only", "matchup.second"], &file);
    assert_eq!(one.status.code(), Some(0), "{one:?}");
    let report: serde_json::Value =
        serde_json::from_slice(&one.stdout).expect("--json prints a JSON report");
    let ids: Vec<&str> = report["matchups"]
        .as_array()
        .expect("a list of matchups")
        .iter()
        .filter_map(|matchup| matchup["id"].as_str())
        .collect();
    assert_eq!(ids, vec!["matchup.second"]);

    let unknown = balance(&["--only", "matchup.third"], &file);
    assert_eq!(unknown.status.code(), Some(1), "{unknown:?}");
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("no matchup with id 'matchup.third'"));
}
