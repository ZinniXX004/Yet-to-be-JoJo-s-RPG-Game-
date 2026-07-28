# Balance log

Every balance change to `data/*.json`, the measurement that motivated it, and the
measurement that followed it. One entry per change.

The rule this file exists to enforce: **change one number, re-run, record.** Two
numbers at once and the result is uninterpretable, because either change could
have produced it. This is slower and it is the only version that produces
knowledge rather than opinion.

Predictions are written before the run, not after. A prediction recorded after
the fact is a rationalisation, and two of them are already wrong below.

All numbers come from:

```powershell
cd src
cargo run -q -p rpg-core --bin balance
```

over the fixed seed list in [`data/matchups.json`](../data/matchups.json).
Seeds are fixed precisely so two rows of this table can be compared.

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

**Result:** pending measurement.

---

## Known limitations of the harness itself

Recorded here so a number is not over-read:

- **Healing is not attributed.** `Event::Healed` carries a target and no source,
  so the report cannot show healing done. Josuke's contribution has to be
  inferred from his SP spend. Fixing this means adding an actor to the event,
  which is a code change, not a tuning change.
- **`miss%` is inflated for area skills.** A miss is counted per target while an
  action is counted once, so a combatant using an `all_enemies` skill can show a
  miss rate above its true accuracy. Kakyoin's 13-20% is mostly this.
- **Twelve seeds is a small sample.** A win rate near a band edge should not be
  treated as decisively inside or outside it. Widen the seed list in a commit of
  its own, never in the same commit as a content change.
