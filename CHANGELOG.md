# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
as interpreted in [docs/RELEASING.md](docs/RELEASING.md).

Every released version has a git tag (`vX.Y.Z`) and a matching
[GitHub Release](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases)
with prebuilt libraries. A section here without a tag is not a release.

## [Unreleased]

M1 code is written and pushed to `development`. None of it has been run inside
the Godot editor yet -- see "Known limitations" below. Do not tag `0.2.0`
until it has.

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

### Planned

- Balance harness: thousands of headless AI-vs-AI battles reporting win rates,
  damage dealt and damage received per combatant (M2). Until it exists, no
  claim about balance in this repository is evidence-based.

### Known limitations

- The M1 UI above has not been run inside the Godot editor. It was written
  against the exact JSON schema in `rpg_core::event::Event` and
  `BattleState`/`Combatant`, but a schema-correct script can still fail at
  runtime for reasons no static read catches -- a wrong signal name, a
  container that lays out unusably on a real viewport, and so on. Treat M1 as
  implemented, not verified, until it has been played once start to finish.
- The tempo gauge shows each combatant's current `tempo` value sorted the way
  the scheduler ranks them; it is not a forecast of the next several turns.
  A real forecast would need effective, status-modified speed exposed from
  the core, which does not happen yet.

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

[Unreleased]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases/tag/v0.1.0
