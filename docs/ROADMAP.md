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

## M2 - Balance harness (done)

**Exit criterion:** `N` AI-vs-AI battles run headless in CI, reporting win rate,
median turn count and damage taken per combatant.

**Met.** 36 battles per run -- three encounters over twelve fixed seeds each --
play headless in the `balance` CI job and in `cargo test`, reporting exactly
those three quantities plus damage dealt, SP spent, miss rate and survival rate
per combatant. The job fails the build when a measured win rate leaves the band
its encounter declares in `data/matchups.json`.

| Encounter | Win rate | Declared band | Median length |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 | 3 turns |
| `matchup.assassin_ambush` | 67% | 45..90 | 19 turns |
| `matchup.dio_boss` | 50% | 35..75 | 50 turns |

The M1 suspicion was half right and half backwards: the numbers were indeed
one-sided, but in the party's favour. Unattended, the party won the boss fight
that the first playthrough lost -- 11 times out of 12.

- [x] Batch runner over `step_with_ai`, seeded from a fixed list so results are
      comparable between commits
- [x] Report per-combatant damage dealt and received, to catch enemies the AI
      never targets
- [x] Assert bounds in CI: a matchup that is a guaranteed win or a guaranteed
      loss fails the build
- [x] Rebalance `data/*.json` against the harness output, not against intuition
- [x] Re-examine the SP economy: if a skill can be spammed for an entire battle
      without running dry, SP is not a resource and the AI is not choosing

On that last item, the answer turned out to be the opposite of the M1 reading.
SP **is** binding: the Flame Assassin's per-battle damage is effectively constant
across fights of 13, 19 and 18 turns because 70 SP buys four expensive casts and
nothing more. The `176 SP` figure from M1 was a fight that ended early, not a
pool that could not be drained.

### Planned changes, by file -- and what was actually delivered

The table below was written before any code. It is kept, rather than rewritten to
match the outcome, because the gap between the two is the interesting part.

| Path | Change | Delivered |
| --- | --- | --- |
| `src/core/src/sim.rs` *(new)* | `run_batch(matchup, seeds) -> BatchReport`, pure, no I/O | Yes |
| `src/core/src/report.rs` *(new)* | `BatchReport`, `MatchupReport`, `CombatantStats`, all `Serialize` | Yes |
| `src/core/src/lib.rs` | Export the two new modules | Yes |
| `src/core/src/bin/balance.rs` *(new)* | Thin CLI: parse args, call `run_batch`, print a table or `--json` | Yes, plus `--only`, `--no-fail` and external-content flags |
| `src/core/tests/balance_bounds.rs` *(new)* | Assert every shipped matchup lands inside its declared win-rate band | Yes, five tests |
| `data/matchups.json` *(new)* | Declared encounters: party, foes, seed list, acceptable win-rate band | Yes |
| `src/data-pipeline/validate_data.py` | Validate `matchups.json`: ids resolve, bands are ordered and within 0..100 | Yes, plus duplicate seeds, both-sides membership and orphan combatants |
| `.github/workflows/ci.yml` | New `balance` job running the bounds test and uploading the JSON report | Job yes, **artefact no** |
| `data/*.json` | Rebalanced numbers, driven by harness output | Yes, five stat changes and one new enemy |
| `CHANGELOG.md`, `README.md` | Record the harness and the measured win rates | Yes |

Two deviations, both worth stating plainly:

1. **The plan contained no rules change, and the milestone turned on one.** Every
   finding reduced to `ai.rs` targeting the lowest-HP enemy unconditionally,
   which made a damage advantage compound and killed the same party member in 12
   of 12 battles. `FOCUS_FIRE_CHANCE = 55` is the single most consequential edit
   in `0.3.0`, and no line of this table anticipated it. A plan that survives
   contact with measurement unchanged is usually a plan that was not measured
   against.
2. **The `balance` job does not upload a JSON artefact.** The report is printed
   to the job log and `--json` exists for local use; an artefact nobody
   downloads is a cost without a reader. `balance-report.json` is gitignored so
   a local run cannot be committed by accident.

Two files were delivered that no plan mentioned: [`BALANCE-LOG.md`](BALANCE-LOG.md),
one entry per change with the prediction written before the run, and
[`BALANCE-CURVE.md`](BALANCE-CURVE.md), the swept response curve that ended six
rounds of guessing.

### Carried into M3

Closed bands are not a fixed game. These are open, and they are content or rules
problems rather than tuning ones:

- **`ai::best_offensive` filters on damage**, so five of eleven skills can never
  be chosen by any profile -- every buff, guard and area attack is unreachable
  content. Every number in `0.3.0` therefore measures a subset of the game.
- **The ambush is decided by one character.** Jotaro deals 76% of the party's
  damage and his survival rate tracks the win rate exactly across all five swept
  points. The band is closed; the roster imbalance behind it is not.
- **The Iron Brawler hits at an effective 157 against Dio's 160.** A random
  encounter should not punch within 2% of the final boss.
- **`Event::Healed` has no actor**, so healing done cannot be reported and has to
  be inferred from SP spent.
- **Twelve seeds resolve to 8.3 percentage points.** Widen the list in a commit
  of its own, never alongside a content change.

### Deliberately out of scope for `0.3.0`

Floating damage numbers, status icons and the single-entry tempo readout are all
presentation defects from the M1 playthrough. They belong to M3, and mixing them
into a balance release would make it impossible to tell whether a changed win
rate came from a number or from the UI.

## M3 - Content depth (next, `0.4.0`)

**Exit criterion:** three playable characters, six enemies, two boss fights, all
authored purely in `data/`.

Every encounter added in M3 declares a band in `data/matchups.json` and is
measured before it is called finished. The harness exists now; authoring against
intuition again would waste it.

- [ ] Give the AI a reason to use the five unreachable skills, so buffs, guards
      and area damage stop being content that no measurement can see
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
