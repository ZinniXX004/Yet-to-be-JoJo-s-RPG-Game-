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

> **Comparability boundary.** Change C alters `ai::choose`, which changes both
> the decisions taken and the order in which the RNG is drawn. Every number
> recorded above that entry is historical. Do not compare it to anything below.

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

**Prediction, written before the run:**

| Matchup | Now | Predicted | Reasoning |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 100% | one enemy, one ally; targeting cannot differ |
| `matchup.assassin_ambush` | 100% | **80-95%** | the party can no longer delete the 300 HP thug first every time, so enemy output decays later; may still exceed 90 |
| `matchup.dio_boss` | 83% | **70-88%** | two effects fight each other: Kakyoin survives more, which *helps* the party, while damage spread across three party members is less efficient for the enemy. Direction genuinely uncertain, and it may rise |

The boss prediction is deliberately allowed to move upward. If it does, that is
information, not a failure: it would mean the party was previously being *saved*
by having a designated victim.

**Secondary expectations:** Kakyoin's survival rises off 0%; Jotaro's and
Josuke's damage taken rises; Flame Assassin's lifetime lengthens in the boss
fight without any further content change; `median_turns` rises in every
multi-combatant matchup.

**Result:** pending measurement.

**Consequence for this document:** the baseline, Change A and Change B are now
historical. The run after this commit is the new reference point, and the bands
in `data/matchups.json` must be judged against it before any further content
number is touched.

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
- **A rules change resets the series.** RNG draw order is part of the rules, so
  after any edit to `ai.rs`, `resolve.rs` or `battle.rs` the same seed no longer
  reproduces the same battle. Cross-boundary comparison is meaningless even when
  the numbers look adjacent.
