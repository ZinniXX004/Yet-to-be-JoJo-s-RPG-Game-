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

/// How many times one combatant used one skill across a whole batch.
///
/// A list rather than a map, and ordered by first use rather than by name: the
/// JSON artefact and the printed table must be byte-identical between two runs
/// and two machines, and hash iteration order is neither.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillUse {
    pub skill: Id,
    pub uses: u32,
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
    /// `actions`, broken down by which skill produced them, in first-use order.
    ///
    /// Exists because no other column can answer *what a combatant did*. Damage
    /// dealt, rolls and SP spent are all consistent with several completely
    /// different repertoires, so every behavioural claim about the AI was an
    /// inference until this field existed -- and the two most recent inferences
    /// were both wrong.
    ///
    /// The sum over skills must equal `actions`; see
    /// [`MatchupReport::unattributed_combatants`].
    #[serde(default)]
    pub skill_uses: Vec<SkillUse>,
    pub damage_dealt: i64,
    pub damage_received: i64,
    /// Part of `damage_received` that came from statuses rather than attacks.
    pub status_damage_received: i64,
    pub sp_spent: i64,
    /// HP this combatant restored, to allies or to itself, through actions it
    /// took.
    ///
    /// Credited to the healer, which makes it the mirror of `damage_dealt`
    /// rather than of `damage_received`. Without it every column in the report
    /// measures harm, and a combatant whose entire job is keeping someone else
    /// standing is indistinguishable from one the AI never lets act --
    /// [`MatchupReport::inert_combatants`] would flag a working healer as a
    /// targeting bug.
    ///
    /// A self-heal counts once. `blood_drain` emits a single
    /// [`crate::event::Event::Healed`] naming the drainer as both healer and
    /// healed, and this field follows the healer, so there is no double count
    /// to avoid.
    ///
    /// Regeneration is deliberately absent: it arrives as
    /// [`crate::event::Event::StatusHealed`], which has no actor, so there is
    /// nobody to credit. That recovery is real but it is not anyone's
    /// contribution.
    ///
    /// `serde(default)` because reports written before `0.4.0` do not carry it.
    #[serde(default)]
    pub healing_done: i64,
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
            skill_uses: Vec::new(),
            damage_dealt: 0,
            damage_received: 0,
            status_damage_received: 0,
            sp_spent: 0,
            healing_done: 0,
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

    /// Healing done per battle, rounded half up, on the same basis as
    /// [`Self::damage_dealt_per_battle`] so the two columns can be read against
    /// each other.
    pub fn healing_done_per_battle(&self) -> i64 {
        divide_rounded(self.healing_done, i64::from(self.battles))
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

    /// Credits one action to one skill id.
    ///
    /// Called once per [`crate::event::Event::ActionUsed`], from the same arm
    /// that increments `actions`, so the two can only disagree if the event
    /// stream itself does.
    pub fn record_skill_use(&mut self, skill: &str) {
        if let Some(entry) = self
            .skill_uses
            .iter_mut()
            .find(|entry| entry.skill == skill)
        {
            entry.uses += 1;
            return;
        }
        self.skill_uses.push(SkillUse {
            skill: skill.to_string(),
            uses: 1,
        });
    }

    /// Actions accounted for by the breakdown. Equals `actions` unless the
    /// accounting drifted.
    pub fn counted_skill_uses(&self) -> u32 {
        self.skill_uses.iter().map(|entry| entry.uses).sum()
    }

    /// The breakdown, most used first, ties broken by skill id.
    ///
    /// Sorted for reading; the stored order stays first-use so the serialized
    /// artefact does not reshuffle when a count changes by one.
    pub fn skill_uses_ranked(&self) -> Vec<&SkillUse> {
        let mut ranked: Vec<&SkillUse> = self.skill_uses.iter().collect();
        ranked.sort_by(|a, b| b.uses.cmp(&a.uses).then_with(|| a.skill.cmp(&b.skill)));
        ranked
    }

    /// Share of this combatant's own actions spent on one skill, in whole
    /// percent. Relative to `actions`, so the shares of all its skills sum to
    /// 100 whenever the accounting is intact.
    pub fn skill_use_percent(&self, skill: &str) -> i32 {
        let uses = self
            .skill_uses
            .iter()
            .find(|entry| entry.skill == skill)
            .map(|entry| entry.uses)
            .unwrap_or(0);
        percent(uses, self.actions)
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

    /// Combatants that contributed nothing across every battle -- no damage and
    /// no healing. Almost always a targeting bug or an unreachable enemy, not a
    /// design choice.
    ///
    /// Healing is part of the test because otherwise a working support
    /// character reports as inert. Before `healing_done` existed there was no
    /// number that could tell the two apart, so the check could only have been
    /// right by accident.
    pub fn inert_combatants(&self) -> Vec<&CombatantStats> {
        self.combatants
            .iter()
            .filter(|stats| stats.damage_dealt == 0 && stats.healing_done == 0)
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

    /// Combatants whose per-skill breakdown does not sum to their action count.
    ///
    /// Exact equality, not a tolerance: one action emits exactly one
    /// `ActionUsed`, so any difference means the breakdown is describing a
    /// different set of turns than the totals are. A behavioural claim read off
    /// a breakdown that does not add up is worth less than no claim at all.
    pub fn unattributed_combatants(&self) -> Vec<&CombatantStats> {
        self.combatants
            .iter()
            .filter(|stats| stats.counted_skill_uses() != stats.actions)
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

    fn report(combatants: Vec<CombatantStats>) -> MatchupReport {
        MatchupReport {
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
            combatants,
        }
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

        assert_eq!(report(vec![broken]).miscounted_combatants().len(), 1);
    }

    /// Healing averages on the same basis as damage, so the two columns are
    /// comparable at a glance.
    #[test]
    fn healing_is_averaged_per_battle_and_rounds_half_up() {
        let mut healer = stats();
        healer.battles = 4;
        healer.healing_done = 250;
        assert_eq!(healer.healing_done_per_battle(), 63);

        let unplayed = stats();
        assert_eq!(
            unplayed.healing_done_per_battle(),
            0,
            "a combatant with no battles must not divide by zero"
        );
    }

    /// The reason the field exists. A support character deals nothing and is
    /// still working; before healing was counted, the inert check could not
    /// tell it apart from a combatant the AI never lets act.
    #[test]
    fn a_healer_that_deals_no_damage_is_not_reported_as_inert() {
        let mut healer = stats();
        healer.battles = 2;
        healer.healing_done = 400;

        let mut bystander = stats();
        bystander.battles = 2;

        let report = report(vec![healer, bystander]);
        let inert = report.inert_combatants();
        assert_eq!(
            inert.len(),
            1,
            "only the combatant that contributed nothing at all is inert"
        );
        assert_eq!(inert[0].healing_done, 0);
    }

    /// The breakdown must be readable as "what did this combatant spend its
    /// turns on", which means shares of its own actions and a stable order.
    #[test]
    fn a_breakdown_ranks_by_use_and_reports_shares_of_the_actors_own_actions() {
        let mut jotaro = stats();
        for skill in ["skill.rush_barrage", "skill.strike", "skill.rush_barrage"] {
            jotaro.actions += 1;
            jotaro.record_skill_use(skill);
        }
        jotaro.actions += 1;
        jotaro.record_skill_use("skill.tempo_halt");

        let ranked = jotaro.skill_uses_ranked();
        assert_eq!(ranked[0].skill, "skill.rush_barrage");
        assert_eq!(ranked[0].uses, 2);
        // A tie on uses resolves by id, so the order cannot depend on which
        // seed happened to run first.
        assert_eq!(ranked[1].skill, "skill.strike");
        assert_eq!(ranked[2].skill, "skill.tempo_halt");

        assert_eq!(jotaro.counted_skill_uses(), 4);
        assert_eq!(jotaro.skill_use_percent("skill.rush_barrage"), 50);
        assert_eq!(jotaro.skill_use_percent("skill.strike"), 25);
        assert_eq!(
            jotaro.skill_use_percent("skill.guard_stance"),
            0,
            "a skill the combatant never used is 0%, not an error"
        );
    }

    /// A breakdown that does not sum to the action count describes a different
    /// set of turns than the totals do, so it must be reported rather than
    /// read.
    #[test]
    fn a_breakdown_that_does_not_add_up_is_an_accounting_defect() {
        let mut honest = stats();
        honest.actions = 2;
        honest.record_skill_use("skill.strike");
        honest.record_skill_use("skill.strike");

        let mut drifted = stats();
        drifted.actions = 9;
        drifted.record_skill_use("skill.strike");

        let report = report(vec![honest, drifted]);
        let unattributed = report.unattributed_combatants();
        assert_eq!(unattributed.len(), 1);
        assert_eq!(unattributed[0].actions, 9);
    }

    /// A report written before `attack_rolls`, `skill_uses` and `healing_done`
    /// existed must still load.
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
        assert_eq!(
            stats.healing_done, 0,
            "an older report has no healing column, and must default rather than fail"
        );
        assert_eq!(stats.healing_done_per_battle(), 0);
        assert!(
            stats.skill_uses.is_empty(),
            "an older report has no breakdown, and must not invent one"
        );
        // And it must be reported as unattributed rather than quietly read as
        // "this combatant used no skills".
        assert_eq!(report(vec![stats]).unattributed_combatants().len(), 1);
    }
}
