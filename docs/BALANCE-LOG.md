# Balance log

Every balance change to `data/*.json`, the measurement that motivated it, and the
measurement that followed it. One entry per change.

The rule this file exists to enforce: **change one number, re-run, record.** Two
numbers at once and the result is uninterpretable, because either change could
have produced it. This is slower and it is the only version that produces
knowledge rather than opinion.

Predictions are written before the run, not after. A prediction recorded after
the fact is a rationalisation, and the running record through `0.4.0` step 7 is
**one hundred and seventeen predictions, fifty-seven of them wrong**. The
scorecards are kept per entry so the rate is visible rather than asserted. That
is the argument for the harness, not against it -- the same wrong guesses shipped
as content, unmeasured, would have been indistinguishable from design.

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

> **Comparability boundary 6 -- `matchup.dio_boss` gains a fourth party member.**
> Issue #15 adds `pc.polnareff` to the boss encounter's party. Nothing in the
> rules changed and no other encounter moved, but the boss fight now has four
> party members where every figure recorded above it has three. 47% and 68% are
> not two readings of the same encounter, and the difference between them is not
> a measurement of anything. **This boundary applies to `matchup.dio_boss` only**
> -- `thug_solo` and `assassin_ambush` reproduce bit for bit across it, which is
> what makes the scope of the boundary a measured claim rather than an assertion.
>
> It is also the first boundary in this file that comes from **content** rather
> than from rules, and that is worth naming. The five above it were all edits to
> `src/`, which made them easy to spot: change `resolve.rs` and obviously the
> numbers move. A roster edit looks like ordinary tuning and is not. Adding a
> combatant to a party changes the denominator of every per-battle average in
> that encounter, changes how long the fight runs, and -- as this entry
> discovered -- can change what the *enemy* chooses to do.

> **Issues #11, #12, #14 and #16 created no boundary, and it matters that they
> did not.** #11 changed reporting only; #12 changed the sample size only; #14
> changed instrumentation that no build reads; **#16 added three enemies and
> three encounters without touching a single existing one**. None touched a rule
> or an existing roster, so every figure spanning them remains an estimate of the
> same underlying quantity. What changed at #12 is how *precisely* those
> quantities are known, which is not the same thing as the quantities having
> moved. A boundary marks "these numbers describe a different game"; a wider
> sample marks "these numbers describe the same game, better"; and a new
> encounter beside the old ones marks "there is more game, and the old part of it
> is untouched". #16 is the strongest case of that last kind in the file, because
> it was verified at **ten consecutive commits** rather than once.

## Current status

After issue #16, commit `f341618d`, measured over **300 seeds** per encounter:

| Matchup | Win rate | 95% interval | Band | Status |
| --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | -- | 85..100 | ok |
| `matchup.assassin_ambush` | 53% | 47..59 | 45..90 | ok |
| `matchup.dio_boss` | 68% | 62..74 | 35..75 | ok |
| `matchup.dancer_rush` | 70% | 65..75 | 60..90 | ok |
| `matchup.weaver_gambit` | 77% | 72..82 | 58..86 | ok |
| `matchup.bell_race` | 68% | 63..73 | 48..80 | ok |

Six encounters, 1800 battles, all inside their declared bands, and every
interval is inside its band rather than merely the point estimate.
`matchup.dio_boss` remains the row to watch: it sits 7 points under its ceiling
against an interval half that wide. `matchup.weaver_gambit` is the second, at 9
points under a ceiling of 86 -- and its band is the narrowest in the file at 28
points, which the sensitivity finding below argues is close to the minimum that
can honestly be declared.

The first three rows are the control for issue #16 and reproduce bit for bit
across all ten of its commits.

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

> **Amended by issue #15 -- the model has no term for the defence toll, and that
> is where it broke.** Every calibration above varies `atk` while holding `power`
> at or above 100. Issue #15 varied `power` instead, to 85, and the model as
> written says a power-85 skill delivers 85% of a power-100 skill. It does not.
> The defence subtraction happens *after* the power scaling, so the loss is
> `power_deficit x atk / 100` against a hit that has already had `def / 2`
> removed -- against the Iron Brawler that is 45 damage falling to 34, a 24% loss
> from a 15% power cut, and against Dio 42 falling to 27, a **36% loss**. The
> multiplier the model implies is right only when defence is zero, and it gets
> worse as the target hardens. **When sizing a change to `power` rather than to
> `atk`, compute the hit against each specific target's defence; do not scale.**

> **Superseded for foe sizing by issue #16, which measured the model's own
> quantities ten times and found a better form.** The version above estimates a
> foe's output from speed shares of a battle length that is itself estimated. #16
> inverted it, because the quantity that is actually stable across fixtures is
> **party damage per party action**, measured at 87 to 110 across every reading
> in this file:
>
> ```
> foe actions/battle ~ (foe HP / party damage per party action)
>                    x (foe action rate / party combined action rate)
> ```
>
> where `action rate = spd / mean tempo cost`, weighted by the measured skill
> mix. The party's combined rate is **0.169**. Measured rates: Jotaro 0.091,
> Kakyoin 0.078, Blade Dancer 0.137, Flame Assassin 0.088, Hex Weaver 0.0833,
> Iron Brawler 0.058, Requiem Bell **0.0242**. This form back-checks to within
> ~13% on the Iron Brawler across six fixtures and it is the only version that
> predicted the Requiem Bell's 1.24 actions per battle in advance. **A skill's
> `tempo_cost` is as much a part of a foe's output as its speed is, and the
> earlier model has no term for it.**

> **One further correction from #16, on the focus-fire discount.** When
> estimating how much damage will land on **one** particular foe, this file had
> been multiplying the party's output by 0.6-0.65, taken from
> `FOCUS_FIRE_CHANCE = 55`. That discount is **too pessimistic when the low-HP
> foe stays the low-HP foe**, because every re-selection finds the same target
> and the redirection compounds in one direction instead of averaging out.
> `bell_race` recorded both models before the run and the reading chose between
> them: undiscounted 95.6 damage per action predicted death near tick 48 and
> won; the discounted ~64 predicted tick 52 and lost.

The two things it makes obvious, both of which the Change E prediction missed:

- **A slow foe is a cheap foe.** The Brawler's 72 speed buys it 20% of the
  actions in the fight, so its 73 damage per action becomes 285 per battle.
  Speed multiplies damage as directly as attack does.

  > **Sharpened by issue #16.** "A slow foe is a cheap foe" is true and it is not
  > about speed alone. The Requiem Bell has the lowest `spd` in the game at 40
  > *and* the highest `tempo_cost` at 2000, giving it an action rate of 0.0242
  > against the Blade Dancer's 0.137 -- a factor of 5.7 from two fields rather
  > than one. It took **1.24 actions per battle** against the Dancer's 5.03.
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

> **Issue #16 found the exception, and it is a narrow one.** HP is inert on a
> *damage dealer* for exactly the reason given above. It is the strongest
> available lever on a **support** enemy, because a debuffer's contribution is
> its uptime and uptime is HP. The Hex Weaver went 480 -> 560 -> 690 and the
> encounter moved 92% -> 88% -> 77%; the Blade Dancer went 340 -> 620 and it
> moved 95% -> 70%. **HP on a debuffer is an uptime knob, not a durability
> knob**, and the two behave nothing alike.

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

> **Issue #16 records the design consequence of `FOCUS_FIRE_CHANCE` that this
> entry did not anticipate: it makes any non-damaging enemy self-defeating.** A
> pure support foe deals little damage, so it is the lowest-HP foe for most of
> the fight, so the party's own focus fire deletes it first -- and the more
> valuable its debuff, the less of it the party ever sees. The Hex Weaver needed
> 690 HP, the largest foe pool outside Dio, purely to survive long enough to
> cast. The Requiem Bell inverted the problem deliberately by hitting hard
> enough to draw fire on purpose, and the inversion held: its `alive%` reads
> **7%**, so the party did aim at it. Filed as a separate issue.

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

> **Quantified by issue #16, and the slope is three times steeper than this file
> has been using.** The Hex Weaver produced a clean measurement of it: party
> damage per party action moved 89.8 -> 96.0, a rise of **6.9%**, and the win
> rate moved **ten points**, 77% -> 87%. That is **~1.45 points of win rate per
> 1% of party damage per action**, against the ~0.4 assumed in six consecutive
> #16 predictions and in the sizing arguments above. Two consequences, both
> retroactive. First, most of the "wrong direction" prediction misses in this
> file were the right direction with the wrong gain. Second, **the minimum
> defensible band width is about 25 points**, because a 1% error in a foe's
> effective output -- which is inside what any of these models can resolve --
> moves the win rate by 1.45. Bands this file has called generous were closer to
> the floor than to the ceiling.

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

> **Bounded by issue #15, which tested it the only way it can be tested: by
> taking Jotaro out.** `probe.chariot_duel` is `assassin_ambush` with Polnareff
> in Jotaro's place and reads **4%**, against the control's 53%. So the claim
> survives as a statement about *this* encounter. What #15 adds is that the
> tracking relationship does not generalise: in the four-person boss fight
> Jotaro's survival reads 62% against a 68% win rate, and Polnareff's 53% and
> Josuke's 59% are close behind it. A party of three with one carry produces a
> survival figure that tracks the win rate; a party of four does not, because no
> single member is load-bearing enough. **"Jotaro is the win condition" is a
> property of the two-person ambush, not a law of the game.**

> **Issue #16 adds three more two-person fixtures and the tracking holds in all
> three**, which is the strongest form of the bounded claim: Jotaro's `alive%` is
> 67% against a 70% win rate in `dancer_rush`, 76% against 77% in
> `weaver_gambit`, and 67% against 68% in `bell_race`. Three points, all within
> three points of the win rate, on encounters designed independently of the
> claim. **In a two-person party this relationship is now measured nine times and
> has never failed.**

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

> **Issue #16 turned this column into a measurement instrument, using the rule
> this entry established.** Because `rolls` counts one accuracy check per target
> per attack, the count decomposes. `grave_toll` is an area attack used 232 times
> beside 140 uses of `strike`, and the Requiem Bell's `rolls` reads **578**. If
> every toll had found two party members that would be 232 x 2 + 140 = 604, so
> 578 means **206 tolls landed on two targets and 26 on one** -- the party was
> already down to a single survivor for **11.2%** of the tolls. No other column
> in the report can count that. The corollary is that a **status-only** skill
> draws no roll at all, measured four times on the Hex Weaver at 295/295,
> 370/370, 524/524 and 594/594, and that is not a property of area skills but of
> skills with no damage effect.

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

> **Issue #15 found the other end of this.** `blade_volley` at 36 SP was 3% of
> Dio's actions for two milestones and is **46%** of them once the party has four
> members, because `affected_count` prices an `all_enemies` skill per target. The
> skill was not fragile because 36 was a knife-edge price; it was fragile because
> its value depends on a number nobody thought of as a tuning input -- how many
> people are standing opposite it.

> **Issue #16 found a third face of the same mechanism, and this one is a
> ceiling rather than a knife edge.** `AGGRESSIVE_SKILL_CHANCE = 65` is rolled
> **before** `best_action` is ever called, so an aggressive combatant uses its
> best skill on at most about 65% of its turns no matter how the pricing is
> arranged. Measured shares across #16: `flurry_cut` 66%, `wither_hex` 62%,
> `grave_toll` 62%. **Content built around a single signature action gets that
> action roughly two turns in three, and there is no per-skill override.** For
> the Requiem Bell, which takes 1.24 actions per battle, that means 0.77 tolls
> per battle and **38% of battles in which the bell never rings at all**. Filed
> as a separate issue; it is a design limit rather than a defect, but it is a
> limit no amount of tuning reaches past.

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

  > **Restated after issue #15.** Thirteen skills now, eleven reachable, the same
  > two not. #15 added `skill.riposte`, which is reachable and heavily used, and
  > it did **not** make `guard_stance` or `rage_focus` reachable -- `ai.rs`
  > already carries tests pinning both as correctly declined at the potencies the
  > game ships. #29 is untouched.

  > **Restated again after issue #16. Sixteen skills, fourteen reachable, the
  > same two not.** `flurry_cut`, `wither_hex` and `grave_toll` are all reachable
  > and all heavily used, at 66%, 62% and 62% of their carriers' turns -- which
  > is the `AGGRESSIVE_SKILL_CHANCE` ceiling rather than a pricing result. #29 is
  > still untouched, and it now has a ready answer: `ai.rs` carries
  > `bracing_is_declined_at_the_potency_the_game_ships` and
  > `an_attack_buff_worth_less_than_one_hit_is_declined`, so both skills are
  > declined correctly on their own numbers and the issue can be closed by
  > citation rather than by measurement.

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

> **All three of issue #16's enemies carry no resistance table at all**, and that
> was a deliberate choice rather than an omission: three new encounters that each
> vary one axis of enemy design are only readable if the damage pipeline is held
> flat across them. The Iron Brawler is the constant foe in all three and does
> carry its table, which is why its eight measured `dealt/b` values are
> comparable to each other and to `assassin_ambush`.

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
>
> **The manual instruction above is superseded by issues #30 and #31.** "Diff it
> before you trust it" was the right diagnosis and the wrong remedy: it asked a
> human to remember something, every time, forever. `validate_data.py --mirror`
> now compares the seven `MIRROR_FIELDS` of every mirrored combatant against
> `data/` and exits non-zero on any difference, and CI runs it on every push. The
> guard was verified in both directions -- green on the real files, and red on a
> one-point change to a single `hp` value. Diffing by hand is no longer the
> procedure; it is the fallback for a case the guard does not cover, and the
> known gap is that the mirror check protects only combatants that appear in the
> probe roster at all.
>
> **Issue #16 walked straight into that known gap and it cost nothing, this
> time.** None of the Blade Dancer, Hex Weaver or Requiem Bell appears in any
> probe fixture, so none of the three is mirrored and none is checked. The mirror
> run still reads `8 mirrored` with 10 warnings, exactly as before, which is
> correct behaviour and is also the shape of a silent gap: adding three
> combatants moved the guard's coverage from five of eight to **five of eleven**
> without producing a single line of output anywhere.

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

> **Issue #15 found the same blindness with a larger price tag.** #22 is about
> the scorer ignoring the target's resistance table. #33 is about the scorer
> ignoring the target's *defence*, which is not a table but the oldest number in
> the game. Both come from `score_action` pricing `Effect::Damage` from `power`
> alone, with no access to the thing it is about to hit. They should be fixed
> together or neither, because either fix alone changes every AI test's
> arithmetic and voids the curve.

> **Issue #16 names the third member of the family, and it is the one with no
> issue number until now.** #22 is resistance-blindness, #33 is
> defence-blindness, and both are about `score_action` not seeing its target.
> The third is that `score_action` **has no term for `tempo_cost` at all** -- not
> a blindness to the target but to the actor's own cost. It priced
> `flurry_cut`'s 700 and `grave_toll`'s 2000 identically to `strike`'s 1000. The
> Requiem Bell is designed *entirely* around `tempo_cost`, so its whole design
> premise is invisible to the AI that plays it. This is the mirror image of #33:
> #33 is a skill chosen despite being worse than free, and this is a skill's
> single most important property never entering the comparison.

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

> **Issue #16 found the gap this guard still leaves, and had to design around
> it.** The guard asks whether a combatant did *something* measurable. It cannot
> ask whether that something mattered. A pure debuffer deals no damage and heals
> nobody, so `no_combatant_sits_out_the_whole_batch` would have failed the Hex
> Weaver as inert while it was quietly cutting Jotaro's hits by 31%. The fix in
> the content was to give it `skill.strike`, which it uses on 38% of its turns --
> so the guard is satisfied by the one thing the character is *not* for. **No
> column in this report can attribute a mitigation, and the guard that exists to
> catch a do-nothing combatant is satisfied by a token attack.** Filed as a
> separate issue.

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

> **The cost of the array itself is now six times what it was.** Each shipped
> matchup carries its own literal 300-entry seed list, about 6031 bytes, and
> issue #16 took the count from three to **six** -- plus seven probe fixtures.
> Roughly 78 KB of `data/` is now a contiguous integer sequence written out by
> hand, repeated thirteen times. A `seed_count` / `seed_base` declaration would
> express the same thing in two fields. Filed; it overlaps issue #31.

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
   annotation on Change F. *(Bounded by issue #15: this holds for the two-person
   ambush and does not generalise to a four-person party. Extended by issue #16:
   it holds in three further two-person fixtures, nine measurements in all.)*
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

   > **Issue #16 used the replacement axis ten times and measured where it fails.**
   > It is calibrated to about two points between 0.43 and 0.92, and it
   > **over-calls the win rate below `a075`'s 0.61 by 3-4 points and by a full 8
   > points at 0.716**. It is also **near-degenerate in any fixture the party
   > usually wins**, because party `out/b` is then pinned to the foe HP pool by
   > definition -- 1179 against 1180, 1256 against 1260, 1368 against 1390 -- so
   > the denominator carries almost no information and the axis reduces to foe
   > output over party HP. Two of the ten readings were mis-sized by trusting it
   > past that point. The formula is also easy to misread and was misread here for
   > three consecutive commits: it is `(foe out/b / party HP pool) / (party out/b
   > / foe HP pool)`, **not** foe output divided by party output.

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

> **Issue #15 supplies a candidate mechanism, and it is not in `ai.rs`.** #33
> establishes that `emerald_snare` at power 75 against Kakyoin's atk 78 already
> deals less than his free `strike`, and that the deficit **widens with the
> target's defence**. The probes vary `atk` and hold `def` at 48, so that cannot
> be the whole story here -- but the same skill's share of Kakyoin's turns also
> moves from 28% in the ambush to 70% against Dio, whose defence is more than
> twice as high. Any explanation of #27 must account for both directions, and
> #33 is the more likely root than anything in the scorer's `atk` handling.

> **Issue #16 adds three more points and they do not settle it either.** Against
> the Blade Dancer (def 38) the snare is 40% of Kakyoin's turns, against the Hex
> Weaver (def 55) 32%, against the Requiem Bell (def 60) 35%. The Iron Brawler at
> def 48 is present in all three. Three foes spanning 22 points of defence and
> the share moves eight points non-monotonically -- so whatever drives #27, a
> single foe's defence is not it in a two-foe fight where the AI also chooses its
> target. #27 remains open and unexplained.

### What this entry does not authorise

- **No content change.** Nothing in `data/` moved and nothing needs to.
- **The curve is valid only for the current rules.** It is now correct on both
  counts that invalidated it -- swept at 300 seeds, under `0.4.0` rules -- but
  issues #15, #16, #17, #22, #27 and #29 all touch rules or the roster, and any
  of them voids it again.

  > **Issue #16 did not void it, and that is worth recording because the line
  > above expected it to.** All six probe rows reproduce their recorded values
  > exactly. None of the three new enemies appears in any probe fixture and none
  > of the four curve participants changed, so the curve survives a milestone
  > step that was listed in advance as likely to invalidate it. **What voids a
  > curve is touching its participants, not adding content beside it.**
- **The probe roster is not self-maintaining.** Diff it against
  `data/combatants.json` before every sweep.

  > **Superseded by issues #30 and #31.** `validate_data.py --mirror data` now
  > enforces this and CI runs it on every push, so the roster *is* self-
  > maintaining for every combatant that appears in it. Issue #15 is the first
  > entry to benefit: `pc.polnareff` was added to both files in one commit and
  > the guard proves them byte-identical. The residual risk is narrower than the
  > line above suggests but it is not zero -- a combatant absent from the probe
  > roster entirely is not mirrored and not checked.

---

## Issue #15 -- a fourth playable character, and a paid skill that was worse than the free one

Commits `3d59744` (content, measured red) and `07af985a` (reprice and relocate).
**A content change. Comparability boundary 6 begins here, and it is the first
boundary in this file that comes from content rather than from rules.**

### What shipped

Four additions to `data/`, nothing in `src/`:

- `skill.riposte` -- 14 SP, one enemy, 90% accuracy, power **110** physical,
  plus `atk_down` 35 potency / 3 turns / 75% chance.
- `stand.silver_chariot` -- `{ atk 16, def 14, spd 26, will 18 }`, one skill.
  The single-skill Stand has precedent in `stand.iron_hymn`.
- `pc.polnareff` -- 570 / 100 / 78 / 66 / 74 / 80, `support` profile, effective
  **atk 94, def 80, spd 100, will 98** after the Stand bonus. Speed sits
  deliberately under Jotaro's 102 so the party's action order is unchanged.
- `pc.polnareff` joins `matchup.dio_boss`.

### The design was decided by reading the engine, not by guessing at it

The character was conceived as a debuffer, on the assumption that debuffs would
need engine support. Reading `ai.rs` and `state.rs` first proved otherwise:
`AtkDown`, `DefDown`, `SpdUp` and `Regen` are all implemented, priced by
`status_points` and validated. They simply had no shipped skill using them. **No
`src/` change was needed, and one was very nearly written.**

The same read killed the original concept. An ally-facing buff is **unreachable
by construction**: `is_candidate` admits only `OneEnemy | AllEnemies | SelfOnly`,
so a `one_ally` buff can never be selected for an action slot no matter how it is
priced. That is the #29 defect exactly, and it would have shipped a dead skill
into a milestone whose acceptance criteria include reducing the count of dead
skills. It was caught by reading a function, not by running anything.

> **Issue #16 kept this practice and it paid for itself twice, then failed once
> in a way that named its own limit.** It paid when a pure debuffer was checked
> against `MatchupReport::inert_combatants` before being written, and again when
> `tick_statuses` was read rather than assumed, establishing that duration is
> counted in the *bearer's* own turns. It failed when the Requiem Bell's tempo
> arithmetic was done against `battle.rs` and `state.rs` correctly and the
> **`aggressive` profile's 65% roll in `ai.rs` was left out of the picture
> entirely**. Reading the engine is necessary and it is not a single act:
> **tempo, damage and duration are properties of the engine, but whether a skill
> is used at all is a property of `ai.rs`, and the two must be read together.**

### The two runs

| Commit | `riposte` power | `chariot_duel` lives in | thug | ambush | boss | Gate |
| --- | --- | --- | --- | --- | --- | --- |
| `4588c33` | -- | -- | 100% | 53% | **47%** | green |
| `3d59744` | **85** | `data/matchups.json` | 100% | 53% | **56%** | **RED** |
| `07af985a` | **110** | `tools/probe/` | 100% | 53% | **68%** | green |

`thug_solo` and `assassin_ambush` reproduce bit for bit across both runs.
Polnareff is in neither, so they are the control, and they are the evidence that
boundary 6 applies to the boss fight alone.

### Run 1 scorecard -- one of four

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `chariot_duel` lands 25-40% | **0%**, 300 losses from 300 | **wrong** |
| 2 | `dio_boss` rises from 47% | 56% | correct |
| 3 | `dio_boss` may breach the 75 ceiling | 56% | **wrong** |
| 4 | Jotaro's survival keeps tracking the win rate | 48% against 56% | **wrong** |

Prediction 1 is the worst single miss in this file. A 25-40% range against a
measured 0% is not a near-miss; it is a claim about the encounter's shape that
was wrong in kind. **The error was treating a defensive rider as compensation for
a 40% loss of party damage output.** Polnareff replacing Jotaro drops party
throughput from 1244 per battle to 744 against 1340 HP of foes, and no amount of
mitigation closes that in an attrition race. A debuff slows the loss; it does not
win the race.

### Run 2 scorecard -- three of three

| # | Prediction | Measured | Verdict |
| --- | --- | --- | --- |
| 1 | `dio_boss` rises to 60-70 | 68% | correct |
| 2 | `probe.chariot_duel` off zero, under 25% | 4% | correct |
| 3 | Mirror green at 8, warnings stay at 10 | 8 mirrored, 7 band + 3 unencountered | correct |

A fourth was recorded -- "may breach the 75 ceiling, and if it does the fix is
Polnareff's `atk` or `riposte`'s SP cost, never the band" -- and is **not scored**,
because "may" is not falsifiable. It is kept on the record as a pre-committed
remedy rather than as a forecast.

Prediction 1 landed and its reasoning did not. It argued Polnareff's output would
roughly double, from the arithmetic that his hit on Dio goes from 27 to 51.
Measured, his `dealt/b` went 321 -> **483**, half the predicted increase, because
the fight also shortened from 54 turns to 49 and he therefore acted less. **A
right answer from wrong reasoning is a failure that happens to score as a
success**, and it is recorded that way here.

### `matchup.dio_boss` -- 300 battles, 203 won, 97 lost, turns 49 / 30 / 74

| Combatant | dealt/b | taken/b | heal/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 1186 | 688 | 0 | 98 | 3040 | 9% | 62% |
| Josuke | 231 | 741 | **1091** | 112 | 1213 | 8% | 59% |
| Kakyoin | **177** | 822 | 0 | **137** | 3173 | 10% | 45% |
| Polnareff | 483 | 740 | 0 | 125 | 2984 | 10% | 53% |
| Dio | 2833 | 1436 | 34 | 205 | 7757 | 11% | 32% |
| Flame Assassin | 150 | 640 | 0 | 23 | 694 | 8% | 0% |

Actions: Jotaro `rush_barrage 1637 (54%), strike 1403 (46%)`; Josuke
`restore 1276 (51%), strike 838 (34%), concussive_slam 375 (15%)`; Kakyoin
`emerald_snare 1740 (70%), emerald_splash 692 (28%), strike 49 (2%)`; Polnareff
`riposte 2690 (90%), strike 294 (10%)`; Dio `blade_volley 1464 (46%),
strike 1349 (42%), blood_drain 205 (6%), concussive_slam 97 (3%),
tempo_halt 95 (3%)`; Assassin `sun_flare 435 (63%), strike 259 (37%)`.

### Finding 1 -- the shipped skill was strictly worse than the free attack it displaced

This is the reason run 1 failed, and it is the most transferable thing in the
entry. Damage is `max(power * atk / 100 - def / 2, 1)`. The defence toll is
subtracted **after** the power scaling, so at Polnareff's effective atk 94:

| Target | `riposte` at power 85 | free `strike` at power 100 |
| --- | --- | --- |
| Iron Brawler (def 66, physical 25) | 79 - 33 = 46 -> **34** | 94 - 33 = 61 -> **45** |
| Flame Assassin (def 60) | **49** | **64** |
| Dio (def 104) | **27** | **42** |

A 15% cut in power produced a 24% cut in damage against the Brawler and a **36%**
cut against Dio. The skill cost 14 SP to deal less damage than the free option,
at worse accuracy, and the gap widened against exactly the targets it was
designed for.

**The AI chose it on 99% of Polnareff's turns anyway.** `status_points` prices
`AtkDown` at `baseline * potency / 100 * duration`, roughly 98 power-units for a
35/3 debuff, and an 11-to-15 point damage deficit disappears inside that. The
scorer was not malfunctioning; it cannot see the defence toll, because
`score_action` never receives the target. Power 110 is the minimum that puts the
paid skill above the free one against every current target.

**The defect is in shipped content too, and predates this branch.**
`skill.emerald_snare` is power 75 against Kakyoin's effective atk 78. Against Dio
it delivers **4 damage** where his free `strike` delivers 26, and he spends 15 SP
on it for 70% of his turns -- which is why his `dealt/b` in the table above is 177
against 344 in the ambush, on nearly double the SP. **Filed as #33 and
deliberately not fixed here**, because raising `emerald_snare` moves
`assassin_ambush`, which sits on the steepest part of the measured curve and is
the control for this entire entry.

The validator warns when a *free* skill's total power exceeds 130 and dominates
the basic attack. It has no warning for the inverse, which is the failure that
actually shipped.

> **Issue #16 used this finding as a design constraint rather than rediscovering
> it.** Every new skill was priced against the free attack before it was written:
> `flurry_cut` at power 105 and 10 SP on a 92-atk carrier, `grave_toll` at power
> 240, and `wither_hex` deliberately given **no damage effect at all** so the
> comparison does not arise. `wither_hex` is the first skill in the game with no
> `damage` effect, and it is reachable precisely because `status_points` prices
> an `atk_down` at 26/8 across two targets at 354 points against a baseline of
> around 100.

### Finding 2 -- the debuff works, and the first analysis of it was confounded

The first check compared damage per **battle turn** between `chariot_duel` and
the control: 44.9 against 45.5, about 1%, which reads as "the status is inert".
That comparison is wrong, because foe survival differs sharply between the two
fixtures -- 94% and 100% in the duel against 16% and 42% in the ambush -- so the
same number of battle turns contains very different numbers of foe actions.

Normalised per **foe action**, at an average party defence of 72 in both
fixtures: chariot `1078 / 13.1 = 82` against ambush `975 / 9.3 = 105`. A **22%
cut in enemy damage per swing**, close to the intended potency and uptime.

**Normalise per actor action, not per battle turn, whenever two fixtures differ
in how long their combatants survive.** The confounded version was one sentence
from being published as evidence that a working feature did nothing.

### Finding 3 -- a debuff's value scales with party size, and this was not stated in advance

The same character moved `dio_boss` up 21 points and left `chariot_duel` at 4%.
The reason is structural rather than numeric: a debuff protects **everyone**, so
its value scales with the number of allies benefiting, while the cost -- the
offence forgone by the debuffer -- is fixed. In a two-person party the debuffer is
half the party's damage. In a four-person party he is a quarter of it and
protects three others.

This should have been written down before the fixture was designed. It is
predictable from the design of `status_points` without running anything.

### Finding 4 -- a fourth party member changed the boss's behaviour, not just his difficulty

Nothing on Dio's sheet changed. His action mix did:

| Skill | Three-person party | Four-person party |
| --- | --- | --- |
| `blade_volley` | 139 uses, **3%** | 1464 uses, **46%** |
| `tempo_halt` | 1188 uses, **23%** | 95 uses, **3%** |
| `strike` | 2698 uses, 53% | 1349 uses, 42% |

`affected_count` multiplies an `all_enemies` skill's value by the number of
targets, so a fourth body reorganised the boss around his area attack. His
`dealt/b` rose 2304 -> 2833 while his action count per battle fell 17.0 -> 11.9.

**Party composition is an input to enemy behaviour, not only to enemy
difficulty.** Issue #16 adds three enemies, at least one of which is likely to
carry an area skill, and this is the mechanism that will decide whether they are
threatening or trivial. It also retroactively explains issue #9's finding 3:
`blade_volley` looked fragile at 36 SP because its value was being measured
against three targets.

### Finding 5 -- "Jotaro is the win condition" is a property of the ambush, not a law

`probe.chariot_duel` is the direct test: remove Jotaro, keep everything else, and
the encounter reads 4% against the control's 53%. So the claim holds where it was
made.

It does not generalise. In the four-person boss fight Jotaro's survival is 62%
against a 68% win rate, with Polnareff at 53% and Josuke at 59% close behind.
The tight tracking measured across all six curve rows is a property of a
two-person party with one carry, not of the engine. See the bound added to
Change F.

### Why moving `chariot_duel` to the probe file is not band-fitting

It is worth stating explicitly, because it has the same shape as the thing this
file exists to prevent. The argument is **categorical, not numeric**:
`chariot_duel` was authored as "`assassin_ambush` with one variable changed",
which is the definition of a probe. It would belong in `tools/probe/` at 53%
exactly as much as at 0%. The reading is preserved, not discarded -- it is in the
sweep and in the table above.

The test of that claim is the counterfactual: had it read 15% against a 20..60
band, widening the band to 10..60 would have been the dishonest fix, and the file
would **still** have been the wrong home for it. Both statements are true at
once, which is what distinguishes relocating a fixture from rescuing a number.

### What this entry does not authorise

- **No band is re-declared.** `dio_boss` at 68% is inside 35..75. It is 7 points
  under the ceiling against a +/-6 interval, so it is inside but not comfortably,
  and the pre-committed remedy if a future change pushes it out is Polnareff's
  `atk` or `riposte`'s SP cost -- never the band.
- **`emerald_snare` is not touched.** See #33 and the note in finding 1.
- **#29 is not closed.** Eleven of thirteen skills reachable; the same two are
  not, and `ai.rs` already pins both as correctly declined.
- **The curve is not voided.** Polnareff appears in no curve probe, and all six
  rows reproduce their recorded values exactly: 96 / 81 / 53 / 34 / 20 / 12.
  `probe.curve_a135` reports 158 won and 142 lost, identical to the shipped
  ambush in every digit, which is what makes the other six readable.
- **The 47% -> 68% movement is not a measurement of anything.** See boundary 6.

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
- **~~The probe roster is a hand-maintained copy and drifts silently.~~ Guarded
  since issues #30 and #31.** It carried no resistance tables for two milestones
  without any error, because `resist` is `#[serde(default)]`: a schema field that
  defaults gracefully makes migration painless and makes drift invisible.
  `validate_data.py --mirror data` now compares the seven `MIRROR_FIELDS` of every
  mirrored combatant against `data/` and exits non-zero on any difference, and CI
  runs it on every push. The guard was verified in both directions, including a
  deliberate one-point `hp` change that correctly turned it red. **Two gaps
  remain:** a combatant absent from the probe roster entirely is not mirrored and
  therefore not checked, and `stand` is deliberately outside `MIRROR_FIELDS`
  because the probe clones legitimately differ there. A control row that does not
  reproduce shipped content in every digit is still proof the sweep is void.
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
- **The scorer prices a skill without ever seeing its target.** `score_action`
  reads `Effect::Damage.power` and knows nothing about the defence or the
  resistance table of the thing it is about to hit. Two separate issues are the
  same root: #22 (resistance-blind) and #33 (defence-blind). #33 is the more
  expensive of the two -- it let a paid skill that deals less damage than the free
  basic attack be chosen on 99% of a character's turns, and it is present in
  shipped content in `skill.emerald_snare`. Any fix changes every AI test's
  arithmetic and voids the curve, so they should be done together.
- **Nothing warns when a paid skill is weaker than the basic attack.** The
  validator warns in the opposite direction only, when a *free* skill's total
  power exceeds 130. Tracked with #33.
- **SP recovery is silent.** `recover_sp` deliberately emits no event, so the
  Godot HUD cannot show a player where their SP came from. `sp/b` above a
  combatant's `max_sp` is the only visible signal.
- **`sp/b` is derived by the CLI, not by the report.** `bin/balance.rs` computes
  it with truncating integer division while every neighbouring column rounds half
  up through `divide_rounded`. Cosmetic in size; tracked as issue #25.
- **A rules change resets the series.** RNG draw order is part of the rules, so
  after any edit to `ai.rs`, `resolve.rs`, `state.rs` or `battle.rs` the same seed
  no longer reproduces the same battle. Issue #9 crossed this boundary six times;
  issue #10 crossed it once; issues #11, #12, #14 and #15 crossed it zero times.
- **A roster change resets it too, and that is easier to miss.** Issue #15 added
  one combatant to one party and every figure in that encounter moved, including
  the *enemy's* choice of skill. A content edit that changes who is standing in a
  fight is a comparability boundary even though no code changed. See boundary 6.
- **One row survived a boundary it should not have.** `Flame Assassin` in
  `dio_boss` was effectively constant across two boundaries (305 at `7f72a4f`,
  304 at `f6ba445`). Until that is explained, treat "bit-identical" as evidence
  only when a *whole matchup* reproduces, never a single row.
- **The harness plays worse than a player.** Auto-battle scores each affordable
  skill once and never sets up a combination across turns. An AI-vs-AI win rate
  is therefore a floor for a competent player, not an estimate of their
  experience.
- **The AI's skill choice responds to enemy `atk` through an unidentified
  channel.** Kakyoin's `emerald_snare` share falls monotonically as the enemy
  gets stronger, with nothing the skill counters having changed. Tracked as issue
  #27, with a candidate mechanism from #33 noted in the #14 entry. Until it is
  explained, treat any measured action mix as descriptive rather than as evidence
  of intended behaviour.
- **An enemy's action mix depends on how many people it is fighting.** Dio's
  `blade_volley` share went 3% -> 46% when the party gained a fourth member,
  because `affected_count` prices an area skill per target. Any action mix
  recorded in this file is a property of the party that was standing opposite it.
- **~~Only five of eleven skills are ever used.~~ Eleven of thirteen as of
  `07af985a`.** Two remain: `guard_stance` and `rage_focus`, both declined
  correctly on their own numbers -- and unlike earlier statements of this line,
  "correctly" is now verified: `ai.rs` carries
  `bracing_is_declined_at_the_potency_the_game_ships` and
  `an_attack_buff_worth_less_than_one_hit_is_declined`. Tracked as issue **#29**
  (previously miscited here as #15 and #16).
- **`skill.tempo_halt` is still mispriced, in the other direction.** The scorer
  values a lock as the actions it *denies*; a lock defers, not removes. Dio used
  it on 23% of his turns against a three-person party and 3% against a
  four-person one, so the mispricing is now masked rather than fixed. If #29
  finds a missing payoff horizon, this and #29 are the same defect seen from
  opposite sides.
- **The curve in [`BALANCE-CURVE.md`](BALANCE-CURVE.md) is a property of the
  current rules, not a constant.** Replace the table rather than appending to it,
  and re-sweep after any change to `resolve.rs`, `ai.rs`, the party roster or the
  skill list.
