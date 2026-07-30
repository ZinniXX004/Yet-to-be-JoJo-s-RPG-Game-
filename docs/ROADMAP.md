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

These are the figures as released. They no longer reproduce: M3 steps 1 and 2
changed the rules, so the same seeds now produce different battles. The
re-measured series is below.

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

That conclusion was right and incomplete, and the incompleteness cost most of
M3's step 1. SP was binding because it was **never returned**: a pool was a
one-off budget for a whole battle, so in a 52-turn boss fight every combatant
spent its interesting options in the first few turns and threw basic attacks for
the rest. See M3 step 1 below.

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
eight. Naming them rather than counting them turned out to matter, because the
count moved twice while the work was done:

- `skill.tempo_halt` was called unreachable because a 140-power slam dominated
  it. That reading was wrong in a way only measurement could show: it is now
  Dio's opening move, 44 uses per batch, and it was unreachable because the old
  chooser ranked raw damage, not because its numbers were weak.
- `skill.blade_volley` was never unreachable at all under the old rules -- it was
  *dominant*, which is why the `miss%` defect in #8 was already live on shipped
  content. It went briefly unused at 40 SP and is reachable at 36.
- `skill.guard_stance` (potency 60) and `skill.rage_focus` (potency 40 for 10 SP)
  are still declined, now for a reason with arithmetic behind it: both are worth
  less than hitting something once, and two unit tests pin that conclusion at
  the numbers the game currently ships. These are content problems, fixed under
  step 5, not AI problems.

Current content is **twelve skills, ten reachable**, after
`skill.emerald_splash` was added so Hierophant Green stops borrowing Dio's
thrown knives. Adding content on top of an AI that cannot use part of it would
multiply the blind spot instead of closing it.

### Order of work, and why this order

Rules first, seeds second, content third, presentation last. The reason is
mechanical rather than stylistic: **RNG draw order is part of the rules**, so the
moment `ai.rs` or `resolve.rs` changes, every win rate measured before it becomes
incomparable -- the same seed no longer produces the same battle. Authoring
content before the rules settle means measuring it twice and trusting neither
number.

1. **Make every skill reachable.** `ai::best_offensive` ranked candidates by total
   damage, so a skill with no damage effect could not be a candidate at all, and
   an area attack was worth the same as a single-target one. Replaced with a
   scored choice that values buffs, area attacks, tempo locks and guards in one
   unit -- the damage a plain attack from that actor would deal -- and that pays
   for SP at what the same SP would have bought from the actor's own remaining
   options. **Delivered; see the result below.**
2. **Apply elemental resistances in damage resolution.** The schema and the
   events already carry an element; `resolve.rs` ignores it. This is the last
   rules change of the milestone, so it lands immediately after step 1 and the
   two are re-measured together. `skill.emerald_splash` is psychic, so this step
   now has one more skill riding on it. **Delivered; see the result below.**
3. **Attribute healing.** `Event::Healed` has a target and no actor, so healing
   done cannot be reported and support characters are invisible in the report.
   Adding the actor is a one-field change that makes a whole class of content
   measurable, and it must precede any support-focused character. **Delivered;
   see the result below.**
4. **Widen the seed list, in a commit that changes nothing else.** Twelve seeds
   resolve to 8.3 percentage points, which is coarser than several of the
   decisions taken in `0.3.0` and coarser than the margin by which the boss
   fight now passes. Twenty-four seeds halve that. Do this alone, so the only
   thing that can explain a moved number is the resolution.
5. **Then the content**, one entity per commit, each with its declared band and
   a log entry: a third playable character, three more enemies to reach six, and
   a second boss fight. `skill.guard_stance` and `skill.rage_focus` are repriced
   here, because that is where the unreachable-skill criterion is actually met.
6. **Then the presentation defects** carried from M1: floating damage numbers,
   status icons with durations, and a tempo readout that stays legible when one
   combatant is left. These cannot be verified by CI -- no job in this repository
   runs Godot -- so each is closed by a recorded playthrough, exactly as M1 was.

### Step 1 as measured

Six runs of the harness separate the released rules from the current ones. The
intermediate figures are kept because the path is the evidence: a single before
and after would suggest the answer was obvious.

| Rules at | `thug_solo` | `assassin_ambush` | `dio_boss` |
| --- | --- | --- | --- |
| `0.3.0`, as released | 100% | 67% | 50% |
| Scored choice, first draft | 100% | 33% | 8% |
| Units fixed (HP vs power) | 100% | 33% | 8% |
| SP priced at its best other use | 100% | 58% | 0% |
| `emerald_splash`, volley at 40 SP | 100% | 50% | 0% |
| **SP recovery, volley at 36 SP** | **100%** | **67%** | **42%** |

Four things this table says that no plan predicted:

1. **The choice function was not the binding constraint; the SP economy was.**
   A better chooser made the boss fight *worse* (50% to 8%), because scoring
   correctly means spending correctly, and there was nothing to spend after the
   first few turns. Dio's pool of 200 bought four tempo halts and then nothing
   for forty turns.
2. **Nothing in the engine ever returned SP.** `state::SP_REGEN_PER_TURN = 4`,
   applied on a combatant's own turn and capped at its starting pool, is the fix
   and it is the largest unplanned change of the milestone. It is charged per
   turn rather than per tick so the engine keeps one notion of "a turn", and it
   is flat rather than proportional because 4 is 5% of Jotaro's pool and 2% of
   Dio's -- deliberately anti-boss.
3. **`thug_solo` never moved, through all six runs.** Not because nothing
   changed, but because a three-turn fight cannot exhaust a pool, so no SP rule
   can reach it. A figure that does not move is worth as much as one that does.
4. **The boss now passes at 42% against a floor of 35**, a margin of 7 points
   where one battle is worth 8.3. It is inside its band and it is not decisively
   inside it, which moves step 4 up in importance.

All figures published before the last row of that table are void, not merely
old: three of the changes touch `ai.rs`, `battle.rs` or `state.rs`.

### Step 2 as measured

One gate run separates step 1 from the post-resistance rules.

| Rules at | `thug_solo` | `assassin_ambush` | `dio_boss` |
| --- | --- | --- | --- |
| Step 1 final (`7f72a4f`) | 100% | 67% | 42% |
| **Step 2 -- resistances applied** | **100%** | **50%** | **42%** |

Three things this table says:

1. **`thug_solo` is bit-identical in every column.** The Street Thug carries no
   resistance table and no skill in that fight deals a typed element, so the
   report is a direct falsifiability check: if anything moved here, the change
   reached further than `resolve.rs`.
2. **The ambush fell 67 -> 50.** The Iron Brawler carries `physical: 25` (cuts
   physical damage by 25%) and `psychic: -25` (amplifies psychic damage by 25%).
   The fall happened even though Kakyoin's psychic skills *should* benefit from
   the vulnerability: the AI scorer does not read resistance tables, so it does
   not prefer them. The 17-point drop is the cost of Jotaro's physical barrage
   being blunted, uncompensated by any routing toward the Brawler's weakness.
   The band still holds at 50%.
3. **The boss is unchanged at 42%.** Every individual row moved (Dio dealt/b
   2486 -> 2392 as Jotaro's `temporal: 50` cuts halt and drain; Jotaro dealt/b
   1413 -> 1466), but the win rate sat on the same integer. The instrument's
   8.3-point resolution is the explanation.

Prediction scorecard for the step 2 gate:

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `thug_solo` bit-identical | bit-identical in every column | correct |
| 2 | Validator 0 errors / 0 warnings | 0 errors, 0 warnings | correct |
| 3 | 67 tests pass | 67 tests pass (54 lib + 5 bin + 5 bounds + 3 det.) | correct |
| 4 | `balance_bounds` 5/5 | 5/5 | correct |
| 5 | balance digit-identical to previous run | digit-identical | correct |

All five correct. The reason is mechanical: the fixture fixes do not touch any
production code path, so the only thing the second run was testing was whether
the compile error was the sole failure. It was.

### Step 3 as measured

| Rules at | `thug_solo` | `assassin_ambush` | `dio_boss` |
| --- | --- | --- | --- |
| Step 2 final (`f6ba445`) | 100% | 50% | 42% |
| **Step 3 -- healing attributed** | **100%** | **50%** | **42%** |

**Every column in all three tables is unchanged**, not only the win rates:
`dealt/b`, `taken/b`, `sp/b`, `rolls`, `miss%` and `alive%` reproduce digit for
digit across fifteen combatant rows, and every `actions by skill` line is
identical. That was the stated pass condition rather than a hoped-for outcome.
No RNG is drawn by adding a field to an event, so any movement at all would have
meant the change reached somewhere it had no business reaching, and the run
would have been a failure regardless of which direction the number went.

This is the first step of the milestone that opens **no new comparability
boundary**. `BALANCE-LOG.md` has five; step 3 adds none.

What the new column showed on its first run is the whole point of the step:

1. **Josuke restores 1044 HP per battle while dealing 358.** The healer's real
   output is close to three times its damage, and against 2740 HP of incoming
   damage per battle it undoes 38% of everything the enemy does. Under the old
   report Josuke was the party's weakest-looking member; it was second only to
   Jotaro in contribution and the table could not say so.
2. **Dio self-heals 183 per battle through `blood_drain`.** Sustain that never
   appeared anywhere, on a boss whose printed pool is 1520. Roughly 12% of its
   effective durability was invisible, which means every previous estimate of
   how much damage the party needs to win the boss fight was low.
3. **The Iron Brawler heals 6 per battle** from three `blood_drain` uses across
   the whole batch -- negligible, and worth recording precisely because it is
   negligible. A column that only ever printed large numbers would be a column
   nobody checked.
4. **`0.3.0`'s estimate was close and low.** The `skill.restore` nerf was argued
   against "roughly 950 HP per battle", inferred from SP spent. Measured
   directly, it is 1044 -- the inference was sound, and 9% short.

Prediction scorecard for the step 3 gate:

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | 72 tests (59 lib + 5 bin + 5 bounds + 3 det.) | exactly 72 | correct |
| 2 | All three win rates and every table figure unchanged | unchanged, digit for digit | correct |
| 3 | Josuke's `heal/b` non-zero | 1044 | correct |
| 4 | Dio's `heal/b` non-zero via `blood_drain` | 183 | correct |
| 5 | No new inert warning | none | correct |

Five of five. That is a weaker result than it looks: predicting that a change
which draws no RNG will move no number is close to predicting arithmetic. The
predictions worth having in this step were the two structural ones made *before*
any code -- that regeneration could not be given an honest actor and needed its
own variant, and that the Godot bridge reads fields by name so only a new
*variant* could break it. Both held, and both were checked against the source
rather than guessed.

Running total across the milestone: **thirty-six predictions, twenty-one wrong.**

### Checklist

- [x] AI can choose buffs, guards and area attacks; zero unreachable skills
      *(scored choice delivered; two skills still declined on their own numbers,
      which step 5 fixes)*
- [x] Elemental resistances applied in `resolve.rs` and covered by a unit test
- [x] `Event::Healed` carries an actor; `CombatantStats` reports healing done
      *(and `Event::StatusHealed` added, which the plan did not contain)*
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
was. The `Delivered` column is filled in as each lands.

| Path | Change | Delivered |
| --- | --- | --- |
| `src/core/src/ai.rs` | Replace `best_offensive` with a scored choice covering buffs, guards, tempo locks and area attacks; keep every draw on the battle RNG | Yes, plus an SP price the plan did not contain |
| `src/core/src/resolve.rs` | Apply elemental resistance to computed damage; add the rounding rule to the module docs | Yes, plus three unit tests |
| `src/core/src/event.rs` | `Event::Healed` gains an `actor` field | Yes, **plus `Event::StatusHealed`**, which the plan did not contain and which the actor field made unavoidable |
| `src/core/src/report.rs` | `CombatantStats` gains healing done; fix `miss%` so an area skill counts one action, not one per target | Yes to both, plus per-skill action accounting the plan did not contain, plus an `inert_combatants` correction the plan did not foresee |
| `data/matchups.json` | 24 seeds; two more encounters, including the second boss | Steps 4 and 5 |
| `data/combatants.json`, `data/stands.json`, `data/skills.json` | Third playable character, three enemies, the skills and stands they need | Partly: `skill.emerald_splash` added and `skill.blade_volley` repriced (step 1); resistance tables for four combatants (step 2) |
| `src/core/src/state.rs`, `src/core/src/battle.rs` | *Not planned.* SP recovery per turn, without which step 1 makes the boss fight unwinnable | Yes; `battle.rs` also carried the regeneration fix in step 3 |
| `src/core/src/data.rs` | *Not planned.* `Resistances` type, `MAX_RESISTANCE`, `MIN_RESISTANCE`, `CombatantDef.resist` | Yes (step 2) |
| `src/core/src/sim.rs` | *Not planned.* Credit healing to the healer in `accumulate`, and deliberately credit `StatusHealed` to nobody | Yes (step 3) |
| `src/data-pipeline/validate_data.py` | Warn on a skill no combatant can use, mirroring the existing orphan-combatant warning | Not yet; cwd-relative default path was fixed instead; resistance validation added in step 2 |
| `src/game/battle_view.gd` | Floating numbers, status icons with durations, tempo readout fix | Step 6, but step 3 landed here early: `status_healed` needed a case or every regen tick would have printed raw JSON |
| `docs/BALANCE-CURVE.md` | Re-sweep; the current curve describes rules that step 1 replaces | Pending; the curve is now invalidated by five separate changes |
| `docs/BALANCE-LOG.md` | One entry per change, prediction written before the run | Yes, with a scorecard per run |

### What this is expected to break

A risk register is worth more than a wish list, because these are the issues that
will actually be filed. Each is stated with its symptom, so it can be recognised
rather than rediscovered.

| Risk | Symptom you will see | Response |
| --- | --- | --- |
| **The curve goes stale** the instant `ai.rs` changes | `BALANCE-CURVE.md` numbers stop reproducing; a probe sweep disagrees with the document | **Fired.** Five changes invalidate it, not one. Re-sweep and mark the old table as describing pre-`0.4.0` rules. Do not delete it; a retracted measurement is evidence too |
| **Bands break in CI** after steps 1 and 2 | `balance_bounds` fails with `outside the declared band` on encounters nobody touched | **Fired, four runs in a row during step 1**. Not a regression. Steps 2 and 3 did not fire this risk: all three bands held |
| **`miss%` was already wrong**, not about to become wrong | Miss rates inflated on anyone holding an area skill | **Resolved before step 1, in #8.** Released `0.3.0` Kakyoin at 20% was two accuracy rolls counted as one action; true rate is 11% |
| **Stalemates** as statuses and heals multiply | `BattleOutcome::Stalemate`, or `no_encounter_stalls` failing on the 500-turn limit | Not yet fired, and closer than it was: boss fight runs 54 turns median against a 500-turn limit |
| **New skills are unaffordable** and quietly never used | A skill appears in `data/` but never in any report | **Fired, by my own hand.** `skill.blade_volley` at 40 SP went unused across twelve boss battles |
| **A scored buff is still declined** even after step 1 | Zero unreachable skills was the goal, and a buff remains unchosen | **Fired, as predicted.** `skill.rage_focus` returns 0.8 of a hit for the price of one; declining it is correct on wrong numbers. Fix is in `data/skills.json`, step 5 |
| **Adding a field to `Event` breaks every exhaustive match** | Compile errors across the crate and the bridge | **Fired and contained in step 3.** The field was the easy half; the new *variant* was the risk this row did not name. See the row below |
| **A new `Event` variant is silently unhandled by the presentation layer** | Nothing fails to compile; the Godot log prints raw JSON where a sentence should be | **Fired in step 3 and caught before merge**, by reading `battle_view.gd` rather than trusting that a compiling bridge means a correct one. GDScript matches on a string and reads fields by name, so it cannot fail loudly. A note now sits in that file's header stating the rule |
| **Godot layer regressions** invisible to CI | Nothing fails; the game misbehaves when played | Every UI item closes on a recorded playthrough, with the console output kept |
| **The 2^53 seed ceiling** resurfaces when seeds are widened | A battle launched from GDScript does not replay | Keep every seed well below 2^53, or pass it across the boundary as a string |

Issue handling, labels and the reproduction a balance report must contain are in
[TRIAGE.md](TRIAGE.md).

### Carried in from M2, still open

- **The ambush is decided by one character.** Jotaro deals 76% of the party's
  damage and his survival rate tracks the win rate exactly. The band is closed;
  the roster imbalance behind it is not. Step 2 confirmed it: ambush fell
  67 -> 50 because Jotaro's physical barrage was blunted without any compensating
  routing toward the Brawler's psychic vulnerability.
- **The Iron Brawler hits at an effective 157 against Dio's 160.** The third
  playable character and the second boss both change the frame this sits in, so
  the decision waits for them rather than being taken twice.
- **Kakyoin is the weakest link in both encounters he appears in.** 201 damage
  per battle in the boss fight against Jotaro's 1466, and 8% survival in both.
  Step 3 removed the last excuse for this reading: Kakyoin's `heal/b` is 0, so
  unlike Josuke there is no hidden contribution the table was failing to show.
  Step 5 has to answer this with numbers, not with another skill.
- **The SP economy is now the loudest unexplained number.** Dio spends 250 SP
  per battle against Jotaro's 118 and Kakyoin's 140, on a pool of 200 that
  refills at 4 per turn. Filed as its own issue rather than folded into step 5,
  because it is a rules question and step 5 is content.

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
