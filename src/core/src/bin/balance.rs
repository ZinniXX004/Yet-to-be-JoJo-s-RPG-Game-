//! `balance` -- headless balance harness runner.
//!
//! All I/O for the harness lives here and nowhere else. The simulation and the
//! statistics are in `rpg_core::sim` and `rpg_core::report`, which know nothing
//! about files, stdout or exit codes; this binary reads content, calls
//! [`rpg_core::run_batch`], prints, and decides whether the numbers are
//! acceptable. That split is what keeps `rpg-core` free of I/O while still
//! shipping a usable tool.
//!
//! It also means this binary must not *derive* statistics. Every figure in the
//! table comes from a method on the report, because a column computed here is a
//! second definition of a number that nothing tests -- which is exactly how
//! `miss%` was wrong for two releases.
//!
//! ```text
//! cargo run -p rpg-core --bin balance
//! cargo run -p rpg-core --bin balance -- --json > ../balance-report.json
//! cargo run -p rpg-core --bin balance -- --matchups ../data/matchups.json
//! cargo run -p rpg-core --bin balance -- --only matchup.dio_boss
//! ```
//!
//! Exit code 1 when a matchup falls outside its declared band or stalls, so the
//! same command works as a CI gate. `--no-fail` reports without judging, which
//! is what you want while actively retuning numbers.
//!
//! # Probing
//!
//! `--combatants`, `--stands` and `--skills` replace the compiled-in content
//! with a file. This exists for one purpose: measuring how a stat maps to a win
//! rate needs several variants of the same enemy in a single run, and editing
//! shipped content once per variant would put five throwaway combatants in
//! `data/` and five commits in the history for numbers nobody intends to keep.
//! Probe rosters live in `tools/probe/`, where the game, the data validator and
//! the CI gate never see them:
//!
//! ```text
//! cargo run -q -p rpg-core --bin balance -- \
//!     --combatants ../tools/probe/combatants.probe.json \
//!     --matchups   ../tools/probe/matchups.probe.json \
//!     --no-fail
//! ```
//!
//! A probe run is a measurement, never a release gate. The gate is
//! `cargo run -p rpg-core --bin balance` with no arguments, which can only ever
//! read the content the build was made from.

use std::process::ExitCode;

use rpg_core::report::{BatchReport, CombatantStats, MatchupReport};
use rpg_core::{parse_matchups, run_batch, Database, Matchup};

// Compiled in, exactly like the determinism test, so that an argument-free run
// measures the content this build was made from and nothing else.
const STANDS: &str = include_str!("../../../../data/stands.json");
const SKILLS: &str = include_str!("../../../../data/skills.json");
const COMBATANTS: &str = include_str!("../../../../data/combatants.json");
const MATCHUPS: &str = include_str!("../../../../data/matchups.json");

#[derive(Debug, PartialEq, Eq)]
struct Options {
    stands_path: Option<String>,
    skills_path: Option<String>,
    combatants_path: Option<String>,
    matchups_path: Option<String>,
    only: Vec<String>,
    json: bool,
    fail_on_violation: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            stands_path: None,
            skills_path: None,
            combatants_path: None,
            matchups_path: None,
            only: Vec::new(),
            json: false,
            fail_on_violation: true,
        }
    }
}

impl Options {
    /// True when any content file came from disk rather than from the build.
    fn uses_external_content(&self) -> bool {
        self.stands_path.is_some() || self.skills_path.is_some() || self.combatants_path.is_some()
    }
}

const USAGE: &str = "\
usage: balance [options]

  --matchups <path>     read the encounter list from a file instead of the
                        compiled-in data/matchups.json
  --combatants <path>   read the roster from a file (probe runs only)
  --stands <path>       read the stand list from a file (probe runs only)
  --skills <path>       read the skill list from a file (probe runs only)
  --only <id>           run a single matchup; repeatable
  --json                print the full report as JSON instead of a table
  --no-fail             always exit 0, even when a band is violated
  -h, --help            print this message
";

fn main() -> ExitCode {
    let options = match parse_args_from(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("balance: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    match run(&options) {
        Ok(report) => {
            if options.json {
                match report.to_json() {
                    Ok(json) => println!("{json}"),
                    Err(error) => {
                        eprintln!("balance: could not serialize the report: {error}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_report(&report);
                if options.uses_external_content() {
                    println!(
                        "\nnote: content was read from disk, so these numbers do not \
                         describe the shipped game"
                    );
                }
            }

            if options.fail_on_violation && !report.all_healthy() {
                if !options.json {
                    eprintln!(
                        "\nbalance: {} matchup(s) outside their declared band or stalling",
                        report.unhealthy().len()
                    );
                }
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("balance: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `Ok(None)` means help was requested; hand-rolled rather than pulling in an
/// argument parser, because a dependency in `rpg-core` is a dependency in every
/// consumer of the library, including the Godot bridge.
///
/// Takes an iterator rather than reading the environment directly so that the
/// parser is testable.
fn parse_args_from<I>(args: I) -> Result<Option<Options>, String>
where
    I: IntoIterator<Item = String>,
{
    let mut options = Options::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => options.json = true,
            "--no-fail" => options.fail_on_violation = false,
            "--matchups" => options.matchups_path = Some(value_for(&mut args, "--matchups")?),
            "--combatants" => options.combatants_path = Some(value_for(&mut args, "--combatants")?),
            "--stands" => options.stands_path = Some(value_for(&mut args, "--stands")?),
            "--skills" => options.skills_path = Some(value_for(&mut args, "--skills")?),
            "--only" => options.only.push(value_for(&mut args, "--only")?),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    Ok(Some(options))
}

/// Pulls the value that follows a flag, naming the flag in the error so that a
/// missing path is diagnosable without reading the source.
fn value_for<I>(args: &mut I, flag: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
    if value.starts_with("--") {
        return Err(format!("{flag} needs a value, found the flag '{value}'"));
    }
    Ok(value)
}

/// Reads an override file, or hands back the compiled-in content.
fn content(path: Option<&String>, embedded: &str) -> Result<String, String> {
    match path {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| format!("could not read '{path}': {error}")),
        None => Ok(embedded.to_string()),
    }
}

fn run(options: &Options) -> Result<BatchReport, String> {
    let stands = content(options.stands_path.as_ref(), STANDS)?;
    let skills = content(options.skills_path.as_ref(), SKILLS)?;
    let combatants = content(options.combatants_path.as_ref(), COMBATANTS)?;

    let db = Database::from_json(&stands, &skills, &combatants)
        .map_err(|error| format!("content failed to load:\n{error}"))?;

    let source = content(options.matchups_path.as_ref(), MATCHUPS)?;
    let all = parse_matchups(&source).map_err(|error| format!("{error}"))?;

    let selected: Vec<Matchup> = if options.only.is_empty() {
        all
    } else {
        for id in &options.only {
            if !all.iter().any(|matchup| &matchup.id == id) {
                return Err(format!("no matchup with id '{id}'"));
            }
        }
        all.into_iter()
            .filter(|matchup| options.only.contains(&matchup.id))
            .collect()
    };

    if selected.is_empty() {
        return Err("the encounter list is empty".to_string());
    }

    run_batch(&db, &selected).map_err(|error| format!("{error}"))
}

fn print_report(report: &BatchReport) {
    // Two units share one table: four columns are per-battle averages and one
    // is a whole-batch total. Dio printing `rolls 179` beside `sp/b 179` is a
    // coincidence, and without this line it reads as a relation.
    println!("columns: dealt/b, taken/b, heal/b and sp/b are per-battle averages,");
    println!("where heal/b counts HP a combatant restored through its own actions");
    println!("and never regeneration, which has no healer; rolls counts every");
    println!("accuracy check in the whole batch and miss% the share of those rolls");
    println!("that failed; the actions block under each table counts whole-batch");
    println!("skill uses, with the share of that combatant's own turns in brackets");

    for matchup in &report.matchups {
        print_matchup(matchup);
    }

    println!("\nsummary");
    println!(
        "  {:<28} {:>5}  {:>13}  {:>6}",
        "matchup", "win%", "band", "status"
    );
    for matchup in &report.matchups {
        println!(
            "  {:<28} {:>4}%  {:>5}..{:<6} {}",
            truncate(&matchup.id, 28),
            matchup.win_rate_percent,
            matchup.band.min_percent,
            matchup.band.max_percent,
            verdict(matchup)
        );
    }
}

fn print_matchup(matchup: &MatchupReport) {
    println!("\n{} [{}]", matchup.name, matchup.id);
    println!(
        "  {} battles: {} won, {} lost, {} stalled -> {}% win rate (band {}..{})",
        matchup.battles,
        matchup.wins,
        matchup.losses,
        matchup.stalemates,
        matchup.win_rate_percent,
        matchup.band.min_percent,
        matchup.band.max_percent
    );
    println!(
        "  turns: {} median, {} shortest, {} longest",
        matchup.median_turns, matchup.shortest_turns, matchup.longest_turns
    );

    // `rolls` is printed next to `miss%` on purpose: the percentage alone cannot
    // distinguish an accurate attacker from one that never attacked, and both
    // read as 0%.
    //
    // `heal/b` sits next to `dealt/b` for the same reason: those two together
    // are the only way to tell a support character from a combatant the AI
    // never lets act, and before this column existed they were identical rows.
    println!(
        "  {:<20} {:>6} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7}",
        "combatant", "team", "dealt/b", "taken/b", "heal/b", "sp/b", "rolls", "miss%", "alive%"
    );
    for stats in &matchup.combatants {
        let team = match stats.team {
            rpg_core::Team::Party => "party",
            rpg_core::Team::Foe => "foe",
        };
        println!(
            "  {:<20} {:>6} {:>8} {:>8} {:>8} {:>8} {:>7} {:>6}% {:>6}%",
            truncate(&stats.name, 20),
            team,
            stats.damage_dealt_per_battle(),
            stats.damage_received_per_battle(),
            stats.healing_done_per_battle(),
            stats.sp_spent / i64::from(stats.battles.max(1)),
            stats.attack_rolls,
            stats.miss_percent(),
            stats.survival_percent()
        );
    }

    // What each combatant actually did. Printed as its own block rather than as
    // a column because the repertoire is a list of variable length, and printed
    // at all because every behavioural claim made about this AI before this
    // block existed was inferred from `rolls` and `sp/b` -- and the inferences
    // were wrong.
    println!("  actions by skill");
    for stats in &matchup.combatants {
        println!(
            "  {:<20} {}",
            truncate(&stats.name, 20),
            skill_breakdown(stats)
        );
    }

    // Called out explicitly because it is invisible in a win rate and is almost
    // always a targeting defect rather than a design choice.
    //
    // The wording tracks what `inert_combatants` actually selects, which is a
    // combatant that neither dealt damage nor healed. Saying "dealt no damage"
    // would name a wider set than the one being printed, and would read as an
    // accusation against a support character doing its job.
    let inert = matchup.inert_combatants();
    if !inert.is_empty() {
        let names: Vec<&str> = inert.iter().map(|stats| stats.name.as_str()).collect();
        println!(
            "  warning: neither dealt damage nor healed in any battle: {}",
            names.join(", ")
        );
    }
    // Arithmetically impossible, so it can only mean the accounting in
    // `sim::accumulate` drifted again. Louder than a wrong percentage.
    let miscounted = matchup.miscounted_combatants();
    if !miscounted.is_empty() {
        let names: Vec<&str> = miscounted.iter().map(|stats| stats.name.as_str()).collect();
        println!(
            "  warning: more misses than accuracy rolls, so miss% is not trustworthy: {}",
            names.join(", ")
        );
    }
    // Same class of defect, one field over: a breakdown that does not sum to
    // the action count is describing a different set of turns than the table.
    let unattributed = matchup.unattributed_combatants();
    if !unattributed.is_empty() {
        let names: Vec<String> = unattributed
            .iter()
            .map(|stats| {
                format!(
                    "{} ({} of {} actions)",
                    stats.name,
                    stats.counted_skill_uses(),
                    stats.actions
                )
            })
            .collect();
        println!(
            "  warning: the skill breakdown does not account for every action: {}",
            names.join(", ")
        );
    }
    if matchup.stalemates > 0 {
        println!(
            "  warning: {} battle(s) hit the turn limit without resolving",
            matchup.stalemates
        );
    }
}

/// One combatant's repertoire as a single line, most used skill first.
fn skill_breakdown(stats: &CombatantStats) -> String {
    let ranked = stats.skill_uses_ranked();
    if ranked.is_empty() {
        // Distinct from "used nothing but the basic attack": this combatant
        // never got a turn, or the accounting lost it.
        return "-".to_string();
    }
    ranked
        .iter()
        .map(|entry| {
            format!(
                "{} {} ({}%)",
                short_skill(&entry.skill),
                entry.uses,
                stats.skill_use_percent(&entry.skill)
            )
        })
        .collect::<Vec<String>>()
        .join(", ")
}

/// Drops the `skill.` namespace for width. Presentation only -- the JSON report
/// keeps the full id, because that is the one a content file can be grepped for.
fn short_skill(id: &str) -> &str {
    id.strip_prefix("skill.").unwrap_or(id)
}

fn verdict(matchup: &MatchupReport) -> &'static str {
    if !matchup.within_band() {
        "OUT OF BAND"
    } else if matchup.stalemates > 0 {
        "STALLED"
    } else {
        "ok"
    }
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>()
        + "~"
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpg_core::Team;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn no_arguments_means_the_shipped_content_and_a_failing_gate() {
        let options = parse_args_from(args(&[])).unwrap().unwrap();
        assert_eq!(options, Options::default());
        assert!(options.fail_on_violation);
        assert!(!options.uses_external_content());
    }

    #[test]
    fn a_probe_run_is_recognisable_from_its_options() {
        let options = parse_args_from(args(&[
            "--combatants",
            "../tools/probe/combatants.probe.json",
            "--matchups",
            "../tools/probe/matchups.probe.json",
            "--no-fail",
        ]))
        .unwrap()
        .unwrap();

        assert!(options.uses_external_content());
        assert!(!options.fail_on_violation);
        assert_eq!(
            options.matchups_path.as_deref(),
            Some("../tools/probe/matchups.probe.json")
        );
    }

    /// A path is mandatory, and swallowing the *next flag* as the path would
    /// turn a typo into a confusing "could not read '--json'".
    #[test]
    fn a_flag_that_needs_a_value_never_eats_the_next_flag() {
        let error = parse_args_from(args(&["--combatants", "--json"])).unwrap_err();
        assert!(error.contains("--combatants"), "{error}");

        let error = parse_args_from(args(&["--matchups"])).unwrap_err();
        assert!(error.contains("--matchups"), "{error}");
    }

    #[test]
    fn help_short_circuits_and_an_unknown_flag_is_reported() {
        assert!(parse_args_from(args(&["--help"])).unwrap().is_none());
        assert!(parse_args_from(args(&["--sweep"]))
            .unwrap_err()
            .contains("--sweep"));
    }

    /// The breakdown is read to decide what to change next, so its order and its
    /// shares are part of the tool, not decoration. An idle combatant must be
    /// distinguishable from one that only ever punched.
    #[test]
    fn a_breakdown_line_ranks_by_use_and_marks_an_idle_combatant() {
        let mut jotaro = CombatantStats::new("pc.jotaro", "Jotaro", Team::Party);
        for skill in [
            "skill.tempo_halt",
            "skill.rush_barrage",
            "skill.tempo_halt",
            "skill.strike",
        ] {
            jotaro.actions += 1;
            jotaro.record_skill_use(skill);
        }

        assert_eq!(
            skill_breakdown(&jotaro),
            "tempo_halt 2 (50%), rush_barrage 1 (25%), strike 1 (25%)"
        );
        assert_eq!(short_skill("skill.strike"), "strike");
        assert_eq!(
            short_skill("strike"),
            "strike",
            "an id without the namespace must survive unchanged"
        );

        let idle = CombatantStats::new("npc.spectator", "Spectator", Team::Foe);
        assert_eq!(skill_breakdown(&idle), "-");
    }
}
