//! `balance` -- headless balance harness runner.
//!
//! All I/O for the harness lives here and nowhere else. The simulation and the
//! statistics are in `rpg_core::sim` and `rpg_core::report`, which know nothing
//! about files, stdout or exit codes; this binary reads content, calls
//! [`rpg_core::run_batch`], prints, and decides whether the numbers are
//! acceptable. That split is what keeps `rpg-core` free of I/O while still
//! shipping a usable tool.
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

use std::process::ExitCode;

use rpg_core::report::{BatchReport, MatchupReport};
use rpg_core::{parse_matchups, run_batch, Database, Matchup};

// Compiled in, exactly like the determinism test, so the harness measures the
// content this build was made from. `--matchups` overrides only the encounter
// list, which is the file a designer actually iterates on.
const STANDS: &str = include_str!("../../../../data/stands.json");
const SKILLS: &str = include_str!("../../../../data/skills.json");
const COMBATANTS: &str = include_str!("../../../../data/combatants.json");
const MATCHUPS: &str = include_str!("../../../../data/matchups.json");

struct Options {
    matchups_path: Option<String>,
    only: Vec<String>,
    json: bool,
    fail_on_violation: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            matchups_path: None,
            only: Vec::new(),
            json: false,
            fail_on_violation: true,
        }
    }
}

const USAGE: &str = "\
usage: balance [options]

  --matchups <path>   read the encounter list from a file instead of the
                      compiled-in data/matchups.json
  --only <id>         run a single matchup; repeatable
  --json              print the full report as JSON instead of a table
  --no-fail           always exit 0, even when a band is violated
  -h, --help          print this message
";

fn main() -> ExitCode {
    let options = match parse_args() {
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
            }

            if options.fail_on_violation && !report.all_healthy() {
                if !options.json {
                    eprintln!("\nbalance: {} matchup(s) outside their declared band or stalling", report.unhealthy().len());
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
fn parse_args() -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => options.json = true,
            "--no-fail" => options.fail_on_violation = false,
            "--matchups" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--matchups needs a path".to_string())?;
                options.matchups_path = Some(value);
            }
            "--only" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--only needs a matchup id".to_string())?;
                options.only.push(value);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    Ok(Some(options))
}

fn run(options: &Options) -> Result<BatchReport, String> {
    let db = Database::from_json(STANDS, SKILLS, COMBATANTS)
        .map_err(|error| format!("shipped content in data/ failed to load:\n{error}"))?;

    let source = match &options.matchups_path {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| format!("could not read '{path}': {error}"))?,
        None => MATCHUPS.to_string(),
    };
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

    println!(
        "  {:<20} {:>6} {:>8} {:>8} {:>8} {:>7} {:>7}",
        "combatant", "team", "dealt/b", "taken/b", "sp/b", "miss%", "alive%"
    );
    for stats in &matchup.combatants {
        let team = match stats.team {
            rpg_core::Team::Party => "party",
            rpg_core::Team::Foe => "foe",
        };
        let attempts = stats.actions.max(1);
        println!(
            "  {:<20} {:>6} {:>8} {:>8} {:>8} {:>6}% {:>6}%",
            truncate(&stats.name, 20),
            team,
            stats.damage_dealt_per_battle(),
            stats.damage_received_per_battle(),
            stats.sp_spent / i64::from(stats.battles.max(1)),
            rpg_core::report::percent(stats.misses, attempts),
            stats.survival_percent()
        );
    }

    // Called out explicitly because it is invisible in a win rate and is almost
    // always a targeting defect rather than a design choice.
    let inert = matchup.inert_combatants();
    if !inert.is_empty() {
        let names: Vec<&str> = inert.iter().map(|stats| stats.name.as_str()).collect();
        println!(
            "  warning: dealt no damage in any battle: {}",
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
    value.chars().take(width.saturating_sub(1)).collect::<String>() + "~"
}
