//! The M2 exit criterion, expressed as a test rather than as a paragraph.
//!
//! Every encounter declared in `data/matchups.json` is replayed over its fixed
//! seed list, and its win rate must land inside the band the content author
//! wrote down. A matchup that is a guaranteed win or a guaranteed loss fails
//! here, which is the entire point: an unwinnable boss is a defect, and so is a
//! boss that cannot lose.
//!
//! When this test fails, the correct response is almost never to widen the band.
//! It is to change the numbers in `data/*.json` until the fight is a fight. The
//! band is a statement of design intent; moving it to match the current content
//! turns the test into a thermometer that reports its own reading.

use rpg_core::{parse_matchups, run_batch, run_matchup, Database, Matchup};

const STANDS: &str = include_str!("../../../data/stands.json");
const SKILLS: &str = include_str!("../../../data/skills.json");
const COMBATANTS: &str = include_str!("../../../data/combatants.json");
const MATCHUPS: &str = include_str!("../../../data/matchups.json");

fn database() -> Database {
    Database::from_json(STANDS, SKILLS, COMBATANTS)
        .expect("shipped content in data/ must load and validate")
}

fn matchups() -> Vec<Matchup> {
    parse_matchups(MATCHUPS).expect("data/matchups.json must parse")
}

#[test]
fn every_matchup_declaration_is_well_formed() {
    let db = database();
    let matchups = matchups();
    assert!(
        !matchups.is_empty(),
        "an empty encounter list would make every other test in this file vacuous"
    );

    let mut issues: Vec<String> = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    for matchup in &matchups {
        issues.extend(matchup.issues(&db));
        if ids.contains(&matchup.id.as_str()) {
            issues.push(format!("duplicate matchup id '{}'", matchup.id));
        }
        ids.push(&matchup.id);
    }

    assert!(
        issues.is_empty(),
        "data/matchups.json is malformed:\n  - {}",
        issues.join("\n  - ")
    );
}

#[test]
fn no_encounter_stalls() {
    let db = database();
    for matchup in matchups() {
        let report = run_matchup(&db, &matchup).expect("a declared matchup must run");
        assert_eq!(
            report.stalemates, 0,
            "matchup '{}' hit the turn limit in {} of {} battles, which means the \
             scheduler ran out of turns with both sides standing",
            report.id, report.stalemates, report.battles
        );
    }
}

#[test]
fn no_combatant_sits_out_the_whole_batch() {
    let db = database();
    for matchup in matchups() {
        let report = run_matchup(&db, &matchup).expect("a declared matchup must run");
        let inert: Vec<&str> = report
            .inert_combatants()
            .iter()
            .map(|stats| stats.name.as_str())
            .collect();
        assert!(
            inert.is_empty(),
            "in matchup '{}', these combatants dealt no damage across all {} battles: {}. \
             That is a targeting defect, not a balance opinion",
            report.id,
            report.battles,
            inert.join(", ")
        );
    }
}

#[test]
fn every_encounter_lands_inside_its_declared_win_rate_band() {
    let db = database();
    let report = run_batch(&db, &matchups()).expect("the declared encounters must run");

    let violations: Vec<String> = report
        .matchups
        .iter()
        .filter(|matchup| !matchup.within_band())
        .map(|matchup| {
            format!(
                "'{}': {}% win rate over {} battles, outside the declared band {}..{}",
                matchup.id,
                matchup.win_rate_percent,
                matchup.battles,
                matchup.band.min_percent,
                matchup.band.max_percent
            )
        })
        .collect();

    assert!(
        violations.is_empty(),
        "balance regression:\n  - {}\n\nRun `cargo run -p rpg-core --bin balance` for the \
         per-combatant breakdown before touching any number",
        violations.join("\n  - ")
    );
}

/// A batch is a measurement, and a measurement that changes between two
/// identical runs is not one. This guards the harness itself rather than the
/// content.
#[test]
fn a_batch_is_reproducible() {
    let db = database();
    let matchups = matchups();
    let first = run_batch(&db, &matchups).expect("the declared encounters must run");
    let second = run_batch(&db, &matchups).expect("the declared encounters must run");
    assert_eq!(
        first, second,
        "two runs over the same fixed seed list disagreed, so the simulation is \
         reading state from somewhere outside BattleState"
    );
}
