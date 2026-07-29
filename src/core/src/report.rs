//! Aggregated results of a headless batch run.
//!
//! These types are plain data: they hold numbers and know how to judge
//! themselves against a declared band, and nothing else. Formatting a table,
//! writing a file and choosing an exit code all belong to the binary, so the
//! same report can be asserted on in a test, serialized as a CI artefact, or
//! rendered by a future frontend without any of them agreeing on presentation.
//!
//! Every figure is an integer. Percentages are whole percent, rounded half up,
//! because `rpg-core` has no float math anywhere and a balance number that
//! differs in the last decimal between two machines would defeat the point of
//! the harness.

use serde::{Deserialize, Serialize};

use crate::data::{Id, Team};

/// Acceptable win-rate interval for a matchup, in whole percent, inclusive at
/// both ends.
///
/// A band rather than a target on purpose. Asserting an exact win rate makes the
/// test fragile and the encounter boring; what actually matters is that the
/// fight is neither a guaranteed win nor a guaranteed loss, and that is an
/// interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WinRateBand {
    pub min_percent: i32,
    pub max_percent: i32,
}

impl WinRateBand {
    pub fn contains(&self, percent: i32) -> bool {
        percent >= self.min_percent && percent <= self.max_percent
    }

    /// Structural problems with the band itself, as opposed to a matchup that
    /// misses it. Reported by the content validator so a typo fails loudly
    /// instead of silently accepting every result.
    pub fn issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if !(0..=100).contains(&self.min_percent) {
            issues.push(format!(
                "min_percent {} is outside 0..=100",
                self.min_percent
            ));
        }
        if !(0..=100).contains(&self.max_percent) {
            issues.push(format!(
                "max_percent {} is outside 0..=100",
                self.max_percent
            ));
        }
        if self.min_percent > self.max_percent {
            issues.push(format!(
                "min_percent {} is above max_percent {}",
                self.min_percent, self.max_percent
            ));
        }
        issues
    }
}

/// Per-combatant totals accumulated across every battle in a matchup.
///
/// Aggregate win rate hides the two failure modes that matter most: a party
/// member who absorbs everything, and an enemy the AI never gets around to
/// attacking. Both are visible here and nowhere else.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatantStats {
    pub id: Id,
    pub name: String,
    pub team: Team,
    pub battles: u32,
    pub survived: u32,
    pub times_downed: u32,
    /// Skills used. One per turn spent acting, regardless of how many
    /// combatants the skill touched.
    pub actions: u32,
    /// Accuracy checks made. One per *target*, so a single area skill against
    /// three foes contributes three.
    ///
    /// Separate from `actions` because these two numbers answer different
    /// questions -- "how often did this combatant get to act" versus "how often
    /// did it try to hit something" -- and conflating them is what made
    /// `miss_percent` wrong before `0.4.0`.
    #[serde(default)]
    pub attack_rolls: u32,
    /// Accuracy checks that failed. Always a subset of `attack_rolls`.
    pub misses: u32,
    pub damage_dealt: i64,
    pub damage_received: i64,
    /// Part of `damage_received` that came from statuses rather than attacks.
    pub status_damage_received: i64,
    pub sp_spent: i64,
}

impl CombatantStats {
    pub fn new(id: &str, name: &str, team: Team) -> Self {
        CombatantStats {
            id: id.to_string(),
            name: name.to_string(),
            team,
            battles: 0,
            survived: 0,
            times_downed: 0,
            actions: 0,
            attack_rolls: 0,
            misses: 0,
            damage_dealt: 0,
            damage_received: 0,
            status_damage_received: 0,
            sp_spent: 0,
        }
    }

    /// Damage dealt per battle, rounded half up. Zero here for a combatant that
    /// is supposed to be a threat means the AI never targeted it or it never
    /// got a turn.
    pub fn damage_dealt_per_battle(&self) -> i64 {
        divide_rounded(self.damage_dealt, i64::from(self.battles))
    }

    pub fn damage_received_per_battle(&self) -> i64 {
        divide_rounded(self.damage_received, i64::from(self.battles))
    }

    pub fn survival_percent(&self) -> i32 {
        percent(self.survived, self.battles)
    }

    /// Share of accuracy checks that failed, in whole percent.
    ///
    /// Divided by `attack_rolls` rather than by `actions`, and it must stay that
    /// way: an area skill rolls once per target, so dividing by actions cannot
    /// be bounded by 100 and stops describing anything once the AI can reach
    /// `all_enemies` skills.
    ///
    /// A combatant that never attacked -- a healer, or an enemy that only ever
    /// buffs -- reports 0% rather than dividing by zero. That is a deliberate
    /// reading of "missed nothing", and it is why the column is worth reading
    /// next to `dealt/b` instead of on its own.
    pub fn miss_percent(&self) -> i32 {
        percent(self.misses, self.attack_rolls)
    }
}

/// Result of running one matchup over its whole seed list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchupReport {
    pub id: Id,
    pub name: String,
    pub battles: u32,
    pub wins: u32,
    pub losses: u32,
    pub stalemates: u32,
    pub win_rate_percent: i32,
    pub median_turns: u64,
    pub shortest_turns: u64,
    pub longest_turns: u64,
    pub band: WinRateBand,
    pub combatants: Vec<CombatantStats>,
}

impl MatchupReport {
    pub fn within_band(&self) -> bool {
        self.band.contains(self.win_rate_percent)
    }

    /// A stalemate is never acceptable: it means the scheduler ran out of turns
    /// with both sides standing, which is a content or rules defect rather than
    /// a balance opinion.
    pub fn is_healthy(&self) -> bool {
        self.within_band() && self.stalemates == 0
    }

    /// Combatants that dealt no damage at all across every battle. Almost always
    /// a targeting bug or an unreachable enemy, not a design choice.
    pub fn inert_combatants(&self) -> Vec<&CombatantStats> {
        self.combatants
            .iter()
            .filter(|stats| stats.damage_dealt == 0)
            .collect()
    }

    /// Combatants whose miss count exceeds the rolls they made, which is
    /// arithmetically impossible and therefore an accounting defect rather than
    /// a balance finding.
    ///
    /// Cheap to check and worth checking: the accounting this method guards was
    /// wrong for two releases without producing a single visibly wrong table,
    /// because no reachable skill hit more than one target.
    pub fn miscounted_combatants(&self) -> Vec<&CombatantStats> {
        self.combatants
            .iter()
            .filter(|stats| stats.misses > stats.attack_rolls)
            .collect()
    }
}

/// Every matchup in one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchReport {
    pub matchups: Vec<MatchupReport>,
}

impl BatchReport {
    pub fn all_healthy(&self) -> bool {
        self.matchups.iter().all(MatchupReport::is_healthy)
    }

    pub fn unhealthy(&self) -> Vec<&MatchupReport> {
        self.matchups
            .iter()
            .filter(|report| !report.is_healthy())
            .collect()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Whole percent of `part` in `total`, rounded half up. Returns 0 for an empty
/// total rather than panicking: an empty run is a reportable fact, not a crash.
pub fn percent(part: u32, total: u32) -> i32 {
    if total == 0 {
        return 0;
    }
    let part = i64::from(part);
    let total = i64::from(total);
    ((part * 100 + total / 2) / total) as i32
}

fn divide_rounded(value: i64, divisor: i64) -> i64 {
    if divisor == 0 {
        return 0;
    }
    (value + divisor / 2) / divisor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats() -> CombatantStats {
        CombatantStats::new("pc.test", "Test", Team::Party)
    }

    #[test]
    fn percentages_round_half_up_and_survive_an_empty_run() {
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(2, 3), 67);
        assert_eq!(percent(1, 2), 50);
        assert_eq!(percent(0, 0), 0, "an empty run must not divide by zero");
    }

    #[test]
    fn a_band_is_inclusive_at_both_ends() {
        let band = WinRateBand {
            min_percent: 20,
            max_percent: 80,
        };
        assert!(band.contains(20));
        assert!(band.contains(80));
        assert!(!band.contains(19));
        assert!(!band.contains(81));
    }

    #[test]
    fn a_reversed_or_out_of_range_band_reports_itself_as_broken() {
        let reversed = WinRateBand {
            min_percent: 90,
            max_percent: 10,
        };
        assert!(!reversed.issues().is_empty());

        let impossible = WinRateBand {
            min_percent: -5,
            max_percent: 140,
        };
        assert_eq!(impossible.issues().len(), 2);

        let sane = WinRateBand {
            min_percent: 0,
            max_percent: 100,
        };
        assert!(sane.issues().is_empty());
    }

    /// The defect this field exists to fix: one area skill, three targets, two
    /// of them missed. Dividing by `actions` would report 200%.
    #[test]
    fn a_miss_rate_is_a_share_of_rolls_not_of_actions() {
        let mut area = stats();
        area.actions = 1;
        area.attack_rolls = 3;
        area.misses = 2;
        assert_eq!(area.miss_percent(), 67);

        let mut single = stats();
        single.actions = 4;
        single.attack_rolls = 4;
        single.misses = 1;
        assert_eq!(
            single.miss_percent(),
            25,
            "single-target accounting must be unchanged"
        );
    }

    #[test]
    fn a_combatant_that_never_attacked_reports_no_miss_rate() {
        let mut healer = stats();
        healer.actions = 12;
        assert_eq!(healer.attack_rolls, 0);
        assert_eq!(
            healer.miss_percent(),
            0,
            "a support character must not divide by zero rolls"
        );
    }

    #[test]
    fn more_misses_than_rolls_is_reported_as_an_accounting_defect() {
        let mut broken = stats();
        broken.attack_rolls = 2;
        broken.misses = 5;

        let report = MatchupReport {
            id: "matchup.test".to_string(),
            name: "Test".to_string(),
            battles: 1,
            wins: 1,
            losses: 0,
            stalemates: 0,
            win_rate_percent: 100,
            median_turns: 3,
            shortest_turns: 3,
            longest_turns: 3,
            band: WinRateBand {
                min_percent: 0,
                max_percent: 100,
            },
            combatants: vec![broken],
        };

        assert_eq!(report.miscounted_combatants().len(), 1);
    }

    /// A report written before `attack_rolls` existed must still load.
    #[test]
    fn a_report_from_an_older_release_still_deserializes() {
        let json = r#"{
            "id": "pc.jotaro",
            "name": "Jotaro",
            "team": "party",
            "battles": 12,
            "survived": 8,
            "times_downed": 4,
            "actions": 70,
            "misses": 6,
            "damage_dealt": 985,
            "damage_received": 339,
            "status_damage_received": 0,
            "sp_spent": 120
        }"#;

        let stats: CombatantStats =
            serde_json::from_str(json).expect("a 0.3.0 report must still load");
        assert_eq!(stats.actions, 70);
        assert_eq!(stats.attack_rolls, 0);
        assert_eq!(stats.miss_percent(), 0);
    }
}
