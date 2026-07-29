# Roadmap

Milestones are ordered so that each one is independently demonstrable. For a
portfolio project, a finished vertical slice beats a half-built epic every time.

Each milestone maps to exactly one version tag. The mapping is authoritative in
[RELEASING.md](RELEASING.md); it is repeated here so the two documents can be
checked against each other.

| Milestone | Version | Exit criterion | State |
| --- | --- | --- | --- |
| M0 Foundation | `0.1.0` | Simulation tested, deterministic, reaching Godot | Released |
| M1 Playable battle loop | `0.2.0` | One battle playable start to finish | Released |
| M2 Balance harness | `0.3.0` | Headless mass simulation reporting win rates | Released |
| M3 Content depth | `0.4.0` | Full roster, statuses and elemental resistances | In progress |
| M4 Portfolio polish | `1.0.0` | A stranger can play it in under 60 seconds | Planned |

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

## M1 - Playable battle loop (done, `0.2.0`)

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

## M2 - Balance harness (done, `0.3.0`)

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

---

## M3 - Content depth (in progress, `0.4.0`)

**Exit criterion:** three playable characters, six enemies and two boss fights,
authored purely in `data/`, with every encounter declaring a win-rate band that
the harness measures and CI enforces.

One addition to the original criterion, and it is not negotiable: **no skill may
be unreachable.** `0.3.0` shipped eleven skills of which the AI could choose
eight. The three it could never choose are named, because a count invites
re-counting and a name can be checked: `skill.guard_stance` and `skill.rage_focus`
deal no damage at all, and `skill.tempo_halt` deals 60 to every enemy but is
dominated by a 140-power single-target slam in the same list. Adding content on
top of an AI that cannot use part of it would multiply the blind spot instead of
closing it.

### Order of work, and why this order

Rules first, seeds second, content third, presentation last. The reason is
mechanical rather than stylistic: **RNG draw order is part of the rules**, so the
moment `ai.rs` or `resolve.rs` changes, every win rate measured before it becomes
incomparable -- the same seed no longer produces the same battle. Authoring
content before the rules settle means measuring it twice and trusting neither
number.

1. **Make every skill reachable.** `ai::best_offensive` ranked candidates by total
   damage, so a skill with no damage effect could not be a candidate at all, and
   an area attack was worth the same as a single-target one. Replace it with a
   scored choice that can value a buff, an area attack against two or more living
   targets, a tempo lock, and a guard at low HP. Expect the three existing win
   rates to move; re-measure and argue any band change in
   [BALANCE-LOG.md](BALANCE-LOG.md) rather than widening bands to fit.
2. **Apply elemental resistances in damage resolution.** The schema and the
   events already carry an element; `resolve.rs` ignores it. This is the last
   rules change of the milestone, so it lands immediately after step 1 and the
   two are re-measured together.
3. **Attribute healing.** `Event::Healed` has a target and no actor, so healing
   done cannot be reported and support characters are invisible in the report.
   Adding the actor is a one-field change that makes a whole class of content
   measurable, and it must precede any support-focused character.
4. **Widen the seed list, in a commit that changes nothing else.** Twelve seeds
   resolve to 8.3 percentage points, which is coarser than several of the
   decisions taken in `0.3.0`. Twenty-four seeds halve that. Do this alone, so
   the only thing that can explain a moved number is the resolution.
5. **Then the content**, one entity per commit, each with its declared band and
   a log entry: a third playable character, three more enemies to reach six, and
   a second boss fight.
6. **Then the presentation defects** carried from M1: floating damage numbers,
   status icons with durations, and a tempo readout that stays legible when one
   combatant is left. These cannot be verified by CI -- no job in this repository
   runs Godot -- so each is closed by a recorded playthrough, exactly as M1 was.

### Checklist

- [ ] AI can choose buffs, guards and area attacks; zero unreachable skills
- [ ] Elemental resistances applied in `resolve.rs` and covered by a unit test
- [ ] `Event::Healed` carries an actor; `CombatantStats` reports healing done
- [ ] Seed list widened to 24 in an isolated commit, bands re-measured
- [ ] Third playable character, authored in `data/` only
- [ ] Six enemies total, each exercised by at least one declared encounter
- [ ] Second boss fight with its own band
- [ ] Floating damage numbers, status icons, legible tempo readout
- [ ] `BALANCE-CURVE.md` re-swept after the rules changes, with the stale curve
      marked rather than deleted
- [ ] A recorded playthrough of both boss fights, console clean

### Planned changes, by file

Written before the work, so it can be scored honestly afterwards the way M2's
was.

| Path | Change |
| --- | --- |
| `src/core/src/ai.rs` | Replace `best_offensive` with a scored choice covering buffs, guards, tempo locks and area attacks; keep every draw on the battle RNG |
| `src/core/src/resolve.rs` | Apply elemental resistance to computed damage; add the rounding rule to the module docs |
| `src/core/src/event.rs` | `Event::Healed` gains an `actor` field |
| `src/core/src/report.rs` | `CombatantStats` gains healing done; fix `miss%` so an area skill counts one action, not one per target |
| `data/matchups.json` | 24 seeds; two more encounters, including the second boss |
| `data/combatants.json`, `data/stands.json`, `data/skills.json` | Third playable character, three enemies, the skills and stands they need |
| `src/data-pipeline/validate_data.py` | Warn on a skill no combatant can use, mirroring the existing orphan-combatant warning |
| `src/game/battle_view.gd` | Floating numbers, status icons with durations, tempo readout fix |
| `docs/BALANCE-CURVE.md` | Re-sweep; the current curve describes rules that step 1 replaces |
| `docs/BALANCE-LOG.md` | One entry per change, prediction written before the run |

### What this is expected to break

A risk register is worth more than a wish list, because these are the issues that
will actually be filed. Each is stated with its symptom, so it can be recognised
rather than rediscovered.

| Risk | Symptom you will see | Response |
| --- | --- | --- |
| **The curve goes stale** the instant `ai.rs` changes | `BALANCE-CURVE.md` numbers stop reproducing; a probe sweep disagrees with the document | Re-sweep and mark the old table as describing pre-`0.4.0` rules. Do not delete it; a retracted measurement is evidence too |
| **Bands break in CI** after steps 1 and 2 | `balance_bounds` fails with `outside the declared band` on encounters nobody touched | Expected, not a regression. Re-measure, then argue each band in the log. Widening a band to make a build green is how the harness becomes decoration |
| **`miss%` was already wrong**, not about to become wrong | Miss rates inflated on anyone holding an area skill | **Resolved before step 1, in #8.** This row originally predicted the defect would appear "once area skills are reachable". It had already appeared: `skill.blade_volley` is Kakyoin's highest-power option and was chosen throughout `0.3.0`, so his released 20% was two accuracy rolls counted as one action. He measures 11% once rolls are counted per roll. The released `0.3.0` report is wrong in that one column and stays on the record |
| **Stalemates** as statuses and heals multiply | `BattleOutcome::Stalemate`, or `no_encounter_stalls` failing on the 500-turn limit | Treat as a content defect first: an encounter that cannot end is unbalanced, not merely slow. Raise the limit only with evidence |
| **New skills are unaffordable** and quietly never used | A skill appears in `data/` but never in any report | The new validator warning catches it. An unused skill is unmeasured content, which is the exact problem M3 exists to remove |
| **A scored buff is still declined** even after step 1 | Zero unreachable skills was the goal, and a buff remains unchosen | Read the arithmetic before touching the scorer. `skill.rage_focus` returns 0.8 of a hit for the price of one, so declining it is correct behaviour on wrong numbers. The fix is in `data/skills.json`, under step 5, with a log entry |
| **Godot layer regressions** invisible to CI | Nothing fails; the game misbehaves when played | Every UI item closes on a recorded playthrough, with the console output kept |
| **The 2^53 seed ceiling** resurfaces when seeds are widened | A battle launched from GDScript does not replay | Keep every seed well below 2^53, or pass it across the boundary as a string |

Issue handling, labels and the reproduction a balance report must contain are in
[TRIAGE.md](TRIAGE.md).

### Carried in from M2, still open

- **The ambush is decided by one character.** Jotaro deals 76% of the party's
  damage and his survival rate tracks the win rate exactly across all five swept
  points. The band is closed; the roster imbalance behind it is not.
- **The Iron Brawler hits at an effective 157 against Dio's 160.** A random
  encounter should not punch within 2% of the final boss. The third playable
  character and the second boss both change the frame this sits in, so the
  decision waits for them rather than being taken twice.

### Explicitly not in `0.4.0`

Overworld navigation, party management, equipment, levelling, saves and dialogue
are all real features and none of them are content depth. They stay listed here
so the milestone cannot quietly absorb them:

- [ ] Overworld or node-based map navigation
- [ ] Party management, equipment, levelling
- [ ] Save/load (serialise the whole state; determinism makes this cheap)
- [ ] Dialogue system fed from `data/`

---

## M4 - Portfolio polish (`1.0.0`)

**Exit criterion:** a stranger can play it in under 60 seconds from the repo.

- [ ] Headless Godot export in `release.yml`, so a release contains a runnable
      game and not only a library
- [ ] Web export playable in-browser, linked from the README
- [ ] 30-second gameplay GIF in the README
- [ ] Audio: SFX per event type, one battle track
- [ ] Write-up of the determinism design with a replay demo
- [ ] `v1.0.0-rc.1` published as a pre-release before the final tag

## Explicit non-goals

Scope discipline is the whole game. These are out until `1.0.0` ships:

- Multiplayer or netplay
- Procedural content generation
- 3D
- Mod support / plugin API
- Mobile ports
