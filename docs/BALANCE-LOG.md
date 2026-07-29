# Balance log

Every balance change to `data/*.json`, the measurement that motivated it, and the
measurement that followed it. One entry per change.

The rule this file exists to enforce: **change one number, re-run, record.** Two
numbers at once and the result is uninterpretable, because either change could
have produced it. This is slower and it is the only version that produces
knowledge rather than opinion.

Predictions are written before the run, not after. A prediction recorded after
the fact is a rationalisation, and **seven of the nine below are wrong**. The two
that are right are the last two, and neither was a better guess: one came from
sweeping the curve instead of extrapolating into it, and the other was not a
prediction at all but a measurement repeated on shipped content. That is the
argument for the harness, not against it -- the same wrong guesses shipped as
content, unmeasured, would have been indistinguishable from design.

All numbers come from:

```powershell
cd src
cargo run -q -p rpg-core --bin balance
```

over the fixed seed list in [`data/matchups.json`](../data/matchups.json).
Seeds are fixed precisely so two rows of this table can be compared.

> **Comparability boundary.** Change C alters `ai::choose`, which changes both
> the decisions taken and the order in which the RNG is drawn. Every number
> recorded above that entry is historical. Do not compare it to anything below.

> **Second comparability boundary.** The `0.4.0` scored-choice change (issue #9,
> the last entry in this file) alters which skill `ai::choose` selects. Every
> number recorded *above* that entry describes rules in which three skills could
> never be chosen. Do not compare across it either.

## Current status

After Change G, commit `e3d4a85`, and unchanged by the `miss%` accounting fix in
`0.4.0` (issue #8), which moved one reported column and no win rate:

| Matchup | Win rate | Band | Status |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 | ok |
| `matchup.assassin_ambush` | 67% | 45..90 | ok -- closed by Change G |
| `matchup.dio_boss` | 50% | 35..75 | ok -- dead centre, closed |

All three encounters are inside their declared bands, so the harness became a
blocking CI job in commit `45c671e`.

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

The two things it makes obvious, both of which the Change E prediction missed:

- **A slow foe is a cheap foe.** The Brawler's 72 speed buys it 20% of the
  actions in the fight, so its 73 damage per action becomes 285 per battle.
  Speed multiplies damage as directly as attack does.
- **A foe's output is capped by its SP, not by the clock.** The Assassin has 70
  SP and `sun_flare` costs 16, so it has at most four expensive turns in it. Its
  per-battle damage has been 353, 332 and 308 across fights of 13, 19 and 18
  turns -- effectively a constant, independent of how long it lives.

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

**Why this lever first:** it is the only single number that pushes both
out-of-band matchups in the same direction, and it does not touch the party at
all, so the Kakyoin and Josuke findings stay measurable afterwards.

**Prediction:** `matchup.assassin_ambush` drops out of 100%. `matchup.dio_boss`
drops by less, because there the Assassin is competing with Dio for the party's
attention.

**Result: the prediction was wrong in both directions.**

| Matchup | Before | After | Predicted |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | unchanged |
| `matchup.assassin_ambush` | 100% | **100%** | drop |
| `matchup.dio_boss` | 92% | **83%** | small drop |

The matchup predicted to move did not move at all, and the matchup predicted to
barely move produced the entire effect. The Assassin's own numbers responded
exactly as intended -- lifetime damage 318 -> 396 in the ambush, 137 -> 209 in
the boss fight, median length 11 -> 14 and 45 -> 49 turns -- so the change did
what it was supposed to do. The prediction was wrong about what that would
achieve.

### Why HP could never have fixed the ambush

Lifetime damage is DPS multiplied by lifetime, and adding HP only touches the
second factor. In the ambush the party out-damages the enemy side by roughly
2.3 to 1, so lengthening the fight lengthens it for both sides and leaves the
ratio untouched:

- Enemy damage per battle after the change: 396 + 12 = **408**
- Party HP pool with no healer: 620 + 520 = **1140**

To close a 2.8x shortfall with HP alone, the Assassin would need roughly 1790
HP -- five times a mid-tier enemy's budget, and a fight that takes 40 turns to
win. **HP is the wrong lever for this encounter; damage per turn is.**

The boss fight responded because there the extra Assassin turns happen while Dio
is still alive, so they add pressure to a race that was already close: party
42 damage per turn against 2160 enemy HP, enemy 39 per turn against a party pool
of 1860 plus roughly 950 healed.

### The structural finding

Both sides focus-fire the lowest-HP target, so the side with more damage per turn
deletes the opposing roster one member at a time while the loser's output decays
with every death. The advantage compounds instead of averaging out. That is why
a 2.3 to 1 DPS edge produces a 100% win rate rather than an 85% one, and it is
the same rule that kills Kakyoin in every battle.

No number in `data/*.json` fixes this. It is `ai::choose`.

---

## Change B -- Restore heal power 230 -> 140

Commit `b13e8f9`.

**Motivation:** finding 5, and the fact that Change A left `matchup.dio_boss` the
only matchup still reachable by tuning. Restore returned 11.5 HP per SP, and
Josuke's 83 SP per battle converted that into roughly 950 HP -- more than half of
all enemy output, from a character who was never targeted. At 140 the same SP
spend returns about 580.

**Why not the SP cost instead:** raising the cost would cut the number of casts,
which changes how many turns Josuke spends healing as well as how much each cast
returns. Two effects, one commit, uninterpretable result. `power` moves exactly
one quantity.

**Prediction:** `matchup.dio_boss` lands between 60% and 78%, so it may still sit
above the 75% band edge. `matchup.assassin_ambush` and `matchup.thug_solo` are
unaffected, because neither party contains Josuke.

**Result: no movement whatsoever.** 83% before, 83% after; the same 10 wins and
the same 2 losses. The prediction was wrong for the third time, and this time it
was wrong about the *direction being detectable at all*.

| Matchup | Before | After |
| --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% |
| `matchup.assassin_ambush` | 100% | 100% |
| `matchup.dio_boss` | 83% | **83%** |

`matchup.dio_boss`, per battle, before -> after:

| Combatant | Dealt | Taken | SP | Alive |
| --- | --- | --- | --- | --- |
| Jotaro | 1262 -> 1284 | 562 -> 547 | 72 -> 72 | 50% -> **42%** |
| Josuke | 470 -> 461 | 218 -> 273 | 83 -> 83 | 83% -> 83% |
| Kakyoin | 308 -> 295 | 1145 -> **1053** | 126 -> 122 | 0% -> 0% |
| Dio | 1717 -> **1665** | 1400 -> 1400 | 181 -> 177 | 17% -> 17% |
| Flame Assassin | 209 -> 209 | 640 -> 640 | 33 -> 33 | 16% -> 16% |

### Why removing 370 HP of healing changed nothing

Josuke's SP spend is identical, so he cast the same number of heals; only their
size fell. The healing that disappeared was healing that did not matter:

- Support triage fires below 55% max HP and picks the **most wounded ally**,
  which in this encounter is almost always Kakyoin -- who is simultaneously the
  focus-fire target of every hostile actor. HP poured into him is removed again
  before his next turn. He still dies in 12 of 12 battles.
- Dio's damage *fell* by 52 and Kakyoin's damage taken fell by 92. Smaller heals
  mean Kakyoin dies slightly sooner, which means the enemy stops shooting a
  corpse and the total damage the enemy manages to deal goes **down**. Part of
  the nerf refunded itself.
- What actually decides the fight is whether Jotaro, at 1284 damage per battle
  out of the party's 2040, out-lives Dio's 1520 effective HP. Healing Kakyoin
  does not enter that race. Jotaro's survival fell 50% -> 42% and the outcome
  still did not move.

**Conclusion: healing magnitude is a third-order lever in this encounter, and
targeting is the first-order one.** Two content changes have now been spent to
learn that the same rule is behind every finding. That is the case for fixing
the rule rather than paying for it repeatedly, one commit at a time.

The 140 value is kept. It was independently defensible -- 11.5 HP per SP was
over-priced healing -- and reverting it would only add a variable.

---

## Change C -- weighted target selection in `ai::choose`

Commit `87c8220`. **This is a rules change, not a content change.**

**Motivation:** every finding above reduces to one rule. Unconditional lowest-HP
focus fire makes an advantage compound, because each kill removes output from the
losing side permanently while the winning side keeps all of its own. Two
consequences were measured, not guessed:

- a 2.3 : 1 damage-per-turn edge produced a **100%** win rate in
  `matchup.assassin_ambush`, where a fair fight of that ratio should sit near the
  top of the 45..90 band, not past it;
- Kakyoin was downed in **12 of 12** boss battles, absorbing 63% of all enemy
  damage, because he is deterministically the softest target from turn one.

Both are the same bug seen from the two sides of the board.

**The change:** a new `FOCUS_FIRE_CHANCE = 55` constant. Every hostile action now
commits to the weakest enemy 55% of the time and picks a uniformly random living
enemy otherwise. `AiProfile::Trickster` is untouched (already fully random) and
healer triage is untouched (still the most wounded ally) -- healing the wrong
ally would be a different bug.

**Why 55 and not 100 or 0:** at 100 the advantage compounds, which is the defect.
At 0 the AI stops finishing wounded enemies, fights lengthen without becoming
harder, and enemy behaviour becomes illegible to a player -- the property
`ai.rs` was written to protect. 55 keeps a wounded target the single most likely
victim while making no death certain.

**Prediction:** `thug_solo` unchanged at 100%; `assassin_ambush` 80-95%;
`dio_boss` 70-88%, with the direction explicitly allowed to be upward.

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

`matchup.assassin_ambush`, per battle, before -> after:

| Combatant | Dealt | Taken | Alive |
| --- | --- | --- | --- |
| Jotaro | 662 -> 676 | 10 -> **106** | 100% -> 100% |
| Kakyoin | 278 -> 264 | 404 -> **272** | 92% -> **100%** |
| Flame Assassin | 396 -> **353** | 640 -> 640 | 0% -> 0% |

### Focus fire was the enemy's weapon, not a symmetric rule

The rule was symmetric in code and wildly asymmetric in effect. A kill is only
worth something if the dead combatant was producing something:

- **What the enemy lost.** Deleting Kakyoin removed a third of the party's output
  for most of the fight. Under weighted targeting Kakyoin survives 33% of
  battles, his damage rises 295 -> 410, and total party output rises 2040 ->
  2160. Enemy output falls 1874 -> 1689, partly because damage now lands on
  Jotaro's def 70 and Josuke's def 78 instead of Kakyoin's def 58.
- **What the party lost: nothing.** Its focus-fire targets were the 300 HP thug
  (13 damage per battle) and the Assassin (215). Killing them early was never
  worth much, so giving that up cost the party almost nothing.

Net effect: **the party gained an entire combatant and the enemy gave up its only
real tactic.** Dio now dies in 12 of 12 battles, taking his full 1520 effective
HP every time.

This is not an argument to revert. A guaranteed casualty is a broken rule
regardless of which way it moves the win rate, and 83% was never a legitimate
83% -- it was a boss fight that the party was surviving by feeding it a
sacrifice. The correct reading is that the encounter's *content* was always far
too weak, and focus fire was hiding it.

### The ambush is not reachable by any combatant stat

The ambush did not move for the third time in a row, and the arithmetic now says
it cannot:

- Enemy lifetime damage: 353 + 13 = **366**
- Party pool, no healer: 620 + 520 = **1140**, of which 378 is actually spent

The enemy side needs roughly **3.1x** its current output before the party is at
risk. That means the Assassin's atk at around 235 against Kakyoin's def 58, on a
mid-tier enemy whose budget is 76. It is the same wall Change A hit from the HP
side, and it is not a tuning failure -- it is the encounter definition. Two
fully-equipped Stand users against one mid-tier enemy and a 300 HP thug is not a
45-90% fight, and the `data/matchups.json` band asserts that it is.

**This requires a decision, not a number.** Either the roster changes (a second
real threat instead of the thug), or the band changes to state that this
encounter is designed to be won. Both are legitimate; silently editing the band
to match the measurement is not, which is why it is not being done here.

### Also fixed in the same run

`sim::tests::damage_is_attributed_to_both_sides_of_every_hit` failed on
`times_downed == 1` against an actual 4. The statistic accumulates over the whole
seed list, so a four-seed hopeless matchup downs the hero four times. This test
had never been executed before -- it was written and pushed without a local run --
so the failure is a wrong expectation, not a regression from Change C.
Commit `a92600a` corrects it and also replaces the vestigial duplicate-seed check
(a `seen` vector guarded by `any(|_| false)`, which could never fire) with a real
membership test plus a test that covers it.

---

## Change D -- Dio atk 100 -> 130

Commit `b279cda`.

**Motivation:** with targeting fixed, the boss fight reduces to one ratio, and
both sides of it are now measured rather than assumed:

- Enemy lifetime damage: 1474 + 215 = **1689**
- Party effective HP: 620 + 720 + 520 = 1860, plus roughly 580 healed = **2440**

The enemy needs about **45% more output** before a loss is possible at all, and
something closer to parity before the declared 35..75 band is reachable. Attack
is the direct lever: Dio already converts 100% of his HP into lifetime, so buying
him more turns yields less than making each turn hurt more.

**Why Dio and not the Assassin:** Dio produces 87% of enemy damage in this
encounter. A change to the 13% cannot move the result, as Change A demonstrated
at some cost.

**Prediction:** 78-92%, deliberately still out of band, as a calibration step to
measure how much win rate one point of enemy attack buys.

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

+30% attack produced +37% damage (1474 -> 2023) and **-50 points of win rate**.
The two-parameter arithmetic behind the prediction was right; the conversion from
damage to win rate was wrong by a factor of roughly three.

- Enemy lifetime damage: 1689 -> **2250**
- Party effective HP: **2440**

The fight is a race, and a race resolves as a **step function around parity**, not
as a slope. At 69% of the party's pool the enemy loses every time; at 92% it wins
half. That single fact explains every earlier surprise in this document:

- Changes A and B looked inert because they moved a ratio that was far from
  parity. Nothing was going to show there.
- Change D overshot because it moved a ratio that had arrived at parity, where
  the derivative is at its steepest.

> **Retracted.** This entry originally continued with a practical rule naming
> **0.8** as the start of the responsive zone and 1.2 as its end. Those numbers
> were never measured; they were interpolated between two points and then quoted
> as findings in three later entries. The sweep recorded in
> [`BALANCE-CURVE.md`](BALANCE-CURVE.md) puts the bend between **0.65 and 0.74**,
> with a plateau at 67% from 0.74 to 0.81 and the steep region running 0.81 to
> 0.99. Sizing Changes E and F against the invented 0.8 is the direct cause of
> both of those predictions failing.

Also worth recording: Dio at 50% survival means the fight now ends near
simultaneous mutual destruction, and Josuke's damage taken tripled (336 -> 763).
The healer is now a target rather than a spectator, which is what a boss fight is
supposed to look like. That was not designed -- it fell out of one number.

**Not tuned further.** 50% is the centre of 35..75 and the harness plays worse
than a human, so this is a floor, not a ceiling. Chasing 60% would be tuning to
noise on a twelve-seed sample.

---

## Change E -- the ambush's second foe becomes the Iron Brawler

Commits `2a9564d` (Stand), `3362d16` (combatant), `a6a906e` (roster).

**Decision, not a measurement:** the encounter definition was the defect. A
300 HP thug that deals 13 damage per battle is a body, not a threat, and no stat
on it can make a two-Stand-user party lose 10-55% of the time. The roster was
changed rather than the band, because editing the band to match the measurement
would have moved the goalposts and left the encounter exactly as unthreatening as
it measured.

**Sizing, taken from measured numbers rather than invented:**

- Party throughput, measured: 940 damage over a 13-turn median = **~72 per turn**
- Party HP pool, no healer in this encounter: **1140**
- Target enemy lifetime damage: **~950**, a ratio of 0.83 -- inside the steep zone
  identified in Change D, below parity so the party is favoured but can lose
- Flame Assassin already contributes 27 per turn, so the new foe needs ~24 per
  turn and enough HP to keep the fight near 18 turns: **~700 effective HP**

That produced `npc.iron_brawler`: hp 620, sp 80, atk 80, def 48, spd 62, will 58,
carrying the new `stand.iron_hymn` (hp +80, atk +22, def +18, spd +10, will +12).
Effective 700 HP and 102 attack, deliberately slower than the Assassin's 94 speed
so it lands fewer, heavier turns.

**Its skill list is a complete SP ladder, and that is a deliberate constraint:**
`concussive_slam` 140 power at 22 SP, then `blood_drain` 120 at 12 SP, then
`strike` 100 free. `ai::best_offensive` picks the highest-power *affordable*
skill, so a skill is unreachable content unless it is the strongest option in some
SP band. This is worth stating as a general finding:

- `skill.guard_stance`, `skill.rage_focus` and every other zero-damage skill can
  never be chosen by any AI profile -- `best_offensive` filters on
  `total_power > 0` and `Trickster` filters on hostile targets. Jotaro and Josuke
  own `guard_stance` and have never once used it in any measurement in this
  document.
- `skill.blade_volley` (110 power, 24 SP) is likewise unreachable for anyone who
  also owns `concussive_slam` (140 power, 22 SP): it costs more and hits less, so
  no SP band exists where it wins.

The AI is the reason, not the data. Fixing it means giving the profiles a reason
to buff, guard and use area damage -- an M3 item, recorded here so it is not
rediscovered a third time.

> **Partially retracted.** The second bullet is true only of a combatant who owns
> both skills, which is Dio. Kakyoin owns `blade_volley` and not
> `concussive_slam`, so it was his highest-power option and was chosen in every
> ambush and boss run recorded here. That is the defect corrected in issue #8:
> because it strikes every enemy, his accuracy rolls outnumbered his actions and
> his reported `miss%` was inflated. See the #8 note below.

**Prediction:** `matchup.assassin_ambush` lands at **55-85%**, inside its 45..90
band, with an explicit warning that a miss would more likely land below the band
than above it.

**Result: 100%. Wrong, and wrong in the direction that was called unlikely --
the encounter did not move a single point.**

| Matchup | Before | After | Predicted | Band |
| --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 100% | 85..100 ok |
| `matchup.assassin_ambush` | 100% | **100%** | 55-85% | 45..90 out of band |
| `matchup.dio_boss` | 50% | 50% | 50% | 35..75 ok |

The two control matchups are **bit-identical** to the previous run, including
turn counts and per-combatant damage. Appending content to `data/stands.json`
and `data/combatants.json` provably does not perturb another matchup's RNG, which
is the one prediction in this document that has never failed.

`matchup.assassin_ambush`, per battle, before -> after:

| Combatant | Dealt | Taken | SP | Miss | Alive |
| --- | --- | --- | --- | --- | --- |
| Jotaro | 676 -> **987** | 106 -> 145 | 57 -> 70 | 5% -> 6% | 100% -> 100% |
| Kakyoin | 264 -> **362** | 272 -> **477** | 94 -> 104 | 11% -> 19% | 100% -> **50%** |
| Flame Assassin | 353 -> **332** | 640 -> 640 | 41 -> 41 | 6% -> 8% | 0% -> 0% |
| Street Thug -> Iron Brawler | 13 -> **285** | 300 -> **709** | 0 -> 58 | 0% -> 15% | 0% -> 0% |

Median length 13 -> **19** turns. Enemy lifetime damage 366 -> **617**, a **69%**
increase, for **zero** points of win rate.

### The parity rule survived its first out-of-sample test

This is the useful part of a failed prediction. Change D's rule says a ratio far
from 1.0 does not respond, and 617 against a party pool of 1140 is **0.54** -- so
by the project's own recorded rule, nothing should have moved, and nothing did.
The rule was inferred from three boss-fight measurements and has now correctly
retrodicted a fourth, in a different encounter, with a different roster.

The prediction failed because it was made **before** that rule was applied to the
sizing. The 950-damage target was right; the enemy delivered 617. The three
errors, all now folded into the throughput model at the top of this file:

1. **Speed was ignored.** The Brawler was given 72 effective speed for flavour --
   "slow, heavy" -- and speed is not flavour, it is a direct multiplier on
   lifetime damage. It took 20% of the actions and delivered 73 damage in each,
   which is 285, not the 450 the sizing assumed.
2. **The Assassin was assumed to be a constant.** Its damage was projected to
   stay at 27 per turn across a longer fight. Instead its total **fell**,
   353 -> 332, because 70 SP buys at most four `sun_flare` casts and everything
   after that is a 67-damage `strike`. Enemy output is SP-bounded.
3. **`blood_drain` was treated as a downside risk and is not one.** The Brawler
   absorbed 709 damage against 700 effective HP, so the drain returned about 9
   net HP over a whole battle. It is a 120-power attack that happens to have a
   rounding error attached.

---

## Change F -- Iron Brawler atk 80 -> 105

Commit `99a4c94`. **The first change in this document sized by arithmetic from
`resolve.rs` rather than by intuition.**

**Why attack and not speed or HP.** All three would work on paper; only attack is
both sufficient and coherent with the enemy's design:

| Lever | Value needed for ~800 enemy damage | Objection |
| --- | --- | --- |
| hp 620 -> ~1280 | 1360 effective HP | Dio-scale HP on a mid-tier ambush foe, and a 29-turn random encounter |
| spd 62 -> ~130 | 140 effective speed | Fastest unit on the field, which contradicts the enemy this was designed as |
| **atk 80 -> 105** | **126 effective attack** | Hits harder than the Flame Assassin -- intended; still below Dio's 160 |

**The arithmetic**, using the model at the top of this file. Its skill is
`concussive_slam`, power 140, accuracy 85, and the party's average defence is
about 64:

```
hit    = 140 * 126 / 100 - 32   =  144   (was 140 * 102 / 100 - 32 = 111)
action = 144 * 0.85             =  122   (was ~73 after misses)
battle = 122 * 3.8 actions      =  468   (was 285)
enemy  = 468 + 332              =  800
ratio  = 800 / 1140             = 0.70
```

**Why 0.70 and not the 0.83 that Change E aimed at.** Change D aimed at the
centre of its band and overshot the prediction by 28 points, because a ratio near
parity is where the win-rate curve is steepest. This one deliberately aims at the
**top half** of 45..90 and accepts a second iteration if it lands short.

**Prediction: 70-90%, in band.** Two named failure modes, in order of likelihood:
still 100% because 0.70 sits below the bend, or below 45% because Kakyoin dies to
four connected slams.

**Result: 100%. The first named failure mode. The model was right and the
prediction was still wrong.**

| Matchup | Before | After | Predicted | Band |
| --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 100% | 85..100 ok |
| `matchup.assassin_ambush` | 100% | **100%** | 70-90% | 45..90 out of band |
| `matchup.dio_boss` | 50% | 50% | 50% | 35..75 ok |

`matchup.assassin_ambush`, per battle, before -> after:

| Combatant | Dealt | Taken | SP | Miss | Alive |
| --- | --- | --- | --- | --- | --- |
| Jotaro | 987 -> **1031** | 145 -> **238** | 70 -> 70 | 6% -> 4% | 100% -> 100% |
| Kakyoin | 362 -> 319 | 477 -> **504** | 104 -> 86 | 19% -> 21% | 50% -> **17%** |
| Flame Assassin | 332 -> 308 | 640 -> 640 | 41 -> 38 | 8% -> 9% | 0% -> 0% |
| Iron Brawler | 285 -> **428** | 709 -> 709 | 58 -> 57 | 15% -> 12% | 0% -> 0% |

Enemy lifetime damage 617 -> **736**, ratio **0.65**. Median length 19 -> 18.

### What was learned, which is more than the win rate suggests

**The model works.** Predicted 122 damage per action, measured 116. Predicted 468
per battle, measured 428. Predicted 18.8 turns, measured 18. This is the first
time this document has predicted a mechanical quantity correctly, and it means
the remaining uncertainty is entirely in **where the bend in the win-rate curve
sits**, not in how much damage a stat buys.

**The bend is somewhere between 0.65 and 0.92.** Four measurements now bracket it:

| Encounter | Ratio | Win rate |
| --- | --- | --- |
| Ambush, Change E | 0.54 | 100% |
| Ambush, Change F | **0.65** | **100%** |
| Boss, Change C | 0.69 | 100% |
| Boss, Change D | **0.92** | **50%** |

The earlier guess of "0.8" as the start of the responsive zone is not supported by
anything; the honest statement is that the curve is flat at 0.69 and halfway down
at 0.92, and nothing has been measured in between.

### The encounter has a structural ceiling, and it is Jotaro

To reach a ratio of about 0.9 the enemy side needs roughly **1030** lifetime
damage, 40% more than it now delivers. Feeding that entire increase to one
mid-tier foe requires, by the model:

| Lever | Value required | What that makes the Iron Brawler |
| --- | --- | --- |
| atk 105 -> **164** | 186 effective | Harder-hitting than Dio (160) |
| hp 620 -> **~1450** | 1530 effective | Tougher than Dio (1400), ~30-turn random encounter |
| spd 62 -> **~110** | 120 effective | The fastest unit in the game, faster than Jotaro (102) |

**Every remaining single-stat route makes a random-encounter enemy boss-tier in
one dimension.** That is not a tuning problem; it is what the numbers say about
the encounter's composition:

- **Jotaro deals 1031 of the party's 1350 damage -- 76% -- and takes 238.** He has
  survived **100% of every battle in every run in this document**, in both
  encounters, under every change. `skill.rush_barrage` at 195 power for 18 SP is
  the largest number in `data/skills.json`, and with 80 SP he casts it four times
  before falling back to `strike`.
- **Kakyoin absorbs 504 of the 736 damage taken (68%) and now dies in 83% of
  battles**, and the party still wins every time, because the fight is decided by
  Jotaro alone.

So the ambush is not a two-versus-two fight. It is Jotaro versus two enemies, with
Kakyoin present as a target. Any enemy sized to threaten Jotaro will delete
Kakyoin instantly; any enemy Kakyoin can survive cannot threaten Jotaro.

**No content change is pushed with this entry.** The next step is a design
decision with four legitimate answers, and picking one silently would be exactly
the goalpost-moving this log exists to prevent:

1. **Remove Jotaro from the encounter.** Fictionally an ambush that catches the
   group split; mechanically it removes the 76% outlier and makes the fight about
   the two characters who actually interact with the enemy.
2. **Nerf `skill.rush_barrage`.** Honest, but it is a shared lever: Jotaro is in
   `matchup.dio_boss`, which is currently at 50% with only 15 points of slack to
   the bottom of its band, so this would very likely re-open a closed encounter.
3. **Accept a boss-tier ambush enemy** -- most likely `spd 62 -> 110`, the cheapest
   of the three by the model -- and rewrite the Iron Brawler's fiction from "slow
   and heavy" to something fast.
4. **Re-declare the band as 85..100** and state in the description that this
   encounter is designed to be won. This was offered and rejected once already,
   and it is recorded again only because it remains defensible.

---

## Change G -- Iron Brawler atk 105 -> 135

Commit `e3d4a85`. **The first content change in this document that is a
measurement rather than a prediction.**

**The decision that preceded it.** None of the four options at the end of Change
F was taken. A fifth was: measure the curve first, then choose. The four options
all required knowing how much damage buys how much win rate, and after six
changes that quantity had still never been measured -- every entry above sized
itself against an interpolation between two distant points.

**The instrument.** `balance` gained `--combatants`, `--stands` and `--skills`
(commit `303a617`), so content can be read from a path instead of the compiled-in
`data/`. `tools/probe/` holds five clones of the Iron Brawler differing only in
`atk`, and five copies of the encounter differing only in which clone appears.
One run, five points, nothing written to shipped content. The probe files cannot
reach the game, the validator or the CI gate; the gate runs `balance` with no
arguments, which can only read what `include_str!` compiled in.

**The curve**, full write-up in [`BALANCE-CURVE.md`](BALANCE-CURVE.md):

| Enemy `atk` | Foe output/battle | Ratio | Win rate |
| --- | --- | --- | --- |
| 105 (control) | 736 | 0.65 | 100% |
| **135** | **847** | **0.74** | **67%** |
| 165 | 925 | 0.81 | 67% |
| 195 | 993 | 0.87 | 42% |
| 225 | 1128 | 0.99 | 8% |

The control row reproduced the shipped Change F report in every digit, which is
what makes the other four readable.

**Why 135 and not 165**, which measures the same 67%: it sits on the near edge of
the plateau rather than the far one, so a later buff to the party pushes this
encounter towards the flat 100% region instead of over the cliff; effective
attack 157 stays under Dio's 160, and a random encounter should not out-hit the
boss; and Kakyoin survives 17% of battles instead of none, which keeps the
encounter survivable rather than merely winnable.

**No prediction is recorded for this change, because none was possible to get
wrong.** `probe.curve_a135` *is* `matchup.assassin_ambush` with this stat and the
same twelve seeds. The expectation was equality, not a range.

**Result: equality, to the digit.**

| Matchup | Before | After | Band |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | 85..100 ok |
| `matchup.assassin_ambush` | 100% | **67%** | 45..90 **ok** |
| `matchup.dio_boss` | 50% | 50% | 35..75 ok |

`matchup.assassin_ambush`, per battle, before -> after:

| Combatant | Dealt | Taken | SP | Miss | Alive |
| --- | --- | --- | --- | --- | --- |
| Jotaro | 1031 -> 985 | 238 -> **339** | 70 -> 70 | 4% -> 8% | 100% -> **67%** |
| Kakyoin | 319 -> 304 | 504 -> 514 | 86 -> 82 | 21% -> 20% | 17% -> 17% |
| Flame Assassin | 308 -> 324 | 640 -> 640 | 38 -> 41 | 9% -> 9% | 0% -> 0% |
| Iron Brawler | 428 -> **523** | 709 -> **648** | 57 -> 57 | 12% -> 12% | 0% -> **33%** |

Median length 18 -> **19** turns. `thug_solo` and `dio_boss` are bit-identical to
their previous runs, so the change reached only the encounter it was aimed at.

### What this cost and what it bought

Six changes were spent moving a ratio along the flat part of a curve nobody had
plotted. The sweep that plotted it cost one run and two files that never ship.
**Measure the response curve before sizing the first change, not after the
sixth.** That is the only transferable lesson in this document, and it was
available from the beginning.

Two things this did **not** fix, both still true and both recorded so they are
not mistaken for solved:

- **Jotaro is still the encounter.** His survival tracks the win rate exactly
  across all five sweep points: 100, 67, 67, 42, 8. The party loses when he
  falls, and nothing else in the fight decides the outcome. Change G closed the
  band; it did not make this a two-versus-two.
- **The Iron Brawler now hits at effective 157 against Dio's 160.** Mechanically
  correct, fictionally awkward: a random-encounter bruiser punching within 2% of
  the final boss. The alternative was nerfing `skill.rush_barrage`, a lever
  shared with `matchup.dio_boss`, which sits at 50% with 15 points of slack. That
  trade is deliberate and is the one to revisit first if the roster grows.

---

## Issue #8 -- `miss%` counted per action instead of per accuracy roll

Commits `2808a26` (`report.rs`), `387d02a` (`sim.rs`), `cec68f7` (CLI legend).
**A reporting fix, not a rules change: no win rate moved and every battle is
bit-identical.**

**What was wrong.** `Event::Missed` is emitted once per target, `Event::ActionUsed`
once per action. An `all_enemies` skill against two foes therefore produced two
accuracy rolls and one action, and `misses / actions` could exceed the skill's
real miss rate without bound. `CombatantStats` gained `attack_rolls`, and `miss%`
is now `misses / attack_rolls`.

**The prediction that was wrong, and it was recorded before the run.** The
prediction was that no shipped number would move, on the reasoning that "every
skill the AI can actually reach is single-target". False, and checkably so:
`skill.blade_volley` is Kakyoin's stand skill, he does not own
`concussive_slam`, and 110 power is his highest option, so he had been using an
area attack in every run recorded in this file.

| Combatant | Encounter | Released `0.3.0` | Corrected | Cause |
| --- | --- | --- | --- | --- |
| Kakyoin | `assassin_ambush` | **20%** | **11%** | Two rolls per `blade_volley` |
| Kakyoin | `dio_boss` | 9% | 9% | Same skill, rounding absorbs it |
| Everyone else | all | unchanged | unchanged | Single-target only |

**The released `0.3.0` report is wrong in that one column, and it stays on the
record** rather than being edited to match. The `rolls` column now printed beside
`miss%` is the diagnostic: whenever `rolls` exceeds `actions`, an area skill is
in use, which is exactly the signal that was missing.

---

## Issue #9 -- the AI scores utility, not only damage

Commits `1602dc5` (the offensive choice becomes an explicit score) and `38b775a`
(non-damaging effects are priced in the same unit). **A rules change. The
comparability boundary at the top of this file applies from here.**

**What was wrong.** `ai::best_offensive` ranked candidates by summed damage
power. Two facts followed from that, neither of them decided by anyone:

- a skill with no `Damage` or `Drain` effect scored zero and was filtered out, so
  `skill.guard_stance` and `skill.rage_focus` could not be chosen by any profile;
- power was read per target, so `skill.blade_volley` at 110 against two foes was
  ranked below `skill.concussive_slam` at 140 against one, and
  `skill.tempo_halt` was 60 points of damage with its entire purpose discarded.

Eight of eleven skills were reachable. Every win rate in `0.3.0` measures a game
played with eight.

**The unit.** Every effect is now priced in **damage-equivalent points**: `def_up`
is the damage it prevents (`potency / 2` per incoming hit, because the model
subtracts `def / 2`), a tempo lock is the enemy actions it denies valued at the
actor's own best hit, `bleed` is its own total, `stun` is the action it costs, and
a `chance` below 100 scales the price. A self-buff is priced over `duration - 1`,
because the turn spent casting is not a turn spent benefiting. Healing scores
zero here: the support triage branch already owns that decision and runs first.

**The design question that had to be answered first: should the AI guard instead
of attacking?** No -- it should price both and let the arithmetic decide. A rule
of the form "guard when HP is low" produces an enemy that stops trying to win,
and it cannot be predicted by a player from the data. With the shipped numbers
the arithmetic says:

| Skill | Score | Against | Verdict |
| --- | --- | --- | --- |
| `guard_stance` | 60, or **120** below 40% HP | `strike` 100 | Refused while healthy, taken while dying, still refused if `rush_barrage` is affordable |
| `rage_focus` | 40% x 2 turns = **0.8 hits** | any attack, 1.0 hits | Refused, correctly. The numbers are wrong, not the scorer |
| `tempo_halt` | 180 damage + **210** denied = 390 | `blade_volley` 330 | **Taken.** Dio finally uses his signature skill |
| `blade_volley` | 110 x 2 or 3 targets | `rush_barrage` 195 | **Taken by Dio**, who now prefers area damage to a single big hit |
| `emerald_snare` | 75 + 79 slow = **154** | `strike` 100 | Taken by Kakyoin below 24 SP |
| `concussive_slam` | 140 + 56 stun = **196** | `blood_drain` 120 | Unchanged for the Iron Brawler |
| `sun_flare` | 150 + 45 bleed = **195** | `rage_focus` 120 | Unchanged for the Flame Assassin |

**Prediction, written before the run.** In order of confidence:

1. `matchup.thug_solo` stays at 100%. The thug is a `Trickster`, which does not
   use the scorer, and Jotaro's `rush_barrage` still outscores everything he owns.
2. `matchup.dio_boss` **falls out of the 35..75 band**, most likely to 8-33%. Dio
   gains a party-wide tempo lock and an area attack he never used, against a
   party whose only new option is a guard worth 120 points.
3. `matchup.assassin_ambush` moves less, 42-67%: the Brawler's choice is
   unchanged and the Assassin's is unchanged, so only Kakyoin's snare and
   Jotaro's desperation guard differ.
4. `rage_focus` remains unused, so the milestone's "zero unreachable skills"
   criterion is **not** met by this change alone. It is met for `guard_stance`
   and `tempo_halt`; the third needs a number in `data/skills.json`, under M3
   step 5, in its own commit with its own entry.

**If prediction 2 lands, `balance_bounds` fails and that is the expected
outcome, not a regression.** The band is re-declared with evidence under issue
#12, in a separate commit, after the number is known. Widening a band in the same
commit as the change that broke it is how a harness becomes decoration.

**Result: not yet measured.** This entry is deliberately committed before the
run, so the numbers cannot be edited into agreement with the prediction.

---

## Known limitations of the harness itself

Recorded here so a number is not over-read:

- **Healing is not attributed.** `Event::Healed` carries a target and no source,
  so the report cannot show healing done. Josuke's contribution has to be
  inferred from his SP spend. Fixing this means adding an actor to the event,
  which is a code change, not a tuning change. Tracked as issue #11.
- **~~`miss%` is inflated for area skills.~~ Fixed in issue #8.** This bullet
  shipped in `0.3.0` describing the defect as a possibility -- "Kakyoin's 13-21%
  is mostly this" -- when it was already live and had already corrupted a shipped
  number. `miss%` is now `misses / attack_rolls` and the `rolls` column beside it
  makes area usage visible. The released `0.3.0` figure of 20% for Kakyoin in the
  ambush is wrong; the measurement is 11%.
- **Twelve seeds is a small sample.** The resolution is 8.3 percentage points:
  one seed flipping moves a reported win rate by that much, so 67% and 75% are
  not distinguishable results. A win rate near a band edge should not be treated
  as decisively inside or outside it. Widen the seed list in a commit of its own,
  never in the same commit as a content change. Tracked as issue #13.
- **A rules change resets the series.** RNG draw order is part of the rules, so
  after any edit to `ai.rs`, `resolve.rs` or `battle.rs` the same seed no longer
  reproduces the same battle. Cross-boundary comparison is meaningless even when
  the numbers look adjacent.
- **The harness plays worse than a player.** Auto-battle scores each affordable
  skill once and never sets up a combination across turns. An AI-vs-AI win rate
  is therefore a floor for a competent player, not an estimate of their
  experience, and a band should be read with that in mind.
- **~~Only five of eleven skills are ever used.~~ Eight were reachable before
  issue #9, and ten after it.** The `0.3.0` bullet undercounted by three:
  `skill.emerald_snare` and `skill.blade_volley` are reachable through Kakyoin,
  who owns neither of the skills that would dominate them, and
  `skill.blood_drain` through Dio and the Iron Brawler. The three genuinely
  unreachable ones were `guard_stance`, `rage_focus` and `tempo_halt`; #9 reaches
  the first and third, and `rage_focus` remains unreachable because its own
  numbers make declining it correct.
- **A "turn" in the report is one action, not one round.** Reading it as a round
  inflates every per-turn estimate by the number of combatants, and that error
  contributed directly to the failed Change E sizing.
- **The curve in [`BALANCE-CURVE.md`](BALANCE-CURVE.md) is a property of the
  current rules, not a constant.** It was measured on one encounter with one
  party, and any edit to `resolve.rs`, `ai.rs`, the roster or the skill list
  invalidates it. Re-run the sweep before sizing an encounter against it, and
  replace the table rather than appending to it. Issue #9 invalidates it now;
  the re-sweep is issue #14.
