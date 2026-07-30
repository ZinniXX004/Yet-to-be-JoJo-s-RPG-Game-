# The win-rate curve

> Re-measured 2026-07-30 on `feat/curve-resweep`, commit `60667aa0`, with
> `tools/probe/`, over 300 seeds per row. Supersedes the `0.3.0` sweep, which is
> kept below under *The retracted table*. Companion to `BALANCE-LOG.md`, which
> records changes; this file records the one measurement that changes have been
> guessing at.

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

`tools/probe/combatants.probe.json` holds the shipped roster plus six clones of
the Iron Brawler that differ in exactly one field: `atk`. Same hp, def, spd,
will, stand, skills, AI profile **and resistance table**.

`tools/probe/matchups.probe.json` holds six copies of `matchup.assassin_ambush`
that differ in exactly one field: which clone stands beside the Flame Assassin.
Same party, same 300 seeds (`1..=300`), same 300-turn limit.

```text
cd src
cargo run -q -p rpg-core --bin balance -- \
    --combatants ../tools/probe/combatants.probe.json \
    --matchups   ../tools/probe/matchups.probe.json \
    --no-fail
```

Neither file is reachable from the game, the data validator or the CI gate. The
gate is `balance` with no arguments, which can only read the content compiled
into the binary by `include_str!`.

### Two things this sweep had to fix before it could run

The probe roster had silently stopped being a clone.

1. **No combatant in it had a `resist` table.** Issue #10 added elemental
   resistance, and the field is `#[serde(default)]`, so a file written before it
   loads without complaint as an empty table. The clones were taking full
   physical damage from Jotaro where the shipped Iron Brawler takes 25% less, and
   ordinary psychic damage from Kakyoin where the shipped one takes 25% more.
   Every clone now carries `{ physical: 25, psychic: -25 }` verbatim.
2. **The control had moved and nobody had moved it.** Change G shipped
   `npc.iron_brawler.stats.atk` 105 -> 135, so `a105` stopped being the shipped
   creature two releases ago. The control is now `probe.curve_a135`.

Had the sweep run without these, the gap between control and shipped encounter
would have mixed six rules changes with one missing resistance table, and there
would have been no way to separate them afterwards.

### The control

`probe.curve_a135` is the shipped encounter with the combatant renamed. It
reported 300 battles, 158 won, 142 lost, 53%, 21 turns median, 13 shortest, 32
longest; Jotaro 900 dealt / 487 taken / 81 sp / 2232 rolls / 8% miss / 51%
alive; Kakyoin 344 / 497 / 78 / 2018 / 10% / 15%; Flame Assassin 344 / 611 / 43
/ 1290 / 8% / 16%; probe 631 / 632 / 6 heal / 67 / 1514 / 11% / 42%; and every
skill-use count identical, down to Kakyoin's six uses of `strike`.

That is the shipped `matchup.assassin_ambush` in every digit. The probe harness
measures what it claims to measure, and the five other rows are readable.

## The curve

Party HP is **1140** — Jotaro 620, Kakyoin 520. Neither Star Platinum nor
Hierophant Green grants `hp`, so no stand bonus applies. (The old table gave
this as "Jotaro 700, Kakyoin 440, stand bonuses included". The total was right
and the split was invented.) Foe HP is **1340** — Flame Assassin 640, probe 620
plus Iron Hymn's `hp: 80`.

"Output" is damage dealt per battle, summed across a side.

| Probe | `atk` | Foe out/b | Party out/b | Foe out / party HP | Exchange ratio | Win rate | 95% interval | Jotaro alive | Turns |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `a075` | 75 | 694 | 1337 | 0.61 | 0.61 | **96%** | 94..98 | 93% | 22 |
| `a105` | 105 | 845 | 1310 | 0.74 | 0.76 | **81%** | 77..85 | 78% | 21 |
| `a135` | 135 | 975 | 1244 | 0.86 | 0.92 | **53%** | 47..59 | 51% | 21 |
| `a165` | 165 | 1044 | 1150 | 0.92 | 1.07 | **34%** | 29..39 | 33% | 19 |
| `a195` | 195 | 1080 | 1048 | 0.95 | 1.21 | **20%** | 16..25 | 19% | 17 |
| `a225` | 225 | 1101 | 953 | 0.97 | 1.36 | **12%** | 8..16 | 12% | 16 |

`a135` is the shipped encounter. **Exchange ratio** is
`(foe out / party HP) / (party out / foe HP)` — how fast each side is being
killed, relative to each other. See *Pricing a new enemy* for why it is there.

### What the shape says

1. **It is a smooth monotone decline. There is no plateau and no cliff.**
   Win rate falls 96, 81, 53, 34, 20, 12 with no flat step anywhere, and the
   95% intervals of adjacent rows do not overlap except at the extremes. The
   steepest segment is `atk` 105 to 135, which loses 28 points, and the slope
   eases in both directions from there — the ordinary shape of any binary
   outcome, steepest where it passes 50%.

2. **The shipped encounter sits on the steepest part of the curve.** At `atk`
   135 the local slope is about 0.8 win-rate points per point of enemy `atk`.
   A ten-point stat change to either side of this encounter moves it about eight
   points of win rate, which is larger than the sampling error and comfortably
   inside the 45..90 band. It is tunable, but it is not stable: this is the
   opposite of what the old table claimed when it placed the encounter on a
   plateau, and it means `assassin_ambush` must be re-measured after any change
   touching Jotaro, Kakyoin, the Flame Assassin or the Iron Brawler.

3. **Jotaro's HP bar is the win condition, and this is now the best-supported
   finding in the file.** His survival tracks the win rate to within three
   points at every row: 93 against 96, 78 against 81, 51 against 53, 33 against
   34, 19 against 20, 12 against 12. The party essentially never wins without
   him and rarely loses with him. Kakyoin's survival is 48, 28, 15, 8, 6, 2 —
   far below the win rate everywhere, so his death is a symptom, not the
   mechanism. Any encounter designed around this party is really an encounter
   designed against one character.

4. **Fights get monotonically shorter as the enemy gets stronger**: 22, 21, 21,
   19, 17, 16. The old table claimed they got shorter *in both directions* and
   read a non-monotonicity into 18, 19, 17, 17, 16. At 300 seeds there is no
   such wobble. Median turn count is still not a difficulty signal on its own —
   it says how fast the battle resolves, not who wins — but it is a clean
   monotone function of enemy strength.

5. **Damage per turn is linear in `atk`; only damage per battle saturates.**
   Probe output per battle grows 347, 501, 631, 700, 745, 781 across a
   threefold rise in `atk` — heavily sublinear. Divided by median turn count it
   is 15.8, 23.9, 30.0, 36.8, 43.8, 48.8, and the ratio of that to `atk` is
   0.21, 0.23, 0.22, 0.22, 0.23, 0.22. **Constant to within 4% across the whole
   range.** The saturation is entirely battle-length compression: a stronger
   enemy ends the fight sooner and therefore banks fewer per-battle totals.
   (Caveat: this divides a mean by a median, so treat it as a good
   approximation rather than an identity.)

6. **The ratio axis dies above 0.9.** `Foe out / party HP` moves only 0.92 ->
   0.97 across the last three rows while the win rate falls 34% -> 12%. It is
   partly an *outcome* of the fight rather than an input to it, because per-
   battle output is depressed by the short battles that a strong enemy causes.
   The exchange ratio does not have this problem: it moves 1.07 -> 1.36 over the
   same rows, and it crosses 1.0 between `a135` and `a165`, exactly where the
   win rate crosses 50%.

## The retracted table

Measured 2026-07-29 at commit `75eba6d6`, twelve Fibonacci seeds, `0.3.0` rules,
no resistance in the engine. Kept because a retracted measurement is still
evidence, and because this project has already retracted one curve claim (the
invented "bend at 0.8", actually 0.65-0.74).

| Probe | Enemy `atk` | Foe output/battle | Ratio | Win rate | Turns (median) |
| --- | --- | --- | --- | --- | --- |
| ~~`a105`~~ | ~~105~~ | ~~736~~ | ~~0.65~~ | ~~**100%**~~ | ~~18~~ |
| ~~`a135`~~ | ~~135~~ | ~~847~~ | ~~0.74~~ | ~~**67%**~~ | ~~19~~ |
| ~~`a165`~~ | ~~165~~ | ~~925~~ | ~~0.81~~ | ~~**67%**~~ | ~~17~~ |
| ~~`a195`~~ | ~~195~~ | ~~993~~ | ~~0.87~~ | ~~**42%**~~ | ~~17~~ |
| ~~`a225`~~ | ~~225~~ | ~~1128~~ | ~~0.99~~ | ~~**8%**~~ | ~~16~~ |

### The comparison issue #14 asked for cannot be made

Issue #14 says the gap between the control and the shipped encounter "**is** the
measurement of what the rules changes did". It is not, and the reason is
instructive.

A twelve-seed reading carries roughly +/-14 points of standard error. Put 95%
intervals on the old rows and compare them to the new ones:

| `atk` | Old | Old 95% interval | New | Contradicted? |
| --- | --- | --- | --- | --- |
| 105 | 100% | 76..100 | 81% | no |
| 135 | 67% | 39..94 | 53% | no |
| 165 | 67% | 39..94 | 34% | **yes** |
| 195 | 42% | 15..72 | 20% | no |
| 225 | 8% | 0..36 | 12% | no |

**Four of the five old points cannot disagree with anything.** Their intervals
are so wide that the new values fall inside them, so the differences are not
evidence that the rules changed the curve — they are equally consistent with the
old instrument having been unable to see. Differencing the two tables would
produce five numbers of which four are noise, and there would be no way to know
which four.

So the old table is **retracted, not differenced**. What the six rules changes
did to this curve is not recoverable from the record, and the honest response is
to say so rather than publish a delta the data does not support. The lesson is
the same one issue #12 arrived at from the other direction: a measurement taken
with an instrument too coarse is not a cheap version of the real thing, it is an
absence of information that looks like a number.

### The one thing that is genuinely overturned

> **There is a plateau at 67% spanning ratio 0.74 to 0.81.** Two independent
> points, `a135` and `a165`, both report 8 won / 4 lost. Sitting a shipped
> encounter on a plateau is worth more than sitting it on a slope.

Retracted. At 300 seeds those two points read **53%** and **34%**, nineteen
points apart with about 3 points of error each. "Two independent points agree"
was two draws of 8/12 landing on the same value, which is the single most
likely coincidence available to a twelve-seed instrument.

This matters beyond the table. Change G chose `atk` 135 over 165 partly *because*
they were believed to measure the same win rate, so 135 could be taken as the
near edge of a flat region that would absorb a later party buff. There is no
flat region. The choice of 135 remains correct for its other two reasons —
effective `atk` stays under Dio's, and it leaves Kakyoin alive some of the time —
but the plateau argument behind it is void.

### What survives

- **Jotaro is the win condition.** Stated from five points, now confirmed on six
  at fifty times the precision, and tighter than before.
- **Kakyoin's death is not the mechanism.** Confirmed.
- **The probe harness reproduces shipped content exactly.** Confirmed again, on
  a different row, after a resistance system was added to the engine.

## Where the throughput model failed

The model's predictions were made under `0.3.0`; the measurements below are new.
The comparison is therefore about the model's *shape*, not a controlled test.

| `atk` | Predicted | Measured (300 seeds) | Error |
| --- | --- | --- | --- |
| 135 | 551 | 631 | **+15%** |
| 165 | 677 | 700 | +3% |
| 195 | 799 | 745 | -7% |
| 225 | 921 | 781 | **-15%** |

The old file recorded a uniform over-prediction of 5-14% and blamed a hard SP
ceiling, noting `sp/b` read 57 on every row. That is no longer the failure mode.
`sp/b` now reads 64, 66, 67, 64, 61, 56 — it is not pinned, because SP regen
arrived after that sweep. The error is now a **slope error**: the model is 15%
low at the bottom of the range and 15% high at the top, crossing zero near
`atk` 175.

Finding 5 above explains it. Damage per *turn* is very nearly proportional to
`atk`; damage per *battle* is not, because battle length falls from 22 turns to
16 across the range. A model that converts a stat into per-battle damage without
modelling battle length must get the slope wrong, and will do so in exactly this
direction.

**Replacement rule of thumb, for this skill kit:** per-turn output is about
`0.22 x atk`. Multiply by the expected battle length rather than fitting
per-battle damage directly.

## An unexplained trend

Kakyoin abandons `emerald_snare` as the enemy gets stronger, and does it very
smoothly:

| Probe | `emerald_splash` | `emerald_snare` | Snare uses per battle | Kakyoin turns per battle |
| --- | --- | --- | --- | --- |
| `a075` | 64% | 35% | 1.75 | 5.00 |
| `a105` | 67% | 32% | 1.45 | 4.49 |
| `a135` | 72% | 28% | 1.09 | 3.92 |
| `a165` | 78% | 21% | 0.74 | 3.47 |
| `a195` | 81% | 19% | 0.60 | 3.19 |
| `a225` | 86% | 14% | 0.40 | 2.83 |

Six points, monotone, and far too regular to be sampling noise. It is not merely
an artefact of shorter battles: Kakyoin's turns per battle fall by a factor of
1.8 across the range while his snare uses fall by a factor of 4.4, so he really
is choosing it less often per turn.

The probe's `spd` is 62 on every row and only `atk` varies, so nothing about the
enemy's speed — the thing `spd_down` is supposed to counter — has changed. The
mechanism is in `ai.rs` and this file will not guess at it. Recorded here as an
observation to be explained, not as a finding.

## Pricing a new enemy

For an encounter shaped like this one — two foes, this party, no healer:

1. Estimate each foe's per-turn output as `0.22 x atk`, adjusted for its skill
   kit, then multiply by the party's HP-weighted expected battle length.
2. Compute the **exchange ratio**, not the one-sided ratio. Use
   `(foe out / party HP) / (party out / foe HP)`.
3. Read across: 0.61 -> 96%, 0.76 -> 81%, 0.92 -> 53%, 1.07 -> 34%,
   1.21 -> 20%, 1.36 -> 12%. Parity (1.0) lands near 45%.
4. Expect to be wrong, and measure. Every number above is a property of the
   current rules and the current party.

## Prediction scorecard

Pre-registered in the commit message of `60667aa0`, before the run:

| Probe | Predicted | Measured | Verdict |
| --- | --- | --- | --- |
| `a075` | 95-100% | 96% | correct |
| `a105` | 72-85% | 81% | correct |
| `a135` | exactly 53%, every digit | 53%, every digit | correct (a determinism check, not a forecast) |
| `a165` | 30-42% | 34% | correct |
| `a195` | 15-27% | 20% | correct |
| `a225` | 3-12% | 12% | correct, on the boundary |
| shape | no 67% plateau | monotone throughout | correct |
| shape | bend moves to lower `atk` | steepest segment is still 105-135 | **wrong** |
| control | matches bit for bit | it did | correct |

Eight of nine, the best run this project has recorded. Three caveats keep it
honest: the control row is arithmetic rather than a forecast, the ranges were
10-13 points wide against an instrument with 3 points of error, and `a225`
landed on the edge of its interval.

The one miss is the interesting one. The curve did shift **down** — 53% where
67% was published at the shipped stat — but it did not shift **sideways**. The
steepest region is where it was. Enemy strength moved the whole response down
rather than relocating the point of maximum sensitivity, which is not what
"the bend moves" would have implied.

## Re-running this

The curve is a property of the current rules, not a constant. Any change to
`resolve.rs`, `ai.rs`, the party roster or the skill list invalidates it. Re-run
the sweep after any of those before sizing another encounter, and replace the
table above rather than appending to it.

**Before re-running, diff `tools/probe/combatants.probe.json` against
`data/combatants.json`.** The probe roster is a hand-maintained copy and it has
silently drifted once already, through a schema field that defaults to empty
when absent. A control row that does not reproduce the shipped encounter in
every digit means the sweep is void, and it is the first thing to check in the
output.
