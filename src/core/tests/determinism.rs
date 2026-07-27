//! Determinism and termination guarantees.
//!
//! These tests load the real `data/*.json` rather than fixtures on purpose: a
//! content edit that breaks the simulation should fail CI, not ship.
//!
//! A failure in `identical_seed_yields_identical_event_log` is a release
//! blocker, not a flaky test. It means replay, save/load, and bug reproduction
//! are all unreliable.

use rpg_core::{Battle, BattleOutcome, Command, Database, Event, Phase};

const STANDS: &str = include_str!("../../../data/stands.json");
const SKILLS: &str = include_str!("../../../data/skills.json");
const COMBATANTS: &str = include_str!("../../../data/combatants.json");

fn database() -> Database {
    match Database::from_json(STANDS, SKILLS, COMBATANTS) {
        Ok(db) => db,
        Err(error) => panic!("content data must be valid:\n{error}"),
    }
}

fn party() -> Vec<String> {
    vec![
        "pc.jotaro".to_string(),
        "pc.josuke".to_string(),
        "pc.kakyoin".to_string(),
    ]
}

fn foes() -> Vec<String> {
    vec!["npc.dio".to_string(), "npc.flame_assassin".to_string()]
}

fn run(seed: u64) -> (Vec<Event>, BattleOutcome) {
    let mut battle = Battle::new(database(), &party(), &foes(), seed).expect("battle must build");
    let outcome = battle.run_with_ai(4_000);
    (battle.events().to_vec(), outcome)
}

#[test]
fn content_data_loads_and_resolves() {
    let (stands, skills, combatants) = database().counts();
    assert!(stands > 0 && skills > 0 && combatants > 0);
}

#[test]
fn identical_seed_yields_identical_event_log() {
    let (events_a, outcome_a) = run(1337);
    let (events_b, outcome_b) = run(1337);
    assert_eq!(outcome_a, outcome_b);
    assert_eq!(events_a.len(), events_b.len(), "event count diverged");
    assert_eq!(events_a, events_b, "event log diverged for the same seed");
}

#[test]
fn different_seeds_diverge() {
    // Not a strict requirement of determinism, but if this ever passes it means
    // the RNG is not actually influencing resolution.
    let (events_a, _) = run(1);
    let (events_b, _) = run(2);
    assert_ne!(events_a, events_b);
}

#[test]
fn battles_terminate_without_hitting_the_safety_valve() {
    for seed in 0..40u64 {
        let (_, outcome) = run(seed);
        assert_ne!(
            outcome,
            BattleOutcome::Stalemate,
            "seed {seed} failed to resolve, which points at unkillable content"
        );
    }
}

#[test]
fn illegal_command_is_rejected_and_the_turn_is_retained() {
    let mut battle = Battle::new(database(), &party(), &foes(), 42).expect("battle must build");
    match battle.advance() {
        Phase::AwaitingCommand { actor } => {
            let bad = Command::Skill {
                skill: "skill.does_not_exist".to_string(),
                target: 3,
            };
            assert!(battle.submit(&bad).is_err());
            assert_eq!(
                battle.awaiting(),
                Some(actor),
                "a rejected command must not consume the turn"
            );
            assert!(battle.submit(&Command::Wait).is_ok());
        }
        other => panic!("expected a command prompt, got {other:?}"),
    }
}

#[test]
fn a_downed_side_ends_the_battle() {
    let mut battle =
        Battle::new(database(), &party(), &vec!["npc.thug".to_string()], 9).expect("battle builds");
    let outcome = battle.run_with_ai(4_000);
    assert_eq!(outcome, BattleOutcome::PartyWins);
}
