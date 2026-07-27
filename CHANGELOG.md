# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
as interpreted in [docs/RELEASING.md](docs/RELEASING.md).

Every released version has a git tag (`vX.Y.Z`) and a matching
[GitHub Release](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases)
with prebuilt libraries. A section here without a tag is not a release.

## [Unreleased]

### Fixed

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
- Removed a needless `mut` in `Database::skills_for` that would have failed
  CI, where warnings are errors.

### Added

- `rpg_core::json_compat` with five unit tests, including a guard assertion
  that an un-normalized Godot-style payload must fail; without it the test
  would pass even if the normalizer were deleted.
- Windows setup guidance covering the failures met in practice: PowerShell
  versus `cmd.exe`, `where` shadowed by `Where-Object`, and archives that
  extract into a subdirectory named after the archive.

### Planned

- Battle UI: HP/SP bars, tempo order preview, command menu, target picker (M1).
- Event animator queue replacing the placeholder `print` calls in `battle_view.gd`.
- Attribute damage-over-time ticks to the status that caused them rather than
  to the victim, so the UI cannot narrate a character attacking itself.
- Balance harness: thousands of headless AI-vs-AI battles reporting win rates (M2).

## [0.1.0] - 2026-07-27

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

### Known limitations

- No gameplay UI yet; `battle_view.gd` auto-selects a basic attack so the loop
  can be run end to end. A battle driven by that stub is not evidence about
  balance: the party never uses a skill, never heals, and never retargets.
- Elemental resistances are carried in events but not yet applied in damage.
- Combat numbers are unbalanced; only determinism and termination are tested.
- Damage-over-time ticks name the victim as the actor, which reads as
  self-inflicted damage in the event log.
- Seeds must stay below 2^53 when a battle is created from GDScript. Godot
  represents every JSON number as a 64-bit float, whose mantissa is 53 bits,
  so a larger seed would be silently rounded and the fight would not replay.
  `Time.get_unix_time_from_system()` is far below that ceiling; a full-range
  `u64` seed would have to cross the boundary as a string.

[Unreleased]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases/tag/v0.1.0
