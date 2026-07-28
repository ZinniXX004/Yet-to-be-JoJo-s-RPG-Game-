//! Headless batch simulation -- the balance harness, as library code.
//!
//! A single battle is already deterministic; this module turns that into
//! evidence. A [`Matchup`] names two sides and a fixed list of seeds, and
//! [`run_batch`] plays every one of them to completion with no player and no
//! engine, producing a [`BatchReport`].
//!
//! Two rules keep this honest:
//!
//! 1. **No I/O.** Reading files, printing tables and choosing exit codes belong
//!    to `src/bin/balance.rs`. Statistics are a property of the simulation, so
//!    they live in the library where they can be unit-tested.
//! 2. **No rules.** The harness never knows what a skill does. If it ever needs
//!    to, the logic is in the wrong layer.
//!
//! The seed list is fixed rather than sampled. Comparability between two commits
//! matters more than statistical purity: a win rate that moves must move because
//! the content changed, not because the sample did. Widen the list deliberately,
//! in a commit that says so.

use serde::{Deserialize, Serialize};

use crate::battle::{Battle, BattleOutcome};
use crate::data::{DataError, Database, Id};
use crate::event::Event;
use crate::report::{percent, BatchReport, CombatantStats, MatchupReport, WinRateBand};

/// Turn budget per battle. Generous: the point of the bound is to turn a
/// scheduler stall into a reported stalemate, not to cut fights short.
fn default_max_turns() -> u64 {
    500
}

/// One declared encounter: who fights, on which seeds, and what win rate is
/// considered acceptable.
///
/// This is content, not code. Adding an encounter must never require a
/// recompile, so the whole struct is deserialized from `data/matchups.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matchup {
    pub id: Id,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub party: Vec<Id>,
    pub foes: Vec<Id>,
    pub seeds: Vec<u64>,
    pub band: WinRateBand,
    #[serde(default = "default_max_turns")]
    pub max_turns: u64,
}

impl Matchup {
    /// Everything wrong with this declaration, as human-readable lines.
    ///
    /// Reported rather than returned as a single error so a content author sees
    /// all the problems at once instead of fixing them one run at a time.
    /// Combatant ids are checked against `db`, so a typo fails here rather than
    /// as a confusing failure deep inside a battle.
    pub fn issues(&self, db: &Database) -> Vec<String> {
        let mut issues = Vec::new();
        let label = &self.id;

        if self.party.is_empty() {
            issues.push(format!("matchup '{label}' has an empty party"));
        }
        if self.foes.is_empty() {
            issues.push(format!("matchup '{label}' has no foes"));
        }
        if self.seeds.is_empty() {
            issues.push(format!(
                "matchup '{label}' has no seeds, so it would report a win rate over zero battles"
            ));
        }
        if self.max_turns == 0 {
            issues.push(format!("matchup '{label}' has a max_turns of 0"));
        }
        for id in self.party.iter().chain(self.foes.iter()) {
            if db.combatant(id).is_none() {
                issues.push(format!("matchup '{label}' references unknown combatant '{id}'"));
            }
        }
        for issue in self.band.issues() {
            issues.push(format!("matchup '{label}' band: {issue}"));
        }

        let mut seen: Vec<&Id> = Vec::new();
        for seed in &self.seeds {
            let duplicate = self.seeds.iter().filter(|other| *other == seed).count() > 1;
            if duplicate && !seen.iter().any(|_| false) {
                // A repeated seed is a silently doubled data point, which skews
                // the win rate without changing the battle count in any visible
                // way.
                issues.push(format!("matchup '{label}' repeats seed {seed}"));
                break;
            }
            seen.clear();
        }

        issues
    }

    pub fn label(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }
}

/// Parses `data/matchups.json`.
pub fn parse_matchups(json: &str) -> Result<Vec<Matchup>, DataError> {
    serde_json::from_str(json).map_err(|e| DataError::Parse(format!("matchups: {e}")))
}

/// Runs every seed of one matchup and aggregates the result.
pub fn run_matchup(db: &Database, matchup: &Matchup) -> Result<MatchupReport, DataError> {
    let issues = matchup.issues(db);
    if !issues.is_empty() {
        return Err(DataError::Invalid(issues));
    }

    let mut wins = 0_u32;
    let mut losses = 0_u32;
    let mut stalemates = 0_u32;
    let mut turns: Vec<u64> = Vec::with_capacity(matchup.seeds.len());
    let mut combatants: Vec<CombatantStats> = Vec::new();

    for &seed in &matchup.seeds {
        // `Battle` owns its database, and a batch must not let one battle
        // observe another's state, so each run gets its own clone.
        let mut battle = Battle::new(db.clone(), &matchup.party, &matchup.foes, seed)?;

        if combatants.is_empty() {
            combatants = battle
                .state()
                .combatants
                .iter()
                .map(|combatant| {
                    CombatantStats::new(&combatant.id, &combatant.name, combatant.team)
                })
                .collect();
        }

        match battle.run_to_completion(matchup.max_turns) {
            BattleOutcome::PartyWins => wins += 1,
            BattleOutcome::PartyWipes => losses += 1,
            BattleOutcome::Stalemate => stalemates += 1,
        }
        turns.push(battle.state().turn);

        let events = battle.take_events();
        accumulate(&mut combatants, &events);

        for (index, combatant) in battle.state().combatants.iter().enumerate() {
            if let Some(stats) = combatants.get_mut(index) {
                stats.battles += 1;
                if combatant.alive() {
                    stats.survived += 1;
                }
            }
        }
    }

    let battles = matchup.seeds.len() as u32;
    turns.sort_unstable();

    Ok(MatchupReport {
        id: matchup.id.clone(),
        name: matchup.label().to_string(),
        battles,
        wins,
        losses,
        stalemates,
        win_rate_percent: percent(wins, battles),
        median_turns: median(&turns),
        shortest_turns: turns.first().copied().unwrap_or(0),
        longest_turns: turns.last().copied().unwrap_or(0),
        band: matchup.band,
        combatants,
    })
}

/// Runs every matchup in order.
pub fn run_batch(db: &Database, matchups: &[Matchup]) -> Result<BatchReport, DataError> {
    let mut reports = Vec::with_capacity(matchups.len());
    for matchup in matchups {
        reports.push(run_matchup(db, matchup)?);
    }
    Ok(BatchReport { matchups: reports })
}

/// Attributes each event to the combatants involved.
///
/// Status damage is counted into `damage_received` *and* kept separately. A
/// character dying to bleed and a character dying to a boss are the same number
/// in an aggregate and completely different balance problems.
fn accumulate(stats: &mut [CombatantStats], events: &[Event]) {
    for event in events {
        match event {
            Event::Damaged {
                actor,
                target,
                amount,
                ..
            } => {
                if let Some(entry) = stats.get_mut(*actor) {
                    entry.damage_dealt += i64::from(*amount);
                }
                if let Some(entry) = stats.get_mut(*target) {
                    entry.damage_received += i64::from(*amount);
                }
            }
            Event::StatusDamaged { target, amount, .. } => {
                if let Some(entry) = stats.get_mut(*target) {
                    entry.damage_received += i64::from(*amount);
                    entry.status_damage_received += i64::from(*amount);
                }
            }
            Event::SpConsumed { actor, amount } => {
                if let Some(entry) = stats.get_mut(*actor) {
                    entry.sp_spent += i64::from(*amount);
                }
            }
            Event::ActionUsed { actor, .. } => {
                if let Some(entry) = stats.get_mut(*actor) {
                    entry.actions += 1;
                }
            }
            Event::Missed { actor, .. } => {
                if let Some(entry) = stats.get_mut(*actor) {
                    entry.misses += 1;
                }
            }
            Event::Downed { target } => {
                if let Some(entry) = stats.get_mut(*target) {
                    entry.times_downed += 1;
                }
            }
            _ => {}
        }
    }
}

/// Lower median of an already sorted slice.
///
/// Lower rather than the mean of the two middle values, because averaging would
/// introduce a half-turn that no battle ever took, and because integer division
/// of an even-length sample is exactly the kind of detail that quietly differs
/// between implementations.
fn median(sorted: &[u64]) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[(sorted.len() - 1) / 2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AiProfile, CombatantDef, Effect, Element, SkillDef, Stats, TargetKind, Team};
    use crate::state::TEMPO_THRESHOLD;

    const STRIKE: &str = "skill.strike";

    fn skill() -> SkillDef {
        SkillDef {
            id: STRIKE.to_string(),
            name: "Strike".to_string(),
            description: String::new(),
            sp_cost: 0,
            target: TargetKind::OneEnemy,
            accuracy: 100,
            tempo_cost: TEMPO_THRESHOLD,
            effects: vec![Effect::Damage {
                power: 100,
                element: Element::Physical,
                variance: 0,
            }],
        }
    }

    fn combatant(id: &str, team: Team, hp: i32, atk: i32, spd: i32) -> CombatantDef {
        CombatantDef {
            id: id.to_string(),
            name: id.to_string(),
            team,
            stats: Stats {
                hp,
                sp: 50,
                atk,
                def: 10,
                spd,
                will: 10,
            },
            stand: None,
            skills: vec![STRIKE.to_string()],
            ai: AiProfile::Aggressive,
        }
    }

    fn db() -> Database {
        Database::new(
            vec![],
            vec![skill()],
            vec![
                combatant("hero", Team::Party, 400, 100, 120),
                combatant("weakling", Team::Foe, 40, 5, 30),
                combatant("titan", Team::Foe, 4000, 400, 200),
            ],
        )
        .expect("fixture content must be valid")
    }

    fn matchup(id: &str, foe: &str, band: WinRateBand) -> Matchup {
        Matchup {
            id: id.to_string(),
            name: String::new(),
            description: String::new(),
            party: vec!["hero".to_string()],
            foes: vec![foe.to_string()],
            seeds: vec![1, 2, 3, 4],
            band,
            max_turns: 500,
        }
    }

    fn wide_band() -> WinRateBand {
        WinRateBand {
            min_percent: 0,
            max_percent: 100,
        }
    }

    #[test]
    fn a_hopeless_matchup_reports_a_zero_win_rate() {
        let report = run_matchup(&db(), &matchup("m.titan", "titan", wide_band()))
            .expect("a valid matchup must run");
        assert_eq!(report.battles, 4);
        assert_eq!(report.win_rate_percent, 0);
        assert_eq!(report.stalemates, 0);
    }

    #[test]
    fn a_trivial_matchup_reports_a_full_win_rate() {
        let report = run_matchup(&db(), &matchup("m.weak", "weakling", wide_band()))
            .expect("a valid matchup must run");
        assert_eq!(report.win_rate_percent, 100);
        assert!(report.within_band());
    }

    /// The whole purpose of the harness: a guaranteed outcome must be visible as
    /// a band violation rather than as a passing test.
    #[test]
    fn a_guaranteed_outcome_falls_outside_a_meaningful_band() {
        let band = WinRateBand {
            min_percent: 20,
            max_percent: 80,
        };
        let report =
            run_matchup(&db(), &matchup("m.titan", "titan", band)).expect("matchup must run");
        assert!(!report.within_band());
        assert!(!report.is_healthy());
    }

    #[test]
    fn the_same_seed_list_produces_the_same_report_twice() {
        let db = db();
        let matchup = matchup("m.weak", "weakling", wide_band());
        let first = run_matchup(&db, &matchup).expect("matchup must run");
        let second = run_matchup(&db, &matchup).expect("matchup must run");
        assert_eq!(
            first, second,
            "a batch over a fixed seed list must be reproducible"
        );
    }

    #[test]
    fn damage_is_attributed_to_both_sides_of_every_hit() {
        let report = run_matchup(&db(), &matchup("m.titan", "titan", wide_band()))
            .expect("matchup must run");
        let hero = &report.combatants[0];
        let titan = &report.combatants[1];
        assert!(hero.damage_dealt > 0, "the hero attacked and must be credited");
        assert_eq!(
            hero.damage_dealt, titan.damage_received,
            "every point dealt must land on someone"
        );
        assert_eq!(hero.times_downed, 1, "the hero cannot survive this matchup");
        assert_eq!(hero.battles, 4);
    }

    #[test]
    fn an_unknown_combatant_id_is_reported_before_any_battle_runs() {
        let mut broken = matchup("m.typo", "weakling", wide_band());
        broken.foes = vec!["npc.does_not_exist".to_string()];
        let error = run_matchup(&db(), &broken).expect_err("an unknown id must not run");
        match error {
            DataError::Invalid(issues) => {
                assert!(issues.iter().any(|issue| issue.contains("npc.does_not_exist")));
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_seed_list_is_rejected_rather_than_reported_as_zero_percent() {
        let mut broken = matchup("m.empty", "weakling", wide_band());
        broken.seeds = Vec::new();
        assert!(run_matchup(&db(), &broken).is_err());
    }

    #[test]
    fn the_median_is_the_lower_of_the_two_middle_values() {
        assert_eq!(median(&[]), 0);
        assert_eq!(median(&[5]), 5);
        assert_eq!(median(&[4, 8]), 4);
        assert_eq!(median(&[1, 4, 9]), 4);
    }
}
