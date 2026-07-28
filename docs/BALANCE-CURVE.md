# The win-rate curve

> Measured 2026-07-29 on `feat/balance-harness`, commit `75eba6d6`, with
> `tools/probe/`. Companion to `BALANCE-LOG.md`, which records changes; this
> file records the one measurement that changes have been guessing at.

## Why this exists

Six content changes (A through F) moved the enemy-to-party damage ratio of
`matchup.assassin_ambush` from 0.32 to 0.65 and left the win rate at 100% every
single time. Each change was sized against a threshold of "about 0.8" that
appears in the Change D entry as if it were a finding. It was not. It was a
guess, and nothing between 0.69 and 0.92 had ever been measured on any
encounter.

A sweep costs one run. Six changes cost six rounds of edit, push, pull, run,
read. The sweep should have come first.

## Method

`tools/probe/combatants.probe.json` holds the shipped roster plus five clones of
the Iron Brawler that differ in exactly one field: `atk`. Same hp, def, spd,
will, stand, skills and AI profile.

`tools/probe/matchups.probe.json` holds five copies of `matchup.assassin_ambush`
that differ in exactly one field: which clone stands beside the Flame Assassin.
Same party, same twelve Fibonacci seeds, same 300-turn limit.

```text
cargo run -q -p rpg-core --bin balance -- \
    --combatants ../tools/probe/combatants.probe.json \
    --matchups   ../tools/probe/matchups.probe.json \
    --no-fail
```

Neither file is reachable from the game, the data validator or the CI gate. The
gate is `balance` with no arguments, which can only read the content compiled
into the binary by `include_str!`.

### The control

`probe.curve_a105` is the shipped encounter with the combatant renamed. It
reported 100%, 18 turns median, 14 shortest, 23 longest, Jotaro 1031 dealt /
238 taken, Kakyoin 319 / 504 / 17% alive, Flame Assassin 308 / 640, probe 428 /
709 -- identical in every digit to the shipped run. The probe harness measures
what it claims to measure.

## The curve

Party HP for this encounter is 1140 (Jotaro 700, Kakyoin 440, stand bonuses
included). "Foe output" is Flame Assassin plus probe, damage dealt per battle.

| Probe | Enemy `atk` | Foe output/battle | Ratio | Win rate | Turns (median) |
| --- | --- | --- | --- | --- | --- |
| `a105` | 105 | 736 | 0.65 | **100%** | 18 |
| `a135` | 135 | 847 | 0.74 | **67%** | 19 |
| `a165` | 165 | 925 | 0.81 | **67%** | 17 |
| `a195` | 195 | 993 | 0.87 | **42%** | 17 |
| `a225` | 225 | 1128 | 0.99 | **8%** | 16 |

### What the shape says

1. **The bend is between 0.65 and 0.74, not at 0.8.** Thirty points of `atk` --
   a 9% change in ratio -- moves the encounter from unloseable to 8 wins in 12.
   Every change from A to F landed on the flat part of the curve, which is why
   each one looked like it did nothing. They were not too small. They were on
   the wrong side of the cliff.
2. **There is a plateau at 67% spanning ratio 0.74 to 0.81.** Two independent
   points, `a135` and `a165`, both report 8 won / 4 lost. Sitting a shipped
   encounter on a plateau is worth more than sitting it on a slope: the same
   design survives a later 10% content change without moving.
3. **The steep part is 0.81 to 0.99.** Fifty-nine percentage points of win rate
   across 18 points of ratio. Any future encounter tuned into this region will
   be fragile and should be expected to need re-measuring after every change
   that touches its participants.
4. **Fights get shorter in both directions.** 18 turns at 100%, 19 at the near
   plateau edge, then 17, 17, 16 as losses take over. Median turn count is not
   a difficulty signal on its own; it says how fast the battle resolves, not who
   wins.
5. **Kakyoin's death is not the mechanism.** Kakyoin is already at 17% survival
   at the 100% row and 0% from `a165` on. The party loses when the enemy gets
   through *Jotaro*, whose survival tracks the win rate exactly: 100, 67, 67,
   42, 8. In this encounter Jotaro's HP bar is the win condition, which is worth
   knowing before designing any encounter that includes him.

## Where the throughput model failed

Predicted probe damage per battle against measured:

| `atk` | Predicted | Measured | Error |
| --- | --- | --- | --- |
| 135 | 551 | 523 | -5% |
| 165 | 677 | 604 | -11% |
| 195 | 799 | 686 | -14% |
| 225 | 921 | 797 | -13% |

The model over-predicts, and the error grows with `atk`. Two causes, both
visible in the report:

- **SP is a hard ceiling.** `sp/b` reads 57 on every row. The probe spends the
  same SP no matter how strong it is, because `concussive_slam` (22 SP) and
  `blood_drain` (12 SP) cost what they cost out of an 80 SP pool. Above that
  budget the extra `atk` only scales `skill.strike`, so damage grows more slowly
  than `atk` does. The model assumes every action is the best skill.
- **Battles shorten.** Sixteen turns instead of 19 is three fewer actions for
  everyone, and per-battle totals fall with them.

The model remains the right tool for sizing a *change in ratio*. It should not
be trusted to convert a stat into damage above the point where a combatant runs
out of SP.

## Prediction scorecard

Pre-registered in the commit message of `75eba6d6`, before the run:

| Probe | Predicted win rate | Measured | Verdict |
| --- | --- | --- | --- |
| `a105` | 100% (control) | 100% | correct |
| `a135` | 90-100% | 67% | **wrong** |
| `a165` | 60-90% | 67% | correct |
| `a195` | 25-55% | 42% | correct |
| `a225` | 0-25% | 8% | correct |

Three of four, after seven wrong predictions in eight attempts. The improvement
is not skill; it is that a range spanning a known steep region is easy to hit.
The one miss is the informative one: it is exactly the point where the curve
bends, and it is the point the model had no data for.

## What was shipped from this

Change G, commit `e3d4a85c`: `npc.iron_brawler.stats.atk` 105 -> 135.

This is the first content change in the project that is a measurement rather
than a prediction. `probe.curve_a135` *is* `matchup.assassin_ambush` with that
stat, so the shipped encounter was expected to report the probe row exactly.

`atk 165` measures the same 67%. 135 was chosen because it sits on the near edge
of the plateau rather than the far one, so a future buff to the party pushes the
encounter towards the flat 100% region rather than off the cliff; because
effective `atk` 157 stays under Dio's 160, and a mid-tier random encounter
should not out-hit the boss; and because it leaves Kakyoin alive in 17% of
battles instead of none, which keeps the encounter survivable rather than merely
winnable.

### Confirmed

Run on the shipped content at commit `773e548f`, no arguments:

```text
Jotaro and Kakyoin vs the Flame Assassin and the Iron Brawler
12 battles: 8 won, 4 lost, 0 stalled -> 67% win rate (band 45..90)
turns: 19 median, 14 shortest, 24 longest
Jotaro          party   985 dealt   339 taken   70 sp    8% miss    67% alive
Kakyoin         party   304 dealt   514 taken   82 sp   20% miss    17% alive
Flame Assassin  foe     324 dealt   640 taken   41 sp    9% miss     0% alive
Iron Brawler    foe     523 dealt   648 taken   57 sp   12% miss    33% alive
```

Every figure is identical to `probe.curve_a135`. A probe row therefore predicts
a shipped row exactly, not approximately, and a future encounter can be tuned in
one sweep instead of one change per round trip.

`matchup.thug_solo` (100%) and `matchup.dio_boss` (50%) were bit-identical to
their previous runs, which is the other half of the result: the change reached
only the encounter it was aimed at. All three encounters are inside their bands,
so the harness became a blocking CI job in commit `45c671e6`.

## Re-running this

The curve is a property of the current rules, not a constant. Any change to
`resolve.rs`, `ai.rs`, the party roster or the skill list invalidates it. Re-run
the sweep after any of those before sizing another encounter, and replace the
table above rather than appending to it.
