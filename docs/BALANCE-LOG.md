# Balance log

Every balance change to `data/*.json`, the measurement that motivated it, and the
measurement that followed it. One entry per change.

The rule this file exists to enforce: **change one number, re-run, record.** Two
numbers at once and the result is uninterpretable, because either change could
have produced it. This is slower and it is the only version that produces
knowledge rather than opinion.

Predictions are written before the run, not after. A prediction recorded after
the fact is a rationalisation, and the running record through `0.4.0` step 5 is
**fifty-four predictions, twenty-nine of them wrong**. The scorecards are kept
per entry so the rate is visible rather than asserted. That is the argument for
the harness, not against it -- the same wrong guesses shipped as content,
unmeasured, would have been indistinguishable from design.

All numbers come from:

```powershell
cd src
cargo run -q -p rpg-core --bin balance
```

over the fixed seed list in [`data/matchups.json`](../data/matchups.json).
Seeds are fixed precisely so two rows of this table can be compared.

> **How to read a win rate in this file.** Every figure recorded here is an
> estimate from a finite sample, and until issue #12 this file consistently
> understated how coarse that estimate was. See
> [the #12 entry](#issue-12----the-seed-list-widens-and-the-instrument-turns-out-to-have-been-misread)
> before comparing any two numbers. In short: readings taken at twelve seeds
> carry roughly **+/-14 points** of standard error, not 8.3, and most of the
> differences argued over in the entries below are inside that.

> **Comparability boundary 1.** Change C alters `ai::choose`, which changes both
> the decisions taken and the order in which the RNG is drawn. Every number
> recorded above that entry is historical. Do not compare it to anything below.

> **Comparability boundary 2.** The `0.4.0` scored-choice change (issue #9)
> alters which skill `ai::choose` selects. Every number recorded *above* that
> entry describes rules in which three skills could never be chosen. Do not
> compare across it either.

> **Comparability boundary 3 -- what the earlier bands were really measuring.**
> Issue #9 established that the pre-`0.4.0` AI did not merely ignore three
> skills: it compared the ones it did use in **mixed units**, and it spent SP as
> though SP were free. Every band in this file above the #9 entry was therefore
> fitted against an opponent that misplayed in a specific, now-corrected way.
> `matchup.dio_boss` at 50% was not a boss fight tuned to 50%; it was a boss who
> could not price his own signature skill. Those bands are not a baseline to
> return to.

> **Comparability boundary 4 -- SP now comes back.** Issue #9 also found that no
> combatant ever recovered a single point of SP in the entire history of this
> project. `SP_REGEN_PER_TURN = 4` is new in `state.rs`. Every band, every ratio
> and the whole curve in [`BALANCE-CURVE.md`](BALANCE-CURVE.md) were measured in
> an engine where a combatant's lifetime output was hard-capped by its starting
> pool.

> **Comparability boundary 5 -- elemental resistances now modify damage.** Issue
> #10 adds resistance lookups to `resolve::compute_damage`. Any figure measured
> before `feat/element-resistance` was built on a flat damage model where every
> element hit for the same amount regardless of target. Do not compare across
> this boundary. The `thug_solo` control remained bit-identical because no
> combatant in that fight carries a resistance table and no skill in it deals a
> typed element, which is why the Street Thug is the explicit control row.

> **There is no boundary 6, and issues #11, #12 and #14 are the reason it matters
> that there is not.** #11 changed reporting only; #12 changed the sample size
> only; #14 changed instrumentation that no build reads. None touched a rule, so
> every figure in this file remains an estimate of the same underlying quantity
> as before. What changed at #12 is how *precisely* those quantities are known,
> which is not the same thing as the quantities having moved. A boundary marks
> "these numbers describe a different game"; a wider sample marks "these numbers
> describe the same game, better".

## Current status

After issue #12, commit `1f5aaed`, re-measured over **300 seeds**:

| Matchup | Win rate | 95% interval | Band | Status |
| --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | -- | 85..100 | ok |
| `matchup.assassin_ambush` | 53% | 47..59 | 45..90 | ok |
| `matchup.dio_boss` | 47% | 41..53 | 35..75 | ok |

All three encounters are inside their declared bands, and for the first time in
this project **the intervals are inside the bands too**, rather than merely the
point estimates. `matchup.dio_boss` is no longer the marginal row it was
reported to be at twelve seeds; that appearance was an artefact of the sample.

Issue #14 reproduced the `assassin_ambush` row independently, from a separate
roster file on a separate branch, in every digit. Two different files describing
the same combatants produce the same 1800-event batch, which is a stronger
statement about determinism than any single run can make.

## The throughput model

Added after Change E, because five of six failed predictions failed for the same
reason: they estimated "damage per turn" by intuition. The engine is not
mysterious, so the estimate does not have to be. From `resolve.rs`:

```
hit = max(power * atk / 100 - def / 2, 1)   then variance, then crit x 3/2
```

and a "turn" in the report is **one action by one combatant**, not a round. The
tempo scheduler hands out actions in proportion to speed, so for a batch:

```
actions_i = T * spd_i / sum(spd)                       (share of a T-turn fight)
T         = sum(foe HP) / (party damage per action * party speed share)
D_enemy   = T * sum over foes of (spd_i / sum(spd)) * damage per action_i
```

Calibrated against the Change E run of `matchup.assassin_ambush`: speeds are
Jotaro 102, Kakyoin 86, Assassin 94, Brawler 72, so the party takes 53% of the
actions; party damage per action is 1349 / 10.0 = 134; foe HP is 640 + 700 =
1340; T = 1340 / 71 = **18.8** against a measured median of **19**, and D = 18.8
x 32.4 = **609** against a measured **617**.

**Change F tested it out of sample and it held**: predicted 122 damage per action
against a measured 116, predicted 468 per battle against a measured 428, predicted
800 enemy damage against a measured 736. Errors of 5-9%, all in the same
direction, because the model uses average party defence and the AI actually
focus-fires the softer target.

**The sweep in [`BALANCE-CURVE.md`](BALANCE-CURVE.md) then found its ceiling.**
Predicted against measured enemy damage per battle at `atk` 135, 165, 195 and
225: -5%, -11%, -14%, -13%. The error grows with attack because **SP is a hard
ceiling** -- `sp/b` reads 57 on every row of the sweep, so beyond that budget the
extra attack only scales `skill.strike` and damage grows more slowly than the
stat does. Use the model to size a change in ratio; do not trust it to convert a
stat into damage past the point where a combatant runs out of SP.

> **Amended by issue #9.** That SP ceiling was real but it was also partly a
> bug: SP never regenerated, so "the budget" was the starting pool for the whole
> battle no matter how long the battle ran. With `SP_REGEN_PER_TURN = 4` the
> ceiling is now a *rate* rather than a total, and it scales with fight length.
> Measured consequence in `matchup.dio_boss`: Dio's `sp/b` went 191 -> **252**
> from a pool of 200. Any sizing done with this model before `7f72a4f`
> understates long-fight output.

> **Amended by issue #10.** The formula above now includes a resistance modifier:
> `max(damage * (100 - resist) / 100, 1)`. The model underestimates damage
> against a resistant target and overestimates it against a vulnerable one by the
> resistance percentage. Calibrate against a post-#10 run before using the model
> to size a change that targets a resistant or vulnerable enemy.

> **Amended by issue #12.** Every calibration above was fitted against a
> twelve-seed run, so each "measured" figure it was checked against carries about
> 14 points of sampling error on win rate and a smaller but real error on the
> per-battle averages. The agreements of 5-9% quoted above are therefore better
> than the data could actually support -- they are partly luck. The model is
> still the right way to size a change; the confidence it earned from those
> checks was overstated.

> **Amended by issue #14, and the SP-ceiling explanation is withdrawn.** The
> re-sweep at 300 seeds reads `sp/b` as 64, 66, 67, 64, 61, 56 across the range.
> It is not pinned at 57 or at anything else, because SP regen arrived after the
> original sweep. The model's error is no longer a uniform over-prediction; it is
> a **slope error**, 15% low at `atk` 135 and 15% high at `atk` 225, crossing zero
> near 175. The cause is battle length: per-*turn* output is `0.22 x atk` to
> within 4% across a threefold range, while battle length falls from 22 turns to
> 16. A model that converts a stat into per-battle damage without modelling
> battle length must get the slope wrong. **Estimate per-turn output first, then
> multiply by expected length.**

The two things it makes obvious, both of which the Change E prediction missed:

- **A slow foe is a cheap foe.** The Brawler's 72 speed buys it 20% of the
  actions in the fight, so its 73 damage per action becomes 285 per battle.
  Speed multiplies damage as directly as attack does.
- **A foe's output is capped by its SP, not by the clock.** *(True until
  `7f72a4f`; withdrawn entirely by issue #14 -- see the amendment above. The
  clock is now the binding constraint.)*

---

## Baseline -- 2026-07-28, commit `0fcfd86`

First measurement of the shipped `0.2.0` content. No changes yet.

| Matchup | Win rate | Band | Verdict |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 | ok |
| `matchup.assassin_ambush` | 100% | 45..90 | out of band |
| `matchup.dio_boss` | 92% | 35..75 | out of band |

`matchup.dio_boss`, per battle:

| Combatant | Dealt | Taken | SP | Miss | Alive |
| --- | --- | --- | --- | --- | --- |
| Jotaro | 1241 | 423 | 72 | 6% | 75% |
| Josuke | 412 | 113 | 83 | 10% | 92% |
| Kakyoin | 290 | 1166 | 134 | 12% | 0% |
| Dio | 1564 | 1462 | 155 | 6% | 8% |
| Flame Assassin | 137 | 480 | 22 | 19% | 0% |

### What the baseline says

1. **The party is winning, not losing.** The first manual playthrough ended in a
   defeat, which made the content look brutally hard. Over 12 unattended battles
   the party wins 11. One playthrough was never evidence; this is the first
   number in the project that is.

2. **Kakyoin dies in every single battle** and absorbs 1166 of the roughly 1700
   damage the enemy side deals -- 63% of it, on the lowest HP pool in the party.
   This is not a content accident. `ai::choose` focus-fires the enemy with the
   lowest current HP, so every hostile actor picks Kakyoin on turn one and keeps
   picking Kakyoin. A party member who is guaranteed to die is a rules problem,
   not a tuning problem, and no HP number fixes it.

3. **Flame Assassin is not a participant.** It deals 137 damage per battle in the
   boss fight and 318 in the ambush, dying in both. Its 480 HP is below
   Kakyoin's 520, so the party's own focus-fire rule deletes it before it acts
   more than a few times.

4. **Dio's SP economy is real, and it is exhausted.** 155 SP spent per battle
   against a pool of 160 plus his stand bonus. The manual playthrough showed him
   spending 22 SP total, which looked like proof that SP was decorative; that
   reading was wrong. The fight simply ended before he could spend it.

5. **Josuke is the reason the party survives.** He takes 113 damage per battle
   and spends 83 SP. At `skill.restore`'s 230 healing for 20 SP, that is roughly
   950 HP returned per battle -- an entire extra party member, and more than half
   of what the enemy side deals.

   > **Superseded by issue #11, which measured it directly rather than inferring
   > it from SP.** The estimate above was arrived at by dividing SP spent by the
   > skill's cost, because nothing in the report could attribute a heal to a
   > healer. Measured: **1044 HP per battle** at twelve seeds, **1088** at three
   > hundred. The inference was sound in method and roughly 9% low. The original
   > figure stays on the record because the point of this file is what was
   > believed when a decision was taken -- the `skill.restore` nerf in Change B
   > was argued against 950, not against 1088.

---

## Change A -- Flame Assassin hp 480 -> 640

Commit `4f21d1f`.

**Motivation:** finding 3. The Assassin is priced as a threat and behaves as a
speed bump, because the party's focus-fire rule targets the lowest HP pool and
480 is below Kakyoin's 520.

**Prediction:** `matchup.assassin_ambush` drops out of 100%. `matchup.dio_boss`
drops by less.

**Result: the prediction was wrong in both directions.**

| Matchup | Before | After | Predicted |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | unchanged |
| `matchup.assassin_ambush` | 100% | **100%** | drop |
| `matchup.dio_boss` | 92% | **83%** | small drop |

### Why HP could never have fixed the ambush

Lifetime damage is DPS multiplied by lifetime, and adding HP only touches the
second factor. In the ambush the party out-damages the enemy side by roughly
2.3 to 1, so lengthening the fight lengthens it for both sides and leaves the
ratio untouched. To close a 2.8x shortfall with HP alone, the Assassin would
need roughly 1790 HP.

### The structural finding

Both sides focus-fire the lowest-HP target, so the side with more damage per turn
deletes the opposing roster one member at a time while the loser's output decays
with every death. No number in `data/*.json` fixes this. It is `ai::choose`.

---

## Change B -- Restore heal power 230 -> 140

Commit `b13e8f9`.

**Prediction:** `matchup.dio_boss` lands between 60% and 78%.

**Result: no movement whatsoever.** 83% before, 83% after.

| Matchup | Before | After |
| --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% |
| `matchup.assassin_ambush` | 100% | 100% |
| `matchup.dio_boss` | 83% | **83%** |

**Conclusion: healing magnitude is a third-order lever in this encounter, and
targeting is the first-order one.**

> **Revisited after issue #11.** This conclusion was drawn from a single
> unchanged win rate at twelve seeds, where the standard error is around 14
> points -- so "no movement whatsoever" means only that the change did not move
> the number by more than the noise floor. Issue #11 measured what the skill
> actually does: 1088 HP per battle, offsetting 38% of everything the enemy side
> deals in the boss fight. A lever that large being invisible in the win rate is
> a statement about the encounter's shape, not about the lever. Whether 140 is
> the right potency is issue #24.

---

## Change C -- weighted target selection in `ai::choose`

Commit `87c8220`. **This is a rules change, not a content change.**

**The change:** `FOCUS_FIRE_CHANCE = 55`. Every hostile action now commits to
the weakest enemy 55% of the time and picks uniformly random otherwise.

**Prediction:** `thug_solo` 100%; `assassin_ambush` 80-95%; `dio_boss` 70-88%.

**Result: one of three predictions correct. The boss fight went up, to 100%.**

| Matchup | Before | After | Predicted |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 100% -- correct |
| `matchup.assassin_ambush` | 100% | **100%** | 80-95% -- wrong |
| `matchup.dio_boss` | 83% | **100%** | 70-88% -- wrong, and above the range |

`matchup.dio_boss`, per battle, before -> after:

| Combatant | Dealt | Taken | SP | Alive |
| --- | --- | --- | --- | --- |
| Jotaro | 1284 -> 1304 | 547 -> **422** | 72 -> 72 | 42% -> **83%** |
| Josuke | 461 -> 446 | 273 -> 336 | 83 -> 83 | 83% -> 92% |
| Kakyoin | 295 -> **410** | 1053 -> **956** | 122 -> 144 | 0% -> **33%** |
| Dio | 1665 -> **1474** | 1400 -> **1520** | 177 -> 176 | 17% -> **0%** |
| Flame Assassin | 209 -> 215 | 640 -> 640 | 33 -> 26 | 16% -> 0% |

### The ambush is not reachable by any combatant stat

Enemy lifetime damage 353 + 13 = 366 against party pool 1140. The enemy side
needs roughly 3.1x its current output before the party is at risk.

---

## Change D -- Dio atk 100 -> 130

Commit `b279cda`.

**Prediction:** 78-92%.

**Result: 50%. In band, dead centre, and 28 points below the bottom of the
predicted range.**

| Matchup | Before | After | Predicted | Band |
| --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 100% | 85..100 ok |
| `matchup.assassin_ambush` | 100% | 100% | 100% | 45..90 out of band |
| `matchup.dio_boss` | 100% | **50%** | 78-92% | 35..75 **ok** |

`matchup.dio_boss`, per battle, before -> after:

| Combatant | Dealt | Taken | SP | Alive |
| --- | --- | --- | --- | --- |
| Jotaro | 1304 -> 1258 | 422 -> **541** | 72 -> 72 | 83% -> **33%** |
| Josuke | 446 -> 425 | 336 -> **763** | 83 -> 83 | 92% -> **25%** |
| Kakyoin | 410 -> 389 | 956 -> 971 | 144 -> 142 | 33% -> 8% |
| Dio | 1474 -> **2023** | 1520 -> **1431** | 176 -> 179 | 0% -> **50%** |
| Flame Assassin | 215 -> 227 | 640 -> 640 | 26 -> 28 | 0% -> 0% |

### The sensitivity finding

The fight is a race, and a race resolves as a **step function around parity**,
not as a slope. At 69% of the party's pool the enemy loses every time; at 92%
it wins half.

> **Retracted.** This entry originally named 0.8 as the start of the responsive
> zone. That number was never measured. The sweep in
> [`BALANCE-CURVE.md`](BALANCE-CURVE.md) puts the bend between 0.65 and 0.74.

> **Retracted a second time, by issue #14.** The replacement claim above -- a
> bend between 0.65 and 0.74 -- was itself read off five twelve-seed points. At
> 300 seeds there is no bend and no step function. The response is a smooth
> monotone decline from 96% to 12%, steepest where it passes 50%, with no flat
> region at either end of the measured range. "A race resolves as a step function
> around parity" is the wrong mental model; it resolves as an ordinary sigmoid,
> and every point on it is reachable by tuning.

---

## Change E -- the ambush's second foe becomes the Iron Brawler

Commits `2a9564d` (Stand), `3362d16` (combatant), `a6a906e` (roster).

**Sizing:** party throughput 940 over 13 turns = ~72 per turn; party HP 1140;
target enemy damage ~950 (ratio 0.83); Brawler designed as hp 620, atk 80,
spd 62, sp 80, carrying `stand.iron_hymn`.

**Prediction:** `assassin_ambush` 55-85%.

**Result: 100%. The encounter did not move a single point.**

| Matchup | Before | After | Predicted | Band |
| --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 100% | 85..100 ok |
| `matchup.assassin_ambush` | 100% | **100%** | 55-85% | 45..90 out of band |
| `matchup.dio_boss` | 50% | 50% | 50% | 35..75 ok |

Enemy lifetime damage 366 -> **617**, ratio **0.54**, for zero points of win rate.
The parity rule says a ratio far from 1.0 does not respond, and 0.54 is far.

---

## Change F -- Iron Brawler atk 80 -> 105

Commit `99a4c94`.

**Arithmetic:** `concussive_slam` at effective 126 atk: hit = 144, action = 122,
battle = 468, enemy = 800, ratio = 0.70.

**Prediction: 70-90%, in band.**

**Result: 100%. The first named failure mode.**

| Matchup | Before | After | Predicted | Band |
| --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 100% | 85..100 ok |
| `matchup.assassin_ambush` | 100% | **100%** | 70-90% | 45..90 out of band |
| `matchup.dio_boss` | 50% | 50% | 50% | 35..75 ok |

Enemy lifetime damage 617 -> **736**, ratio **0.65**. The model works: predicted
116 damage per action, measured 116; predicted 428 per battle, measured 428.

### The encounter has a structural ceiling, and it is Jotaro

Jotaro deals 1031 of the party's 1350 damage (76%) and has survived 100% of
every battle in every run. Any enemy sized to threaten him deletes Kakyoin first.

Four options for the design decision; one was chosen:

1. Remove Jotaro from the encounter
2. Nerf `skill.rush_barrage` (risky: shared with boss fight at 50%)
3. Accept a boss-tier ambush enemy
4. Re-declare the band as 85..100

Option 5: **measure the curve first**. See Change G.

> **Strengthened by issue #14.** "Jotaro is the structural ceiling" was inferred
> here from one run. The re-sweep measures it directly across six enemy strengths:
> his survival tracks the encounter's win rate to within three points at every
> point on the curve (93/96, 78/81, 51/53, 33/34, 19/20, 12/12), while Kakyoin's
> sits far below it throughout (48, 28, 15, 8, 6, 2). The party essentially never
> wins without Jotaro and rarely loses with him. This is now the best-supported
> claim in either document.

---

## Change G -- Iron Brawler atk 105 -> 135

Commit `e3d4a85`. **The first content change in this document that is a
measurement rather than a prediction.**

The curve (`tools/probe/`, one run, five points):

| Enemy `atk` | Foe output/battle | Ratio | Win rate |
| --- | --- | --- | --- |
| 105 (control) | 736 | 0.65 | 100% |
| **135** | **847** | **0.74** | **67%** |
| 165 | 925 | 0.81 | 67% |
| 195 | 993 | 0.87 | 42% |
| 225 | 1128 | 0.99 | 8% |

**No prediction recorded.** `probe.curve_a135` *is* the shipped encounter with
this stat; the expectation was equality, not a range.

**Result: equality, to the digit.**

| Matchup | Before | After | Band |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 85..100 ok |
| `matchup.assassin_ambush` | 100% | **67%** | 45..90 **ok** |
| `matchup.dio_boss` | 50% | 50% | 35..75 ok |

`matchup.assassin_ambush`, per battle:

| Combatant | Dealt | Taken | SP | Miss | Alive |
| --- | --- | --- | --- | --- | --- |
| Jotaro | 1031 -> 985 | 238 -> **339** | 70 -> 70 | 4% -> 8% | 100% -> **67%** |
| Kakyoin | 319 -> 304 | 504 -> 514 | 86 -> 82 | 21% -> 20% | 17% -> 17% |
| Flame Assassin | 308 -> 324 | 640 -> 640 | 38 -> 41 | 9% -> 9% | 0% -> 0% |
| Iron Brawler | 428 -> **523** | 709 -> **648** | 57 -> 57 | 12% -> 12% | 0% -> **33%** |

Six changes were spent moving a ratio along the flat part of a curve nobody had
plotted. **Measure the response curve before sizing the first change, not after
the sixth.**

> **Revisited after issue #12, and the lesson is now larger than it was.** Each
> of the five curve points is a twelve-seed reading with roughly 14 points of
> standard error. The two middle points, 67% and 67%, and the neighbouring 42%,
> are not reliably distinguishable from one another. The curve's *shape* -- flat
> at low ratio, collapsing near parity -- survives, because the endpoints are 100%
> and 8% and no amount of sampling error bridges that. Its *resolution* does not.
> The six wasted changes were partly chasing a flat curve and partly chasing
> noise, and at the time there was no way to tell those two apart.

> **The plateau is retracted outright by issue #14.** The 67% at `atk` 135 and the
> 67% at `atk` 165 were not two independent points agreeing. At 300 seeds they
> read **53%** and **34%** -- nineteen points apart, with about three points of
> error each. Two draws of 8/12 landing on the same value is the single most
> likely coincidence available to a twelve-seed instrument.
>
> This reaches a real decision. `atk` 135 was chosen over 165 partly *because*
> they measured the same win rate, so 135 could be taken as the near edge of a
> flat region that would absorb a later party buff rather than push the encounter
> off a cliff. **There is no flat region, and there is no cliff.** The choice of
> 135 survives on its other two grounds -- effective `atk` stays under Dio's, and
> it leaves Kakyoin alive some of the time -- but the plateau argument behind it is
> void, and the shipped encounter sits on the *steepest* part of the curve rather
> than a stable one. It must be re-measured after any change touching its four
> participants.
>
> The prior annotation also over-credited the shape. "Flat at low ratio" is not
> supported either: the new lowest rung, `atk` 75, reads 96% rather than 100%.
> What survives is only that the curve is monotone and spans the full range.

---

## Issue #8 -- `miss%` counted per action instead of per accuracy roll

Commits `2808a26` (`report.rs`), `387d02a` (`sim.rs`), `cec68f7` (CLI legend).
**A reporting fix, not a rules change: no win rate moved and every battle is
bit-identical.**

**What was wrong.** An `all_enemies` skill against two foes produced two accuracy
rolls and one action; `misses / actions` could exceed the real miss rate.
`CombatantStats` gained `attack_rolls`; `miss%` is now `misses / attack_rolls`.

| Combatant | Encounter | Released `0.3.0` | Corrected | Cause |
| --- | --- | --- | --- | --- |
| Kakyoin | `assassin_ambush` | **20%** | **11%** | Two rolls per `blade_volley` |
| Kakyoin | `dio_boss` | 9% | 9% | Same skill, rounding absorbs it |
| Everyone else | all | unchanged | unchanged | Single-target only |

**The released `0.3.0` report is wrong in that one column, and it stays on the
record** rather than being edited to match.

---

## Issue #9 -- the AI scores utility, and SP acquires a price and a refill

Commits: `1602dc5` and `38b775a` (scoring), `6ab2c8c` (unit fix),
`d1c1741` / `cd947af` / `4a5f76f` (per-skill accounting), `bcc46cb` (SP priced),
`5fd20df` / `3825de8` / `1ba5ef2` (content), `1c5cda7` / `89f0edc` / `7f72a4f`
(SP recovery). **A rules change. Boundaries 2, 3 and 4 begin here.**

**What was wrong.** `ai::best_offensive` ranked by summed damage power. A skill
with no damage effect scored zero. Power was read per target, not per encounter.

**The unit.** Every effect priced in damage-equivalent points.

**Prediction scorecard.**

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `thug_solo` stays at 100% | 100%, bit-identical | **correct** |
| 2 | `dio_boss` falls to 8-33% | 8% on first run | correct on the first run |
| 3 | `assassin_ambush` moves to 42-67% | 33% | **wrong** |
| 4 | `rage_focus` stays unused | unused; `guard_stance` too | correct as a floor, **wrong in scope** |

### The six runs, in order

| Commit | What changed | thug | ambush | boss |
| --- | --- | --- | --- | --- |
| `f485b5a` | pre-#9 baseline | 100% | 67% | 50% |
| `6ab2c8c` | scoring + unit fix | 100% | **33%** | **8%** |
| `4a5f76f` | per-skill accounting (reporting only) | 100% | 33% | 8% |
| `bcc46cb` | SP priced at its best other use | 100% | **58%** | **0%** |
| `1ba5ef2` | `emerald_splash`, Hierophant swap, volley 24 -> 40 SP | 100% | 50% | **0%** |
| `7f72a4f` | SP recovers 4/turn, volley **36** SP | 100% | **67%** | **42%** |

> **Read this table with issue #12 in hand.** Every cell is a twelve-battle
> reading. The ambush moving 33 -> 58 -> 50 -> 67 looks like a response curve and
> is largely inside one standard error end to end. The *boss* column is the
> trustworthy one: 50 -> 8 -> 0 -> 42 spans far more than noise, and the SP-regen
> row in particular moved a genuinely large distance. Findings 1 and 2 below rest
> on the boss column and on direct inspection of the code, not on the ambush
> column, so they stand.

### Finding 1 -- the choice function was never the binding constraint

Dio's SP pool of 200 was exhausted by turn twenty of a fifty-one-turn fight.
A choice function cannot allocate a budget that does not exist.

### Finding 2 -- SP never came back, in any battle, ever

No code path anywhere in `state.rs`, `battle.rs` or `resolve.rs` added SP to a
combatant. Every fight in the entire history of this project was fought on the
starting pool. `SP_REGEN_PER_TURN = 4`, applied in `end_turn`, is the fix.
Two tests pin the semantics. Measured effect in `matchup.dio_boss`, `sp/b`:

| Combatant | SP pool | Before (`1ba5ef2`) | After (`7f72a4f`) |
| --- | --- | --- | --- |
| Jotaro | 80 | 70 | **115** |
| Josuke | ~155 | 82 | **127** |
| Kakyoin | 145 | 112 | **121** |
| Dio | **200** | 191 | **252** |

Dio now spends more SP in one battle than he owns. So does Jotaro.

### Finding 3 -- repricing `blade_volley` to 40 SP deleted the skill

At 40 SP the volley loses to a free attack before the halt is even considered.
36 SP is the price at which it is chosen only when the halt is unaffordable and
only because the pool regenerates. A skill reachable for one specific reason is
fragile.

### The final measurement -- commit `7f72a4f`

`matchup.thug_solo` **bit-identical** to the pre-#9 baseline. `matchup.dio_boss`
-- 5 won, 7 lost, turns 51 / 43 / 65:

| Combatant | dealt/b | taken/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 1413 | 996 | 115 | 155 | 8% | 33% |
| Josuke | 379 | 895 | 127 | 88 | 7% | 42% |
| Kakyoin | 209 | 937 | 121 | 117 | 12% | 8% |
| Dio | 2486 | 1360 | 252 | 308 | 4% | 58% |
| Flame Assassin | 305 | 640 | 41 | 45 | 9% | 0% |

`matchup.assassin_ambush` -- 8 won, 4 lost, turns 19 / 14 / 24:

| Combatant | dealt/b | taken/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 976 | 367 | 76 | 83 | 8% | 67% |
| Kakyoin | 295 | 516 | 73 | 76 | 7% | 8% |
| Flame Assassin | 330 | 631 | 42 | 47 | 9% | 17% |
| Iron Brawler | 547 | 639 | 65 | 53 | 13% | 33% |

### What this entry does not authorise

- **The bands are not re-declared here.** Re-declaring with evidence is issue #12.
- **The curve is void for the fifth time.** Re-sweep is issue #14.
- **"Zero unreachable skills" is not met.** Twelve skills, ten reachable, two not:
  `guard_stance` and `rage_focus`. Tracked as issue **#29**.

  > **Correction, filed with #29.** This line read "Issues #15 and #16" from the
  > day it was written until #29 was opened. It was wrong the whole time: #15 is
  > a third playable character and #16 is three more enemies, and neither body
  > mentions either skill. A search across every issue in the repository for
  > `guard_stance` or `rage_focus` returned nothing. Two skills were recorded as
  > tracked, in two places in this file, for two milestones, with no issue behind
  > either citation -- and the claim was carried forward verbatim into `1eadf86`
  > without being checked. A cross-reference is a claim like any other and decays
  > the same way the probe roster did.

---

## Issue #10 -- elemental resistances applied in damage resolution

Commits: `46a06df` (data schema), followed by state.rs (runtime field),
`f8f7592` (resolve pipeline), `a684c7c` (initial tables), `5644ee7` (validator
checks), `482be74` (battle fixture fix), `f6ba445` (sim fixture fix).
**A rules change. Boundary 5 at the top of this file begins here.**

**What was wrong.** The engine carried `element` on `Effect::Damage` and on every
`Event::Damaged` since `0.1.0`. `resolve::compute_damage` passed it through the
pipeline and then did nothing with it. Every hit landed as if the target had zero
resistance to everything.

**The schema.** `data.rs` gains:
- `type Resistances = HashMap<Element, i32>` with `MAX_RESISTANCE = 100` and
  `MIN_RESISTANCE = -100`
- `CombatantDef.resist: Resistances` with `resistance(element) -> i32`
- `#[serde(default)]` on `state::Combatant.resist` so existing save data loads
  without a field

**The formula.** `compute_damage` now calls `target.resistance(element)` after
the base-minus-defence step:

```
damage = max(damage * (100 - resist) / 100, 1)
```

Positive resist cuts damage, floored at 1 -- a 100% resistant element deals 1,
not 0, which prevents an accidental heal-on-hit. Negative resist amplifies.
Three unit tests cover the modifier, the immunity edge case, and element
isolation.

**The initial tables** in `data/combatants.json`:
- `pc.jotaro`: `{ "temporal": 50 }` -- Star Platinum's time-stop affinity
- `npc.dio`: `{ "temporal": 50, "psychic": 20 }` -- The World's domain
- `npc.flame_assassin`: `{ "fire": 75 }` -- Magicians' Red fire immunity
- `npc.iron_brawler`: `{ "physical": 25, "psychic": -25 }` -- armour and
  psychic exposure
- All others carry empty tables (intentional control: `thug_solo` must not move)

**Schema defect, owned here.** Adding `resist` to `CombatantDef` broke every
struct literal that initialises it without sweeping all files. Both failures
(`battle.rs:359`, `sim.rs:310`) were inside `#[cfg(test)]`, so
`cargo run --bin balance` compiled and produced a full report while
`cargo clippy --all-targets` and `cargo test` could not build.

> **A second instance of the same defect surfaced in issue #14.** `#[serde(default)]`
> on `resist` means a roster file written before this change loads without error
> as an empty table. `tools/probe/combatants.probe.json` did exactly that for two
> milestones, silently, and would have produced a full and entirely wrong sweep.
> The field that made migration painless also made drift invisible. Any hand-
> maintained copy of `data/*.json` must be diffed against the original before it
> is trusted, and the probe README now says so.

### The two runs, in order

| Commit | What changed | thug | ambush | boss |
| --- | --- | --- | --- | --- |
| `7f72a4f` | pre-#10 baseline (step 1 final) | 100% | 67% | 42% |
| `5644ee7` | resistance schema + tables + validator (gate broken) | 100% | 50% | 42% |
| `f6ba445` | fixture fixes (gate green) | 100% | 50% | 42% |

### Scorecard for the first run

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `thug_solo` bit-identical | bit-identical in every column | correct |
| 2 | Validator 0 errors / 0 warnings | 0 errors, 0 warnings | correct |
| 3 | 67 tests pass | build broken -- 0 tests ran | **wrong** |
| 4 | `balance_bounds` FAILED, bands break | all three in band | **wrong** |
| 5 | ambush rises from 67%, boss falls from 42% | ambush fell to 50%, boss held | **wrong** |

Two of five. The second run scored five of five, mechanically: the fixture fixes
touch no production code path.

### Finding 1 -- the ambush fell because the AI is resistance-blind

The Iron Brawler carries `physical: 25`, which cuts Jotaro's `rush_barrage` and
`strike` by 25%. It also carries `psychic: -25`, which amplifies Kakyoin's
psychic skills by 25%. The net was a **17-point drop**, 67% -> 50%.

That drop is not symmetric. Jotaro's physical barrage was blunted without any
compensating routing toward the Brawler's psychic vulnerability, because
`score_action` prices `Effect::Damage` from its `power` field without calling
`target.resistance(element)`. Filed as issue #22.

> **Amended by issue #12.** The 17-point drop was a twelve-seed reading and sits
> close to one standard error, so its *magnitude* is not established. The
> mechanism is, because it was found by reading `score_action` rather than by
> reading the win rate. The scorer genuinely does not consult resistance tables.
> What cannot be claimed from that run is that resistance-blindness costs the
> party seventeen points; only that it costs them something.

### Finding 2 -- `thug_solo` is the control, and it held

Street Thug carries no resistance table. No skill in `matchup.thug_solo` deals a
typed element that any combatant there resists. The report is **bit-identical**
to the pre-#10 run in every column, which is the strongest available evidence
that the resistance formula reached only combatants and elements it was supposed
to reach. Note that bit-identity is a far stronger signal than an unchanged win
rate, and it is not affected by sample size: two runs either produce the same
event stream or they do not.

### Finding 3 -- the boss win rate held at 42%, but every row moved

Dio deals `temporal` damage; Jotaro resists temporal at 50%. Jotaro's `dealt/b`
rose 1413 -> **1466** while Dio's fell 2486 -> **2392**.

The Flame Assassin's row changed by exactly one `dealt/b` point (305 -> 304)
despite carrying `fire: 75` and receiving no fire damage in this fight. This
anomaly was first noted in the #9 entry and is still undiagnosed.

---

## Issue #11 -- healing is attributed to the combatant that performed it

Commits: `a7bfadc` (`event.rs`), `c9d9b45` (`resolve.rs`), `ea74551`
(`battle.rs`), `d49ab4c` (`report.rs`), `883eddb` (`sim.rs`), `4b74979`
(`bin/balance.rs`), `e15b05a` (`battle_view.gd`). **A reporting change, not a
rules change. No comparability boundary.**

**What was wrong.** `Event::Healed` carried a target and no source, so no report
could say who did the healing. The party's dedicated healer appeared in every
published table as a row of zeroes in the only column that measured
contribution.

**The decision it forced.** Three sites emit `Healed`, and one of them has no
actor to name: regeneration ticks belong to a status, not to a combatant.
Filling `actor` with the carrier's own index would have produced a log claiming
a character healed itself through an action it never took -- exactly the defect
`Event::StatusDamaged` was added in `0.1.0` to fix, with the sign flipped. So
`Healed` gained an actor and regeneration moved to a new `Event::StatusHealed`.

**Result: every pre-existing figure is bit-identical.** No RNG is drawn by adding
a field to an event, so the pass condition was that nothing moves, and nothing
did -- `dealt/b`, `taken/b`, `sp/b`, `rolls`, `miss%` and `alive%` reproduce digit
for digit across all fifteen combatant rows, and every `actions by skill` line is
identical.

### Scorecard

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | 72 tests (59 lib + 5 bin + 5 bounds + 3 det.) | exactly 72 | correct |
| 2 | Every existing figure unchanged | unchanged, digit for digit | correct |
| 3 | Josuke's `heal/b` non-zero | 1044 | correct |
| 4 | Dio's `heal/b` non-zero via `blood_drain` | 183 | correct |
| 5 | No new inert warning | none | correct |

**An earlier prediction, made before the code and superseded before the run, is
recorded here because only recording the corrected one would be dishonest:** it
stated that `heal/b` would read **0 for every combatant**, on the reasoning that
"the shipped roster has no reachable healing skill and `blood_drain` is not in
any current matchup". Both halves were false. `skill.restore` is 39% of Josuke's
boss turns and `blood_drain` is used in both the ambush and the boss fight. It
was corrected by reading `data/skills.json` rather than by running anything.

### What the column showed on its first run

1. **Josuke restores 1044 HP per battle while dealing 358.** Against roughly 2740
   HP of incoming damage per battle, the healer undoes 38% of everything the
   enemy side does. It looked like the party's weakest member and was second only
   to Jotaro in contribution.
2. **Dio self-heals 183 per battle** through `blood_drain` -- around 12% of its
   effective durability, on a boss whose printed pool is 1520.
3. **The Iron Brawler heals 6 per battle**, from three uses in a whole batch.
   Recorded precisely because it is negligible: a column that only ever printed
   large numbers would be a column nobody checked.
4. **The `0.3.0` estimate was sound and 9% low.** See the annotation on baseline
   finding 5.

### Also fixed

`MatchupReport::inert_combatants` tested `damage_dealt == 0`, which would have
accused a dedicated healer of sitting out a fight it was carrying. It now
requires zero damage **and** zero healing.

---

## Issue #12 -- the seed list widens, and the instrument turns out to have been misread

Commits `08a40c4` (12 -> 24 seeds) and `1f5aaed` (24 -> 300 seeds). **Data only.
No code, no rules, no content. No comparability boundary** -- see the note at the
top of this file on why a wider sample is not a boundary.

This entry is the most consequential in the file, and not for the reason the
issue anticipated. The issue expected to re-declare bands against changed rules.
What actually happened is that the project discovered it had been misreading its
own instrument since the harness was built.

### The error

Every document here -- this file, `ROADMAP.md`, half a dozen commit messages --
has described twelve seeds as "resolving to 8.3 percentage points". That is
**granularity**: 1/12, the distance between two adjacent possible readings. It is
not the measurement error, and the two were used interchangeably.

The sampling error of a proportion is `sqrt(p(1-p)/n)`:

| Seeds | Standard error at p~0.5 | 95% interval width |
| --- | --- | --- |
| 12 | **+/-14.4 points** | +/-28 |
| 24 | +/-10.2 | +/-20 |
| 300 | **+/-2.9** | +/-5.7 |

So the true uncertainty at twelve seeds was roughly **three times** the figure
this file kept quoting. Every entry above that argues about a ten- or
fifteen-point movement is arguing inside the noise floor.

### The two runs

| Seeds | thug | ambush | boss | Gate |
| --- | --- | --- | --- | --- |
| 12 (`f6ba445`) | 100% | 50% | 42% | green |
| **24** (`08a40c4`) | 100% | **42%** | **29%** | **RED, two out of band** |
| **300** (`1f5aaed`) | 100% | **53%** | **47%** | green |

### The mistake I made in between, recorded because it is the whole lesson

The 24-seed run put two encounters out of band. Because the twelve original seeds
were kept as a subset, the twelve added ones could be isolated: they gave the
ambush 4 wins from 12 and the boss 2 from 12. I concluded from this that **the
original twelve seeds had been a lucky sample** and that every band in the
project had been fitted against an optimistic instrument.

That conclusion was wrong, and it was wrong by exactly the error it was
diagnosing. At 300 seeds the ambush reads 53% and the boss 47% -- both *above* the
twelve-seed readings of 50% and 42%. The twelve added seeds were an unlucky
sample, not the original twelve a lucky one. I over-read a difference that was
comfortably inside one standard error, in the middle of writing an argument about
over-reading differences inside one standard error.

**The practical consequence, had the harness been trusted at 24 seeds:** the next
commit would have been a content commit buffing the party to rescue a boss fight
reading 29%. The boss fight was never at 29%. That buff would have shipped, and
the 300-seed run would then have shown a party that was far too strong.

### Scorecard -- run 1, 24 seeds

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `thug_solo` 100%, unchanged | 100% | correct |
| 2 | ambush 45-58% | 42% | **wrong** |
| 3 | boss 38-50% | 29% | **wrong** |
| 4 | Gate green but uncomfortable | red, two encounters | **wrong** |
| 5 | Ambush likelier of the two to fail | both failed; boss failed by more | **wrong** |

### Scorecard -- run 2, 300 seeds

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `thug_solo` 100% | 100% | correct |
| 2 | ambush 38-46%, **out of band** | 53%, in band | **wrong** |
| 3 | boss 25-33%, **out of band** | 47%, in band | **wrong** |
| 4 | Gate stays red | green | **wrong** |

One of four, and the one correct prediction was that a guaranteed win would
remain a guaranteed win. Both scorecards are bad in the same direction: I twice
projected the most recent reading forward as though it were the true value, which
is the specific error a confidence interval exists to prevent.

### The bands are correct, and nothing else changes

Issue #12 was written expecting to re-declare bands and warning that widening a
band because CI is red "converts the gate into decoration". The measured answer
is that **no band needs to move**. All three encounters sit inside the bands
declared before any of the M3 rules changes, and now the intervals sit inside
them too. The bands were a statement of design intent, and the content meets the
intent.

This is a much better outcome than a band adjustment would have been, and it was
only reachable by refusing to touch content while the instrument was still too
coarse to justify touching it.

### Why 300, and why a contiguous range

300 seeds puts the 95% interval at about +/-5.7 points, finer than any distance
this project has argued over. The cost is nothing: `balance_bounds` runs 900
battles in **0.36 seconds**. Three releases of content decisions were taken on
+/-14 points of noise when removing that noise cost a third of a second and one
edit to a JSON array.

Seeds are now the contiguous range `1..=300`. SplitMix64 places the seed directly
into state and passes it through a Murmur3-style finalizer, so sequential seeds
produce decorrelated streams; a scattered list buys no independence and only makes
the set harder to audit. All twelve original Fibonacci seeds are <= 233, so this
run remains a superset of every figure measured before it.

### `matchup.thug_solo` -- 300 battles, 300 won, turns 3 / 1 / 7

| Combatant | dealt/b | taken/b | heal/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 300 | 15 | 0 | 24 | 656 | 10% | 100% |
| Street Thug | 15 | 300 | 0 | 0 | 260 | 5% | 0% |

Actions: Jotaro `rush_barrage 412 (63%), strike 244 (37%)`; Street Thug
`strike 260 (100%)`.

The control row is instructive about small samples in a different way. At twelve
seeds the Street Thug's `miss%` read **0%** over ten rolls; over 260 rolls it
reads **5%**. Nothing changed but the number of observations. The same is true of
Jotaro's 4% -> 10% and of the longest battle, 4 turns -> 7.

### `matchup.assassin_ambush` -- 300 battles, 158 won, 142 lost, turns 21 / 13 / 32

| Combatant | dealt/b | taken/b | heal/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 900 | 487 | 0 | 81 | 2232 | 8% | 51% |
| Kakyoin | 344 | 497 | 0 | 78 | 2018 | 10% | 15% |
| Flame Assassin | 344 | 611 | 0 | 43 | 1290 | 8% | 16% |
| Iron Brawler | 631 | 632 | 6 | 67 | 1514 | 11% | 42% |

Actions: Jotaro `rush_barrage 1356 (61%), strike 876 (39%)`; Kakyoin
`emerald_splash 843 (72%), emerald_snare 326 (28%), strike 6 (1%)`; Assassin
`sun_flare 823 (64%), strike 467 (36%)`; Brawler `concussive_slam 899 (59%),
strike 574 (38%), blood_drain 41 (3%)`.

The Iron Brawler's `miss%` was recorded in the #10 entry as **16%**, "the highest
recorded for any combatant", from 61 rolls. Over 1514 rolls it is **11%**, in
line with everyone else. That superlative was noise and is retracted here.

Kakyoin uses `strike` six times in 300 battles -- 1% of its turns, invisible at
twelve seeds. Rare branches only appear when the sample is large enough to
contain them.

### `matchup.dio_boss` -- 300 battles, 141 won, 159 lost, turns 54 / 28 / 84

| Combatant | dealt/b | taken/b | heal/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 1489 | 773 | 0 | 114 | 4028 | 7% | 44% |
| Josuke | 373 | 930 | **1088** | 123 | 2014 | 7% | 34% |
| Kakyoin | 231 | 985 | 0 | 131 | 3321 | 9% | 15% |
| Dio | 2304 | 1453 | **162** | 249 | 7765 | 4% | 53% |
| Flame Assassin | 369 | 639 | 0 | 46 | 1393 | 9% | 1% |

Actions: Jotaro `strike 2119 (53%), rush_barrage 1909 (47%)`; Josuke
`strike 1492 (45%), restore 1273 (39%), concussive_slam 522 (16%)`; Kakyoin
`emerald_snare 1394 (56%), emerald_splash 847 (34%), strike 233 (9%)`; Dio
`strike 2698 (53%), tempo_halt 1188 (23%), blood_drain 745 (15%),
concussive_slam 341 (7%), blade_volley 139 (3%)`; Assassin `sun_flare 874 (63%),
strike 519 (37%)`.

Three things worth noting:

1. **Josuke's survival was badly understated.** 17% at twelve seeds, **34%** at
   three hundred. The healer is twice as durable as the report claimed, which
   matters because "the healer dies early" was an assumption behind more than one
   tuning argument.
2. **Josuke's healing is confirmed at 1088/battle**, close to the 1044 measured at
   twelve seeds. Per-battle averages converge much faster than win rates, because
   each battle contributes hundreds of observations rather than one.
3. **The Flame Assassin survives 1% of boss battles.** At twelve and twenty-four
   seeds it read 0%, which invited the conclusion that it never survives. It
   does, about three times in three hundred.

### What this entry does not authorise

- **No content change.** The bands hold and the intervals hold. Any content edit
  now needs its own justification, not this run's.
- **The curve is still void.** [`BALANCE-CURVE.md`](BALANCE-CURVE.md) was swept
  at twelve seeds under pre-`0.4.0` rules; it is now invalid on both counts.
  Re-sweep is issue #14, and it must be swept at 300. *(Done -- see below.)*
- **The historical entries are not rewritten.** Their annotations mark what is
  now known to be inside the noise. A retracted measurement is evidence too.

---

## Issue #14 -- the response curve is re-swept, and the plateau it published never existed

Commits `60667aa` (probe roster repaired and widened to 300 seeds) and `9c8a88f`
(`BALANCE-CURVE.md` rewritten). **Instrumentation only. No code, no rules, no
content, no comparability boundary** -- nothing under `tools/probe/` is reachable
from the game, the validator or the CI gate.

Six probes, 1800 battles, `atk` 75 to 225. Full write-up and the pricing tool are
in [`BALANCE-CURVE.md`](BALANCE-CURVE.md); this entry records what it changes
about the rest of this file.

### The instrument had drifted, silently, and would have produced a wrong sweep

Before anything could be measured, `tools/probe/combatants.probe.json` had to be
repaired. Three faults, of which the first is the serious one:

1. **Not one combatant in it carried a `resist` table.** The file predates issue
   #10, and `resist` is `#[serde(default)]`, so it loaded without complaint as
   empty. The Iron Brawler clones were taking full physical damage from Jotaro
   where the shipped one takes 25% less, and ordinary psychic damage from Kakyoin
   where the shipped one takes 25% more.
2. **The control had moved and nobody had moved it.** Change G shipped `atk`
   105 -> 135 two milestones ago, so `probe.curve_a105` had quietly stopped being
   the shipped creature. The control is now `probe.curve_a135`.
3. **The seed lists were still the twelve Fibonacci seeds.**

Issue #14 asked for the gap between control and shipped encounter and called that
gap the measurement of what the rules changes did. Run uncorrected, the gap would
have mixed six rules changes with one missing resistance table, and there would
have been no way to separate them afterwards. This is the same failure the
"change one number" rule at the top of this file exists to prevent, arriving
through a file nobody thought of as content.

### The control

`probe.curve_a135` reproduced the shipped `matchup.assassin_ambush` in **every
digit** -- 158 won, 142 lost, turns 21/13/32, all four combatant rows, and every
skill-use count down to Kakyoin's six uses of `strike`. Two different roster
files describing the same combatants produce the same 300-battle event stream.

### The curve

| Probe | `atk` | Win rate | 95% interval | Jotaro alive | Turns |
| --- | --- | --- | --- | --- | --- |
| `a075` | 75 | **96%** | 94..98 | 93% | 22 |
| `a105` | 105 | **81%** | 77..85 | 78% | 21 |
| `a135` | 135 | **53%** | 47..59 | 51% | 21 |
| `a165` | 165 | **34%** | 29..39 | 33% | 19 |
| `a195` | 195 | **20%** | 16..25 | 19% | 17 |
| `a225` | 225 | **12%** | 8..16 | 12% | 16 |

Monotone throughout. No plateau, no cliff, and no flat region at either end of
the measured range.

### Scorecard

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `a075` 95-100% | 96% | correct |
| 2 | `a105` 72-85% | 81% | correct |
| 3 | `a135` exactly 53%, every digit | exactly that | correct (arithmetic, not a forecast) |
| 4 | `a165` 30-42% | 34% | correct |
| 5 | `a195` 15-27% | 20% | correct |
| 6 | `a225` 3-12% | 12% | correct, on the boundary |
| 7 | The 67% plateau does not reappear | it did not | correct |
| 8 | The bend moves to a lower `atk` | steepest segment is still 105-135 | **wrong** |
| 9 | The control matches bit for bit | it did | correct |

Eight of nine, the best run this project has recorded, and three things keep that
honest: row 3 is arithmetic, the ranges were 10-13 points wide against an
instrument with 3 points of error, and row 6 landed on its boundary. The one miss
is informative: the curve moved **down** without moving **sideways**. Six rules
changes lowered the whole response without relocating the point of maximum
sensitivity.

### What this retracts

- **The 67% plateau.** See the annotation on Change G. Two draws of 8/12 landing
  on the same value.
- **"A race resolves as a step function around parity."** See the second
  retraction on Change D. It resolves as an ordinary sigmoid.
- **The SP ceiling as the explanation for the throughput model's error.** See the
  amendment in the model section. The binding constraint is battle length.

### What it could not deliver, and why that is the finding

Issue #14's second acceptance criterion says the gap between the old control row
and the new one **is** the measurement of what the rules changes did. It is not.
Put 95% intervals on the five old twelve-seed points and compare:

| `atk` | Old | Old 95% interval | New | Contradicted? |
| --- | --- | --- | --- | --- |
| 105 | 100% | 76..100 | 81% | no |
| 135 | 67% | 39..94 | 53% | no |
| 165 | 67% | 39..94 | 34% | **yes** |
| 195 | 42% | 15..72 | 20% | no |
| 225 | 8% | 0..36 | 12% | no |

**Four of five old points cannot disagree with anything.** Their intervals are
wide enough to contain the new values, so the differences are equally consistent
with "the rules changed the curve" and with "the old instrument could not see".
Differencing the tables would yield five numbers of which four are noise, with no
way to identify which four. The old table is therefore **retracted, not
differenced**, and what six rules changes did to this curve is not recoverable
from the record.

That is the same conclusion issue #12 reached from the opposite direction. A
measurement taken with an instrument too coarse is not a cheap version of the
real thing; it is an absence of information shaped like a number, and it cannot
be rescued later by measuring properly, because there is nothing to compare
against.

### What it establishes

1. **Jotaro is the win condition, and this is now the best-supported claim in the
   project.** His survival tracks the win rate to within three points at all six
   points on the curve. Kakyoin's sits far below it throughout. See the
   annotation on Change F.
2. **Per-turn output is linear in `atk`; only per-battle output saturates.**
   `0.22 x atk` holds to within 4% across a threefold range. The saturation
   visible in per-battle figures is battle-length compression, 22 turns down to
   16.
3. **The shipped ambush sits on the steepest part of the curve**, about 0.8 win
   rate points per point of enemy `atk`. It is tunable and it is not stable.
4. **The one-sided ratio axis is unusable above 0.9.** `Foe out / party HP` moves
   0.92 -> 0.97 while the win rate falls 34% -> 12%, because per-battle output is
   depressed by the very short battles a strong enemy causes. The replacement is
   the exchange ratio, `(foe out / party HP) / (party out / foe HP)`, which
   crosses 1.0 exactly where the win rate crosses 50%.

### An unexplained trend, filed rather than guessed at

Kakyoin abandons `emerald_snare` as the enemy gets stronger: 35%, 32%, 28%, 21%,
19%, 14% of his turns across the six rows. Six points, monotone, 1800 battles --
not noise. It is not merely shorter battles either: his turns per battle fall by
a factor of 1.8 while his snare uses fall by 4.4.

The probe's `spd` is 62 on every row and only `atk` differs, so the thing
`spd_down` counters has not changed at all. The mechanism is somewhere in
`ai.rs`. **Filed as issue #27 rather than explained here**, with the condition
that the explanation and any fix must not share a commit -- the curve above was
measured with the current behaviour, and if that behaviour is a defect then part
of this curve is measuring the defect.

### What this entry does not authorise

- **No content change.** Nothing in `data/` moved and nothing needs to.
- **The curve is valid only for the current rules.** It is now correct on both
  counts that invalidated it -- swept at 300 seeds, under `0.4.0` rules -- but
  issues #15, #16, #17, #22, #27 and #29 all touch rules or the roster, and any
  of them voids it again.
- **The probe roster is not self-maintaining.** Diff it against
  `data/combatants.json` before every sweep.

---

## Known limitations of the harness itself

Recorded here so a number is not over-read:

- **~~Healing is not attributed.~~ Fixed in issue #11.** `Event::Healed` now
  carries an actor and `CombatantStats` reports `healing_done`. Regeneration is
  reported as `Event::StatusHealed` and is deliberately credited to nobody.
- **~~Twelve seeds is a small sample.~~ Fixed in issue #12**, and it was worse
  than this list claimed. The figure quoted here for a year was 8.3 points, which
  is granularity rather than sampling error; the true standard error at twelve
  seeds was around 14. Now 300 seeds and about +/-2.9.
- **~~`miss%` is inflated for area skills.~~ Fixed in issue #8.**
- **~~The curve is void.~~ Re-swept at 300 seeds in issue #14**, under current
  rules, with the control reproducing shipped content exactly.
- **The probe roster is a hand-maintained copy and drifts silently.** It carried
  no resistance tables for two milestones without any error, because `resist` is
  `#[serde(default)]`. A schema field that defaults gracefully makes migration
  painless and makes drift invisible. Diff `tools/probe/combatants.probe.json`
  against `data/combatants.json` before every sweep, and treat a control row that
  does not reproduce shipped content in every digit as proof the sweep is void.
- **Cross-references in this file decay the same way.** Two skills were recorded
  as "tracked by issues #15 and #16" for two milestones while no such issue
  existed; see the correction in the #9 entry. A citation is a claim and should
  be checked when it is carried forward, not assumed to have been checked by
  whoever wrote it first.
- **Confidence is not reported, only the point estimate.** `balance_bounds`
  compares a single measured percentage against a band and says nothing about how
  precisely that percentage is known. At 300 seeds the interval is narrow enough
  that this rarely matters, but the gate would report a 47% reading and a 34%
  reading with identical confidence. Reporting the interval alongside the
  estimate would make the instrument honest about itself.
- **SP recovery is silent.** `recover_sp` deliberately emits no event, so the
  Godot HUD cannot show a player where their SP came from. `sp/b` above a
  combatant's `max_sp` is the only visible signal.
- **`sp/b` is derived by the CLI, not by the report.** `bin/balance.rs` computes
  it with truncating integer division while every neighbouring column rounds half
  up through `divide_rounded`. Cosmetic in size; tracked as issue #25.
- **A rules change resets the series.** RNG draw order is part of the rules, so
  after any edit to `ai.rs`, `resolve.rs`, `state.rs` or `battle.rs` the same seed
  no longer reproduces the same battle. Issue #9 crossed this boundary six times;
  issue #10 crossed it once; issues #11, #12 and #14 crossed it zero times.
- **One row survived a boundary it should not have.** `Flame Assassin` in
  `dio_boss` was effectively constant across two boundaries (305 at `7f72a4f`,
  304 at `f6ba445`). Until that is explained, treat "bit-identical" as evidence
  only when a *whole matchup* reproduces, never a single row.
- **The harness plays worse than a player.** Auto-battle scores each affordable
  skill once and never sets up a combination across turns. An AI-vs-AI win rate
  is therefore a floor for a competent player, not an estimate of their
  experience.
- **The AI is resistance-blind.** `score_action` prices `Effect::Damage` from
  `power` without consulting the target's resistance table. Tracked as issue #22.
- **The AI's skill choice responds to enemy `atk` through an unidentified
  channel.** Kakyoin's `emerald_snare` share falls monotonically as the enemy
  gets stronger, with nothing the skill counters having changed. Tracked as issue
  #27. Until it is explained, treat any measured action mix as descriptive rather
  than as evidence of intended behaviour.
- **~~Only five of eleven skills are ever used.~~ Ten of twelve as of `7f72a4f`.**
  Two remain: `guard_stance` and `rage_focus`, both declined correctly on their
  own numbers -- though "correctly" has itself never been verified against
  `score_action`, and if the scorer cannot price deferred value then every
  duration-based buff in the game is underpriced. Tracked as issue **#29**
  (previously miscited here as #15 and #16).
- **`skill.tempo_halt` is still mispriced, in the other direction.** The scorer
  values a lock as the actions it *denies*; a lock defers, not removes. Dio uses
  it on 23% of its turns. If #29 finds a missing payoff horizon, this and #29 are
  the same defect seen from opposite sides.
- **The curve in [`BALANCE-CURVE.md`](BALANCE-CURVE.md) is a property of the
  current rules, not a constant.** Replace the table rather than appending to it,
  and re-sweep after any change to `resolve.rs`, `ai.rs`, the party roster or the
  skill list.
