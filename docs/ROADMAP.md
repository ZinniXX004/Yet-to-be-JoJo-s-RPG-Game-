# Roadmap

Milestones are ordered so that each one is independently demonstrable. For a
portfolio project, a finished vertical slice beats a half-built epic every time.

Each milestone maps to exactly one version tag. The mapping is authoritative in
[RELEASING.md](RELEASING.md); it is repeated here so the two documents can be
checked against each other.

| Milestone | Version | Exit criterion |
| --- | --- | --- |
| M0 Foundation | `0.1.0` | Simulation tested, deterministic, reaching Godot |
| M1 Playable battle loop | `0.2.0` | One battle playable start to finish |
| M2 Balance harness | `0.3.0` | Headless mass simulation reporting win rates |
| M3 Content depth | `0.4.0` | Full roster, statuses and elemental resistances |
| M4 Portfolio polish | `1.0.0` | A stranger can play it in under 60 seconds |

## M0 - Foundation (done)

- [x] Engine/architecture decision recorded (ADR-0001)
- [x] Correct `.gitignore`, Git LFS configured before any binary asset
- [x] Data-driven content schema in `data/`
- [x] Deterministic battle core with tempo scheduler and event pipeline
- [x] Determinism regression test
- [x] Python data validator + CI on every push
- [x] `godot` crate version pinned; GDExtension loads in the editor and in the
      exported console binary
- [x] End-to-end proof: a full battle runs from GDScript through the FFI to a
      decisive outcome

## M1 - Playable battle loop (done)

**Exit criterion:** one full battle, start to victory screen, playable with mouse
and keyboard, no placeholder crashes.

**Met.** A full battle was played start to finish in Godot 4.7.1 on
2026-07-28: commands issued by hand, targets chosen by hand, events animated in
order, ending on a resolution screen (a defeat) with a clean console -- no
errors, no warnings, no placeholder crash.

- [x] `BattleView` scene: HP/SP bars, tempo order preview, command menu
- [x] Event animator: queue events, play one at a time, never block input forever
- [x] Target selection UI, including multi-target skills
- [x] Replace the placeholder command stub, which always attacks the first living
      foe. Until this is gone, any enemy after the first can go a whole battle
      untouched, and no balance conclusion drawn from a stub-driven run is valid
- [x] Surface `status_damaged` separately from `damaged` in the combat log, and
      suppress the meaningless `potency` on binary statuses such as stun
- [x] Victory / defeat resolution and transition

The milestone is closed on its stated exit criterion, which is playability, not
quality. What the first real playthrough exposed is recorded as M2 input rather
than quietly folded into M1:

- The party lost with the boss still on 389 of 1520 HP, and three of five
  combatants ended on 0 HP. One data point is not a balance finding, but it is
  the first evidence that the numbers are not merely untuned, they are
  one-sided.
- The log is the only feedback channel. Damage numbers do not appear on the
  combatants themselves, so a player reads the fight by scrolling text.
- The tempo readout collapses to a single entry once everyone else is downed,
  which is correct but reads as a bug. It is a presentation problem, not a
  scheduler one.

## M2 - Balance harness (next, `0.3.0`)

**Exit criterion:** `N` AI-vs-AI battles run headless in CI, reporting win rate,
median turn count and damage taken per combatant.

This lands before the content pass on purpose. Authoring a roster against
numbers that are still moving means authoring it twice.

- [ ] Batch runner over `step_with_ai`, seeded from a fixed list so results are
      comparable between commits
- [ ] Report per-combatant damage dealt and received, to catch enemies the AI
      never targets
- [ ] Assert bounds in CI: a matchup that is a guaranteed win or a guaranteed
      loss fails the build
- [ ] Rebalance `data/*.json` against the harness output, not against intuition
- [ ] Re-examine the SP economy: if a skill can be spammed for an entire battle
      without running dry, SP is not a resource and the AI is not choosing

### Planned changes, by file

Written out before any code so the diff can be reviewed against an intention
rather than justified after the fact.

| Path | Change | Why |
| --- | --- | --- |
| `src/core/src/sim.rs` *(new)* | `run_batch(matchup, seeds) -> BatchReport`, pure, no I/O | Statistics are a rule of the simulation, not of a binary. Keeping them in the library means they are unit-testable and reusable from a future frontend |
| `src/core/src/report.rs` *(new)* | `BatchReport`, `MatchupReport`, `CombatantStats`, all `Serialize` | A struct that serialises to JSON is both a CI artefact and a test fixture. Printing straight to stdout would make assertions impossible |
| `src/core/src/lib.rs` | Export the two new modules | -- |
| `src/core/src/bin/balance.rs` *(new)* | Thin CLI: parse args, call `run_batch`, print a table or `--json` | All I/O lives here, so `rpg-core`'s no-engine-no-clock-no-globals rule survives. A binary target needs no new crate and no workspace change |
| `src/core/tests/balance_bounds.rs` *(new)* | Assert every shipped matchup lands inside its declared win-rate band | This is the exit criterion, expressed as a test rather than as a paragraph |
| `data/matchups.json` *(new)* | Declared encounters: party, foes, seed list, acceptable win-rate band | Content, not code, in line with rule 4 of the layering rules. Adding an encounter must not require a recompile |
| `src/data-pipeline/validate_data.py` | Validate `matchups.json`: ids resolve, bands are ordered and within 0..100 | A typo in a combatant id must fail in the validator, not as a confusing panic inside the harness |
| `.github/workflows/ci.yml` | New `balance` job running the bounds test and uploading the JSON report | Cheap: it is CPU-only, no engine, no network |
| `data/*.json` | Rebalanced numbers, driven by harness output | -- |
| `CHANGELOG.md`, `README.md` | Record the harness and the measured win rates | A portfolio reader should see numbers, not adjectives |

### Design constraints for the harness

1. **Determinism is the whole point.** A run is `(matchup, seed)`; the same pair
   must produce the same report on any machine. No wall-clock, no thread pool
   ordering, no `HashMap` iteration leaking into the output.
2. **A fixed seed list, not a random sample.** Comparability between commits
   matters more than statistical purity. Widen the list deliberately, in a commit
   that says so.
3. **The harness never contains rules.** If it needs to know what a skill does,
   the rule is in the wrong layer.
4. **A band, not a target.** Asserting a 50 percent win rate makes the test
   fragile and the game boring. Assert an interval, and let a matchup that is a
   guaranteed win or a guaranteed loss fail the build.
5. **Report damage received per combatant.** The M1 playthrough had one character
   absorbing 78, 79, 87 and 105 damage in sequence while the boss spent 22 SP of
   200. Aggregate win rate hides that; per-combatant totals do not.

### Deliberately out of scope for `0.3.0`

Floating damage numbers, status icons and the single-entry tempo readout are all
presentation defects from the M1 playthrough. They belong to M3, and mixing them
into a balance release would make it impossible to tell whether a changed win
rate came from a number or from the UI.

## M3 - Content depth

**Exit criterion:** three playable characters, six enemies, two boss fights, all
authored purely in `data/`.

- [ ] Status effect icons and durations surfaced in the UI
- [ ] Floating damage numbers on the combatants, so the log stops being the only
      feedback channel (raised by the first M1 playthrough)
- [ ] Tempo readout that stays legible when only one combatant is standing
      (raised by the first M1 playthrough)
- [ ] Elemental resistance table (extension point already in the core)
- [ ] Enemy AI profiles beyond aggressive/support (scripted boss phases)
- [ ] Overworld or node-based map navigation
- [ ] Party management, equipment, leveling
- [ ] Save/load (serialize the whole state; determinism makes this cheap)
- [ ] Dialogue system fed from `data/`

## M4 - Portfolio polish

**Exit criterion:** a stranger can play it in under 60 seconds from the repo.

- [ ] Web export playable in-browser, linked from the README
- [ ] 30-second gameplay GIF in the README
- [ ] Audio: SFX per event type, one battle track
- [ ] Write-up of the determinism design with a replay demo

## Explicit non-goals

Scope discipline is the whole game. These are out until `1.0.0` ships:

- Multiplayer or netplay
- Procedural content generation
- 3D
- Mod support / plugin API
- Mobile ports
