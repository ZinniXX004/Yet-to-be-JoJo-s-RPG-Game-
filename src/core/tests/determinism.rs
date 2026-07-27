//! The load-bearing guarantee of this project: identical seed plus identical
//! decisions produce an identical event log.
//!
//! This test runs against the real shipped content in `data/`, not fixtures, so
//! it also fails if that content stops loading or stops producing a decisive
//! fight. That double duty is deliberate: a content typo should break CI here.

use rpg_core::{Battle, BattleOutcome, Database, Id};

const STANDS: &str = include_str!("../../../data/stands.json");
const SKILLS: &str = include_str!("../../../data/skills.json");
const COMBATANTS: &str = include_str!("../../../data/combatants.json");

fn ids(values: &[&str]) -> Vec<Id> {
    values.iter().map(|value| value.to_string()).collect()
}

fn run(seed: u64) -> (BattleOutcome, Vec<String>) {
    let db = Database::from_json(STANDS, SKILLS, COMBATANTS)
        .expect("shipped content in data/ must load and validate");
    let party = ids(&["pc.jotaro", "pc.josuke", "pc.kakyoin"]);
    let foes = ids(&["npc.dio", "npc.flame_assassin"]);

    let mut battle = Battle::new(db, &party, &foes, seed).expect("battle must build");
    let outcome = battle.run_to_completion(500);
    // Debug formatting is enough of a fingerprint here and avoids asserting on a
    // serialization format that is still allowed to change.
    let log = battle
        .take_events()
        .iter()
        .map(|event| format!("{event:?}"))
        .collect();
    (outcome, log)
}

#[test]
fn shipped_content_loads() {
    let db = Database::from_json(STANDS, SKILLS, COMBATANTS)
        .expect("shipped content in data/ must load and validate");
    let (stands, skills, combatants) = db.counts();
    assert!(stands > 0, "expected at least one stand");
    assert!(skills > 0, "expected at least one skill");
    assert!(combatants > 0, "expected at least one combatant");
}

#[test]
fn same_seed_produces_identical_logs() {
    let (first_outcome, first_log) = run(7);
    let (second_outcome, second_log) = run(7);

    assert_eq!(first_outcome, second_outcome);
    assert_eq!(
        first_log.len(),
        second_log.len(),
        "event counts diverged between two runs of the same seed"
    );
    for (index, (left, right)) in first_log.iter().zip(second_log.iter()).enumerate() {
        assert_eq!(left, right, "logs diverged at event {index}");
    }
}

#[test]
fn auto_battle_terminates_decisively() {
    // A stalemate here means the scheduler stalled or nothing could deal damage.
    for seed in [1_u64, 7, 99, 12_345] {
        let (outcome, log) = run(seed);
        assert_ne!(
            outcome,
            BattleOutcome::Stalemate,
            "seed {seed} stalled instead of resolving"
        );
        assert!(!log.is_empty(), "seed {seed} produced no events");
    }
}
