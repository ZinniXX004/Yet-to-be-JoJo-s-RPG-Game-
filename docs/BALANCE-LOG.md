# Balance log

Every balance change to `data/*.json`, the measurement that motivated it, and the
measurement that followed it. One entry per change.

The rule this file exists to enforce: **change one number, re-run, record.** Two
numbers at once and the result is uninterpretable, because either change could
have produced it. This is slower and it is the only version that produces
knowledge rather than opinion.

Predictions are written before the run, not after. A prediction recorded after
the fact is a rationalisation, and the running record through `0.4.0` step 2 is
**thirty-one predictions, twenty-one of them wrong**. The scorecards are kept per
entry so the rate is visible rather than asserted. That is the argument for the
harness, not against it -- the same wrong guesses shipped as content, unmeasured,
would have been indistinguishable from design.

All numbers come from:

```powershell
cd src
cargo run -q -p rpg-core --bin balance
```

over the fixed seed list in [`data/matchups.json`](../data/matchups.json).
Seeds are fixed precisely so two rows of this table can be compared.

> **Comparability boundary 1.** Change C alters `ai::choose`, which changes both
> the decisions taken and the order in which the RNG is drawn. Every number
> recorded above that entry is historical. Do not compare it to anything below.

> **Comparability boundary 2.** The `0.4.0` scored-choice change (issue #9, the
> last entry in this file) alters which skill `ai::choose` selects. Every number
> recorded *above* that entry describes rules in which three skills could never
> be chosen. Do not compare across it either.

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
> pool. Re-declaring the three bands against the new economy is issue #12 and it
> is mandatory, not optional.

> **Comparability boundary 5 -- elemental resistances now modify damage.** Issue
> #10 adds resistance lookups to `resolve::compute_damage`. Any figure measured
> before `feat/element-resistance` was built on a flat damage model where every
> element hit for the same amount regardless of target. Do not compare across
> this boundary. The `thug_solo` control remained bit-identical because no
> combatant in that fight carries a resistance table and no skill in it deals a
> typed element, which is why the Street Thug is the explicit control row.

## Current status

After issue #10, commit `f6ba445`, re-measured in full:

| Matchup | Win rate | Band | Status |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 | ok -- bit-identical control |
| `matchup.assassin_ambush` | 50% | 45..90 | ok |
| `matchup.dio_boss` | 42% | 35..75 | ok, but only 7 points above the floor |

All three encounters are inside their declared bands. The third row carries the
same caveat as always: one seed is worth **8.3 points**, so a 7-point margin is
less than one battle. `matchup.dio_boss` is *not* comfortably in band; it is
inside it by less than the instrument can resolve.

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
> resistance percentage. For the ambush, the Iron Brawler's `physical: 25` cuts
> every physical hit by 25%; the model predicts as if resistance were zero.
> Calibrate against a post-#10 run before using the model to size a change that
> targets a resistant or vulnerable enemy.

The two things it makes obvious, both of which the Change E prediction missed:

- **A slow foe is a cheap foe.** The Brawler's 72 speed buys it 20% of the
  actions in the fight, so its 73 damage per action becomes 285 per battle.
  Speed multiplies damage as directly as attack does.
- **A foe's output is capped by its SP, not by the clock.** The Assassin has 70
  SP and `sun_flare` costs 16, so it has at most four expensive turns in it. Its
  per-battle damage has been 353, 332 and 308 across fights of 13, 19 and 18
  turns -- effectively a constant, independent of how long it lives. *(True until
  `7f72a4f`; see the amendment above.)*

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

---

## Change A -- Flame Assassin hp 480 -> 640

Commit `4f21d1f`.

**Motivation:** finding 3. The Assassin is priced as a threat and behaves as a
speed bump, because the party's focus-fire rule targets the lowest HP pool and
480 is below Kakyoin's 520. Raising it above the party's squishiest member lets
the Assassin act for the number of turns its damage output was designed around.

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

- **The bands are not re-declared here.** `matchup.dio_boss` at 42% sits 7 points
  above a 35 floor on a 8.3-point instrument. Re-declaring with evidence is
  issue #12.
- **The curve is void for the fifth time.** Re-sweep is issue #14.
- **"Zero unreachable skills" is not met.** Twelve skills, ten reachable, two not:
  `guard_stance` and `rage_focus`. Issues #15 and #16.

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
isolation (fire resistance does not change a physical hit).

**The initial tables** in `data/combatants.json`:
- `pc.jotaro`: `{ "temporal": 50 }` -- Star Platinum's time-stop affinity
- `npc.dio`: `{ "temporal": 50, "psychic": 20 }` -- The World's domain
- `npc.flame_assassin`: `{ "fire": 75 }` -- Magicians' Red fire immunity
- `npc.iron_brawler`: `{ "physical": 25, "psychic": -25 }` -- armour and
  psychic exposure
- All others carry empty tables (intentional control: `thug_solo` must not move)

**The validator** now checks: every declared resistance element must be dealt by
at least one skill (warns if not), and every value must fall within -100..100
(errors if not).

**Schema defect, owned here.** Adding `resist` to `CombatantDef` broke every
struct literal that initialises it without sweeping all files. Both failures
(`battle.rs:359`, `sim.rs:310`) were inside `#[cfg(test)]`, so
`cargo run --bin balance` compiled and produced a full report while
`cargo clippy --all-targets` and `cargo test` could not build. That is why
there are two gate runs: the first confirmed the design; the second confirmed the
fix.

### The two runs, in order

| Commit | What changed | thug | ambush | boss |
| --- | --- | --- | --- | --- |
| `7f72a4f` | pre-#10 baseline (step 1 final) | 100% | 67% | 42% |
| `5644ee7` | resistance schema + tables + validator (gate broken) | 100% | 50% | 42% |
| `f6ba445` | fixture fixes (gate green) | 100% | 50% | 42% |

The second run is **digit-for-digit identical** to the first. That is expected:
the fixture fixes add `Resistances::new()` to two test-only struct literals and
change nothing that any production code path reaches.

### Scorecard for the first run (five sub-predictions)

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `thug_solo` bit-identical | bit-identical in every column | correct |
| 2 | Validator 0 errors / 0 warnings | 0 errors, 0 warnings | correct |
| 3 | 67 tests pass | build broken -- 0 tests ran | **wrong** |
| 4 | `balance_bounds` FAILED, bands break | all three in band | **wrong** |
| 5 | ambush rises from 67%, boss falls from 42% | ambush fell to 50%, boss held | **wrong** |

Two of five. Predictions 3 and 4 share the same cause: the fixture defect.
Prediction 5 was wrong about direction for a reason (finding 1 below).

### Scorecard for the second run (five sub-predictions)

All five correct. The reason is mechanical: the fixture fixes do not touch any
production code path, so the only thing the second gate was testing was whether
the compile error was the sole failure. It was.

### Finding 1 -- the ambush fell because the AI is resistance-blind

The Iron Brawler carries `physical: 25`, which cuts Jotaro's `rush_barrage` and
`strike` by 25%. It also carries `psychic: -25`, which amplifies Kakyoin's
psychic skills by 25%. The net was a **17-point drop**, 67% -> 50%.

That drop is not symmetric. Jotaro's physical barrage was blunted without any
compensating routing toward the Brawler's psychic vulnerability, because
`score_action` prices `Effect::Damage` from its `power` field without calling
`target.resistance(element)`. The scorer sees the same expected value for a
physical hit and a psychic hit regardless of who the target is.

Filing this as a follow-up issue in milestone `0.4.0`; it is a scorer defect,
not a content defect, and it does not belong in the same commit as the resistance
data.

### Finding 2 -- `thug_solo` is the control, and it held

Street Thug carries no resistance table. No skill in `matchup.thug_solo` deals a
typed element that any combatant there resists. The report is **bit-identical**
to the pre-#10 run in every column -- `300/15/27/23/4%/100%` -- which is the
strongest available evidence that the resistance formula reached only combatants
and elements it was supposed to reach.

### Finding 3 -- the boss win rate held at 42%, but every row moved

Dio deals `temporal` damage; Jotaro resists temporal at 50%. Jotaro's `dealt/b`
rose 1413 -> **1466** while Dio's fell 2486 -> **2392**. The net of those
movements is close to zero in win-rate terms. The instrument's 8.3-point
resolution explains the rest: a shift smaller than one seed cannot be observed.

The Flame Assassin's row changed by exactly one `dealt/b` point (305 -> 304)
despite carrying `fire: 75` and receiving no fire damage in this fight. This
anomaly was first noted in the #9 entry; it is still undiagnosed, and the
pre-resistance baseline is no longer reproducible.

### `matchup.assassin_ambush` -- full post-resistance table

12 battles: 6 won, 6 lost, turns 19 / 17 / 24:

| Combatant | dealt/b | taken/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 924 | 429 | 79 | 88 | 8% | 50% |
| Kakyoin | 328 | 516 | 71 | 75 | 7% | 8% |
| Flame Assassin | 343 | 631 | 42 | 47 | 6% | 17% |
| Iron Brawler | 596 | 620 | 72 | 61 | 16% | 50% |

Actions by skill: Jotaro `rush_barrage 53 (60%), strike 35 (40%)`; Kakyoin
`emerald_splash 33 (79%), emerald_snare 9 (21%)`; Assassin `sun_flare 32 (68%),
strike 15 (32%)`; Brawler `concussive_slam 38 (62%), strike 20 (33%),
blood_drain 3 (5%)`.

Iron Brawler `miss%` 16% is the highest recorded for any combatant.
`concussive_slam` at accuracy 85 accounting for 62% of turns is the mechanical
cause.

### `matchup.dio_boss` -- full post-resistance table

12 battles: 5 won, 7 lost, turns 54 / 45 / 74:

| Combatant | dealt/b | taken/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 1466 | 714 | 118 | 170 | 9% | 42% |
| Josuke | 358 | 962 | 122 | 80 | 9% | 17% |
| Kakyoin | 201 | 1064 | 140 | 137 | 12% | 8% |
| Dio | 2392 | 1385 | 250 | 315 | 3% | 58% |
| Flame Assassin | 304 | 640 | 41 | 45 | 9% | 0% |

Actions by skill: Jotaro `strike 91 (54%), rush_barrage 79 (46%)`; Josuke
`strike 58 (45%), restore 49 (38%), concussive_slam 22 (17%)`; Kakyoin
`emerald_snare 71 (65%), emerald_splash 28 (26%), strike 10 (9%)`; Dio
`strike 112 (53%), tempo_halt 46 (22%), blood_drain 32 (15%),
concussive_slam 15 (7%), blade_volley 6 (3%)`; Assassin `sun_flare 31 (69%),
strike 14 (31%)`.

### What this entry does not authorise

- **The bands are not re-declared here.** All three pre-date resistances.
  `matchup.dio_boss` at 42% sits 7 points above a 35 floor on a 8.3-point
  instrument. Re-declaring with evidence is issue #12; doing it in the same
  commit as the change that moved the numbers is how a harness becomes
  decoration.
- **The curve is void for the sixth time.** [`BALANCE-CURVE.md`](BALANCE-CURVE.md)
  was measured before the resistance modifier. Re-sweep is issue #14; the file is
  not to be cited until then.
- **The AI's resistance-blindness is not fixed here.** It is a scorer defect,
  not a content defect, and belongs in a separate issue and commit.

---

## Known limitations of the harness itself

Recorded here so a number is not over-read:

- **Healing is not attributed.** `Event::Healed` carries a target and no source,
  so the report cannot show healing done. Josuke's contribution has to be
  inferred from his SP spend, which is now also inflated by regeneration. Fixing
  this means adding an actor to the event, which is a code change, not a tuning
  change. Tracked as issue #11.
- **SP recovery is silent.** `recover_sp` deliberately emits no event, so the
  Godot HUD cannot show a player where their SP came from, and the report cannot
  separate "spent from the pool" from "spent from regeneration". `sp/b` above a
  combatant's `max_sp` is the only visible signal.
- **~~`miss%` is inflated for area skills.~~ Fixed in issue #8.** `miss%` is now
  `misses / attack_rolls` and the `rolls` column makes area usage visible. The
  released `0.3.0` figure of 20% for Kakyoin in the ambush is wrong; the
  measurement is 11%.
- **Twelve seeds is a small sample.** Resolution is 8.3 percentage points. One
  seed flipping moves a reported win rate by that much, so 67% and 75% are not
  distinguishable results. `matchup.dio_boss` now passes by 7 points, less than
  one seed. Tracked as issue #13.
- **A rules change resets the series.** RNG draw order is part of the rules, so
  after any edit to `ai.rs`, `resolve.rs`, `state.rs` or `battle.rs` the same
  seed no longer reproduces the same battle. Issue #9 crossed this boundary six
  times; issue #10 crossed it once.
- **One row survived a boundary it should not have.** `Flame Assassin` in
  `dio_boss` has been effectively constant across two boundaries now (305 at
  `7f72a4f`, 304 at `f6ba445`). Until that is explained, treat "bit-identical"
  as evidence only when a *whole matchup* reproduces, never a single row.
- **The harness plays worse than a player.** Auto-battle scores each affordable
  skill once and never sets up a combination across turns. An AI-vs-AI win rate
  is therefore a floor for a competent player, not an estimate of their
  experience.
- **The AI is resistance-blind.** `score_action` prices `Effect::Damage` from
  `power` without consulting the target's resistance table. The ambush fell
  67 -> 50 as a measured consequence. Tracked as a follow-up issue in milestone
  `0.4.0`.
- **~~Only five of eleven skills are ever used.~~ Ten of twelve as of `7f72a4f`.**
  Two remain: `guard_stance` and `rage_focus`, both declined correctly on their
  own numbers. Content defects, issues #15 and #16.
- **`skill.tempo_halt` is still mispriced, in the other direction.** The scorer
  values a lock as the actions it *denies*; a lock defers, not removes. Dio used
  it 44-46 times per batch.
- **The curve in [`BALANCE-CURVE.md`](BALANCE-CURVE.md) is a property of the
  current rules, not a constant.** It is now invalidated six times over. Re-sweep
  is issue #14. Replace the table rather than appending to it.
