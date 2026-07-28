# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
as interpreted in [docs/RELEASING.md](docs/RELEASING.md).

Every released version has a git tag (`vX.Y.Z`) and a matching
[GitHub Release](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases)
with prebuilt libraries. A section here without a tag is not a release.

## [Unreleased]

M2. Balance stops being an opinion. Every encounter in the game now declares the
win rate it is supposed to produce, a headless harness measures whether it does,
and CI fails the build when it does not.

The measurement changed the content. Before the harness, two of three encounters
were outside the range their designer would have claimed for them -- one was a
100% guaranteed win presented as a real fight, and the boss was won 92% of the
time. Both are now inside their declared bands, and the reasoning behind every
number that moved is in [docs/BALANCE-LOG.md](docs/BALANCE-LOG.md).

### Added

- **Headless batch simulator** (`rpg_core::sim`). `run_matchup` plays one
  encounter over a fixed seed list with both sides driven by the existing AI;
  `run_batch` runs every encounter in the file. No Godot, no rendering, no
  wall-clock access -- the same seed produces the same battle on every machine.
- **`rpg_core::report`**: per-encounter win rate, median and min/max battle
  length, and per-combatant damage dealt, damage taken, SP spent, miss rate and
  survival rate. A report is the unit of evidence in this project; a
  playthrough is not.
- **`data/matchups.json`**: encounters as content, not as test fixtures. Each
  declares its party, its foes, twelve fixed seeds, a `min_percent..max_percent`
  win-rate band and a turn limit. The band is the design intent, written down
  where it can be checked.
- **`cargo run -p rpg-core --bin balance`**: the harness as a command. Prints a
  per-encounter breakdown, exits non-zero when any encounter leaves its band.
  Flags: `--only <id>`, `--json`, `--no-fail`, and `--combatants`/`--stands`/
  `--skills` to run against content outside `data/`.
- **`balance` CI job**, in the `ci` aggregate alongside format, clippy, tests
  and the audits. It runs the binary with no arguments, so the gate can only
  ever measure the content compiled into the crate.
- **`core/tests/balance_bounds.rs`**: the same assertion as a test, so a local
  `cargo test` catches a balance regression before a push does.
- **[docs/BALANCE-CURVE.md](docs/BALANCE-CURVE.md)**: the measured relationship
  between enemy output and win rate, swept in one run over five clones of the
  same enemy. The curve is flat up to a ratio of 0.65, bends between 0.65 and
  0.74, holds a plateau to 0.81, then falls to 8% by 0.99. Six of the seven
  changes in this milestone were sized against an interpolation that turned out
  to be wrong by 0.15; the sweep that replaced it cost a single run.
- **`tools/probe/`**: throwaway content used for that sweep, with a README
  stating plainly that it is not game content and cannot reach the game, the
  validator or the CI gate.
- The content validator now checks `data/matchups.json`: id namespacing and
  uniqueness, every combatant reference resolving, no combatant on both sides of
  a fight, duplicate seeds rejected, bands within 0..100 and correctly ordered,
  and warnings for a 0..100 band, a seed list too short to resolve a percentage,
  and any combatant that no encounter exercises.

### Changed

Seven changes, each measured before and after, each in its own commit, all
argued in [docs/BALANCE-LOG.md](docs/BALANCE-LOG.md):

- **Enemy targeting is no longer deterministic** (`FOCUS_FIRE_CHANCE = 55` in
  `ai.rs`). Unconditional lowest-HP focus fire meant Kakyoin was downed in 12 of
  12 boss battles and absorbed 63% of all enemy damage -- a guaranteed casualty
  from turn one, decided before the fight started. Hostile actors now commit to
  the weakest target 55% of the time and pick at random otherwise. This is the
  only change in the milestone that touches the rules, and it is the one that
  every earlier finding turned out to be about.
- `skill.restore` heal power 230 -> 140. Healing returned 11.5 HP per SP and
  roughly 950 HP per battle from a character nobody was attacking.
- `npc.dio` atk 100 -> 130. With targeting fixed, the boss could not win: the
  party out-healed and out-lasted it 100% of the time.
- `npc.flame_assassin` hp 480 -> 640, so it stops dying below Kakyoin's HP pool
  and lives long enough to act.
- **New enemy `npc.iron_brawler` and new `stand.iron_hymn`**, replacing the
  300 HP thug in the ambush. That thug dealt 13 damage per battle; the encounter
  was not a fight, and no stat on a body that small could make it one.
- `npc.iron_brawler` atk 80 -> 105 -> 135, the last step taken from the curve
  sweep rather than predicted. It reproduced the swept report digit for digit.

Measured result of all of the above:

| Encounter | Win rate | Declared band |
| --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 |
| `matchup.assassin_ambush` | 67% | 45..90 |
| `matchup.dio_boss` | 50% | 35..75 |

### Fixed

- `sim::tests::damage_is_attributed_to_both_sides_of_every_hit` asserted a
  hero could only be downed once across a whole seed list. The test had never
  been executed before it was pushed; the expectation was wrong, not the engine.
- The duplicate-seed guard in the matchup parser was a `seen` vector filtered by
  a predicate that could never return true, so it accepted duplicates silently.
  Replaced with a real membership test and a test that covers it.
- `balance-report.json` is ignored, so a local `--json` run cannot be committed
  by accident.

### Known limitations

Recorded so a win rate in this file is not read as more than it is:

- **Twelve seeds resolve to 8.3 percentage points.** 67% and 75% are not
  distinguishable results, and a win rate near a band edge is not decisively
  inside or outside it.
- **The harness plays worse than a person.** Auto-battle takes the
  highest-power affordable skill and never sets up, so a reported win rate is a
  floor for a competent player, not a forecast of their experience.
- **Five of eleven skills are unreachable.** `ai::best_offensive` filters on
  damage, so buffs, guards and area attacks are never chosen -- Jotaro has never
  used `skill.guard_stance` in any recorded run. Every number here therefore
  measures a subset of the content.
- **Healing is not attributed.** `Event::Healed` carries a target and no source,
  so healing done has to be inferred from SP spent.
- **`miss%` is inflated for area skills**, which count a miss per target and an
  action once.
- **A rules change resets the series.** RNG draw order is part of the rules, so
  after any edit to `ai.rs`, `resolve.rs` or `battle.rs` the same seed no longer
  reproduces the same battle, and numbers across that boundary are not
  comparable.
- **The ambush is still decided by one character.** Jotaro deals 76% of the
  party's damage and his survival rate tracks the encounter's win rate exactly
  across all five swept points. The band is closed; the roster imbalance behind
  it is not.
- **The Iron Brawler hits at an effective 157 against Dio's 160**, which is
  mechanically correct and fictionally awkward for a random-encounter enemy. The
  alternative was nerfing a skill shared with the boss fight.
- Elemental resistances are still carried in events but not applied in damage.

## [0.2.0] - 2026-07-28

M1. The game is playable: one battle, start to finish, driven entirely by the
player. Verified by playing it in Godot 4.7.1 on Windows -- commands and targets
chosen by hand, events animated in order, ending on a resolution screen with a
clean console. No CI job in this repository can make that claim, because none of
them run Godot.

### Added

- `BattleView` rebuilt around a real, procedurally-built UI: per-combatant
  HP/SP bars, a tempo gauge (sorted by each combatant's current `tempo`
  value, the same key `Battle::ready_actor` uses to pick who acts next), a
  command menu (Attack / known skills / Guard / Wait), and a target picker
  for single-target actions with a cancel path back to the command menu.
- Event animator queue in `battle_view.gd`: events are queued and played one
  at a time with a fixed delay, replacing the immediate `print` dump. The
  queue is re-entrant-safe, so a call arriving mid-drain appends instead of
  racing it.
- Real target selection, replacing the stub that always attacked the first
  living foe. Single-enemy and single-ally actions list every legal living
  target; multi-target and self-only skills submit immediately, since
  `rpg_core::resolve::resolve_targets` ignores the `target` field for those
  kinds.
- `status_damaged` now renders as its own log line, distinct from `damaged`;
  the meaningless `potency` field on binary statuses (stun) is no longer
  printed.
- Victory / defeat / stalemate resolution: the command and target menus are
  torn down and a result line is shown once `Phase::Finished` is reached.

### Fixed

- The Markdown link check no longer fails on a link that is not dead. It
  reported zero errors and two timeouts, both on the same cited URL
  (`prng.di.unimi.it`, the reference SplitMix64 source), and lychee exits
  non-zero on a timeout. That host is a single unfronted academic server;
  blocking a merge on its uptime measures nothing about this repository, so
  it joined `.lycheeignore` with the reason recorded inline. The citation
  itself stays in the prose.
- The temporary `.lycheeignore` entries for `v0.1.0` URLs were removed. They
  existed only because the release did not exist yet when they were written;
  those links now resolve and are checked for real.
- The Windows quickstart invoked the content sync script through `pwsh`,
  which is PowerShell 7 and is not present on a stock Windows install. The
  script has always been 5.1 compatible, so the documented command was the
  only obstacle. The step numbering was also corrected: the `Copy-Item` line
  is relative to the repository root, and running it from `src/` looks for
  `src/src/`.

### Known limitations

- Combat is one-sided against the player. The first real playthrough ended in
  defeat with the boss on 389 of 1520 HP and three of five combatants downed.
  One battle is an anecdote, not a measurement, which is precisely why M2
  exists: no balance claim in this repository is evidence-based until the
  harness reports win rates over thousands of seeded runs.
- The event log is the only feedback channel. Damage does not appear on the
  combatants themselves, so the fight is read by scrolling text.
- The tempo gauge shows each combatant's current `tempo` value sorted the way
  the scheduler ranks them; it is not a forecast of the next several turns.
  A real forecast would need effective, status-modified speed exposed from
  the core, which does not happen yet.
- Elemental resistances are still carried in events but not applied in damage.
- SP is still not a binding constraint for most combatants.

## [0.1.0] - 2026-07-28

First fixation. The simulation is complete and tested; the presentation layer is
a runnable stub, not a game.

### Added

- **`rpg-core`**: deterministic, engine-free turn-based battle simulation.
  - Tempo (ATB) scheduler with integer accumulation and deterministic
    tie-breaking.
  - `tempo_lock` primitive for tempo-denial abilities, with no special case in
    the scheduler.
  - Command to event pipeline: validation precedes mutation, and a rejected
    command does not consume the turn.
  - SplitMix64 RNG stored inside `BattleState`; no floats, no globals, no clock
    access, so a seed plus a command sequence fully determines the event log.
  - Data-driven content: skills, stands and combatants loaded from `data/*.json`
    with reference validation at load time.
  - Three AI profiles (aggressive, support, trickster) drawing from the battle
    RNG, so unattended battles stay replayable.
- **`rpg-bridge`**: GDExtension boundary exposing five methods that exchange JSON
  strings only, so a schema mismatch produces a parse error rather than
  undefined behaviour.
- **Godot 4 project**: `BattleView` scene and script that drive the simulation
  and animate its events without recomputing any outcome.
- **Content**: 11 skills, 5 stands, 6 combatants, plus a stdlib-only Python
  validator that checks types, enums, cross-references and balance smells.
- **Documentation**: architecture, engine decision record (ADR-0001), roadmap,
  Windows development guide, release process, data provenance policy.
- **CI/CD**: rustfmt, clippy with `-D warnings`, tests and doc builds on Linux
  and Windows, content validation, `cargo-deny` supply-chain audit, spellcheck,
  Markdown link check, Dependabot, and tag-triggered releases producing
  per-platform archives with checksums.
- `rpg_core::json_compat` with five unit tests, including a guard assertion
  that an un-normalized Godot-style payload must fail; without it the test
  would pass even if the normalizer were deleted.
- Regression test asserting both halves of the damage-over-time fix: the
  presence of `status_damaged` and the absence of any self-attributed
  `Damaged`.
- Windows setup guidance covering the failures met in practice: PowerShell
  versus `cmd.exe`, `where` shadowed by `Where-Object`, and archives that
  extract into a subdirectory named after the archive.

### Fixed

These are defects found and corrected before the first tag existed. They are
recorded rather than squashed away, because the reasoning is the useful part.

- **GDExtension no longer fails to compile.** `GString` implements
  `From<&str>` and `From<&String>` but not `From<String>`, because the
  conversion copies into Godot-managed memory. Every conversion now goes
  through a single `json_to_gstring` helper.
- **Content authored in GDScript is accepted again.** Godot's JSON has no
  integer type, so numbers round-tripped through `JSON.parse_string` and
  `JSON.stringify` arrive as `95.0` where the schema declares `i32`.
  `rpg_core::json_compat` normalizes integral floats at the FFI boundary,
  leaving the simulation's integer-only schema intact.
- **`compatibility_minimum` now states the truth.** It claimed 4.2 while
  `godot` 0.5.3 compiles against the Godot 4.6 API. Lowering that number
  never widened support; it only postponed the failure from a clear version
  rejection to a missing-symbol crash. Verified against the engine's own
  startup line: `API v4.6.stable.official, runtime v4.7.1.stable.official`.
- **Damage over time is attributed to the status, not to its victim.** A bleed
  tick was logged as `Damaged { actor, target }` with both fields set to the
  victim and the element hard-coded to `physical`, so the log claimed a
  character attacked itself with a blow it never made. It now emits
  `status_damaged { target, status, amount }`, with no attacker, no element
  and no crit -- none of which exist for a condition. `Damaged` consequently
  means exactly one thing, which an animator can rely on.
- Removed a needless `mut` in `Database::skills_for` that would have failed
  CI, where warnings are errors.
- **CI no longer denies warnings across the dependency tree.** A
  workflow-level `RUSTFLAGS: -D warnings` applies to every crate cargo
  compiles, so a single warning from `gdext`, `glam`, `venial` or `syn` failed
  the Linux and Windows builds on commits that touched none of them. Denying
  warnings is a policy for code we own, so it is passed to clippy per job.
- **The supply-chain audit distinguishes wildcards by origin.** `cargo-deny`
  rejected `rpg-core = { path = "../core" }` as a wildcard dependency. A path
  dependency carries no version requirement by construction, but it can only
  ever resolve to a directory in this repository. Wildcards remain denied for
  registry crates, where they really do accept any future release.
- **The link checker actually checks links.** It was invoked with
  `--exclude-mail`, removed in lychee 0.24 in favour of `--include-mail`,
  which already defaults to false. The unknown argument aborted the run before
  a single link was fetched, producing a failure that looked like a dead link.
- Corrected a misspelling in `battle_view.gd` reported by the spellchecker,
  and moved every workflow to `actions/checkout@v5`, which clears the Node.js
  20 deprecation warning.

### Known limitations

- No gameplay UI yet; `battle_view.gd` auto-selects a basic attack so the loop
  can be run end to end. A battle driven by that stub is not evidence about
  balance: the party never uses a skill, never heals, and never retargets, and
  it always attacks the first living foe, so a second enemy is never touched.
- SP is not yet a meaningful constraint. In recorded runs the strongest foe
  spent 176 SP across eight uses of one skill without ever running dry.
- Elemental resistances are carried in events but not yet applied in damage.
- Combat numbers are unbalanced; only determinism and termination are tested.
- Seeds must stay below 2^53 when a battle is created from GDScript. Godot
  represents every JSON number as a 64-bit float, whose mantissa is 53 bits,
  so a larger seed would be silently rounded and the fight would not replay.
  `Time.get_unix_time_from_system()` is far below that ceiling; a full-range
  `u64` seed would have to cross the boundary as a string.

[Unreleased]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases/tag/v0.1.0
