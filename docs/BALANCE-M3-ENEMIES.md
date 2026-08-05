# Issue #16 -- three enemies, three encounters, ten readings

**This is the issue #16 entry of [`BALANCE-LOG.md`](BALANCE-LOG.md).** It lives in
its own file, and the reason is itself a recorded failure. The entry was first
written as part of a single full-file rewrite of `BALANCE-LOG.md`, which is 81 KB.
The write path available for these documents can only send a **whole file**, with
no patch and no append, and the content sent was cut off mid-sentence inside the
issue #15 entry. The commit succeeded, the file was silently truncated, and
everything after that point -- four findings of #15 and the whole "Known
limitations of the harness itself" section -- vanished from the branch until
`1255f35` restored it byte-for-byte from `f341618`.

That was the **second** occurrence of that exact failure on that exact file, and
the second happened immediately after warning about the first. So the rule is not
"be more careful": **a document that can only be written whole must be kept small
enough to write whole.** This file is the first application of it.

The annotations #16 makes to earlier entries are in `BALANCE-LOG.md`, beside the
claims they amend, and are not repeated here: HP as an uptime knob on Change A,
`FOCUS_FIRE_CHANCE` against support enemies on Change C, the sensitivity
correction on Change D, Jotaro's survival tracking on Change F, and the
amendments to issues #8, #9, #10, #11, #12, #14 and #15.

---

## What shipped

Nine additions to `data/`, nothing in `src/`. Three enemies, each varying **one**
axis of enemy design, each with its own encounter, and the Iron Brawler held
constant as the second foe in all three so the encounters are comparable to each
other and to `matchup.assassin_ambush`.

| Enemy | Axis | `hp` / `sp` / `atk` / `def` / `spd` / `will` | Signature skill |
| --- | --- | --- | --- |
| Blade Dancer | speed and cheap actions | 620 / 60 / 92 / 38 / **110** / 50 | `flurry_cut` -- 10 SP, power 105, `tempo_cost` **700** |
| Hex Weaver | pure mitigation | **690** / 90 / 60 / 55 / 78 / 85 | `wither_hex` -- 18 SP, `atk_down` 26 / 8 turns / 85%, **no damage effect** |
| Requiem Bell | one enormous slow action | 560 / 90 / 96 / 60 / **40** / 70 | `grave_toll` -- 40 SP, power **240** psychic, `tempo_cost` **2000** |

None of the three carries a resistance table, deliberately: three encounters that
each vary one axis are only readable if the damage pipeline is held flat across
them. `wither_hex` is the first skill in the game with no `damage` effect;
`grave_toll` holds the largest `power` and the largest `tempo_cost` in the file;
the Requiem Bell has the lowest `spd`.

Bands, all at 300 seeds: `dancer_rush` **60..90**, `weaver_gambit` **58..86**,
`bell_race` **48..80**.

## The ten readings, in order

| # | Commit | The one thing that changed | Encounter | Win rate | Gate |
| --- | --- | --- | --- | --- | --- |
| 1 | `3f5551ae` | Blade Dancer ships, foes = Dancer + Flame Assassin, hp 340 | `dancer_rush` | **100%** | RED |
| 2 | `02bea2d5` | second foe becomes the Iron Brawler | `dancer_rush` | **95%** | RED |
| 3 | `d84a3894` | Dancer hp 340 -> **620** | `dancer_rush` | **70%** | green |
| 4 | `3381e0e1` | Hex Weaver ships, hp 480 | `weaver_gambit` | **92%** | RED |
| 5 | `8edcf653` | Weaver hp 480 -> **560** | `weaver_gambit` | **88%** | RED |
| 6 | `3b4ca816` | Weaver hp 560 -> **690** | `weaver_gambit` | **77%** | green |
| 7 | `618dad83` | `wither_hex` duration 8 -> **3** | `weaver_gambit` | **87%** | RED |
| 8 | `14c7b14a` | duration **3 -> 8** again | `weaver_gambit` | **77%** | green |
| 9 | `769ffb16` | Requiem Bell ships, hp 560 | `bell_race` | **68%** | green |
| 10 | `f341618d` | two `description` strings only | -- | unchanged | green |

Reading 8 reproduced reading 6 **cell for cell**, and reading 10 reproduced
reading 9 cell for cell across all six encounters. Together with `14c7b14a` that
is three separate proofs that `description` strings never reach the simulation --
worth having, because two of them were written to be exactly that.

**The first three encounters in the report never moved a digit at any of the ten
commits.** That is what makes "#16 creates no comparability boundary" a measured
claim, and it is the strongest form of that claim in the project: verified ten
times rather than once.

## Scorecard for the milestone

| Commit | Predictions | Wrong |
| --- | --- | --- |
| `3f5551ae` | 5 | 2 |
| `02bea2d5` | 4 | 3 |
| `d84a3894` | 5 | 2 |
| `3381e0e1` | 6 | 3 |
| `8edcf653` | 6 | 4 |
| `3b4ca816` | 7 | 3 |
| `618dad83` | 8 | 5 |
| `14c7b14a` | 5 | **0** |
| `769ffb16` | 10 | 3 |
| **Total** | **56** | **25** |

45% wrong, against 52% over the project's whole history. The running record moves
from sixty-one predictions with thirty-two wrong to **one hundred and seventeen
with fifty-seven wrong**. The only clean sheet, `14c7b14a`, is also the least
interesting run: it predicted that reverting one field would reproduce an earlier
reading exactly, and it did.

---

## The Blade Dancer -- what "fast and fragile" cost

**The brief was retreated from, and the retreat is the finding.** The Dancer was
conceived as fast and fragile. Fragility is `def` 38, the lowest of any foe, and
that part shipped. Low **HP** did not: at hp 340 the encounter read 100% and then
95%, and hp had to go to **620** -- equal to Jotaro's -- before the fight was a
fight. At this party's output a foe with a small pool does not get to act, so
"fragile" is not expressible in HP in this engine. It is expressible in defence
only.

Reading 1 also wrongly sized the encounter for a second reason: the Flame Assassin as
the second foe. Swapping it for the Iron Brawler at reading 2 moved the exchange
ratio 0.427 -> 0.594 for five points of win rate, which says the fixture was
nowhere near parity and the Dancer was never the binding constraint.

**Authoritative reading, `d84a3894` and reproduced twice since** -- 209 won, 91
lost, 70%, turns 22 / 13 / 33:

| Combatant | dealt/b | taken/b | heal/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 911 | 409 | 0 | 79 | 2208 | 8% | 67% |
| Kakyoin | 357 | 483 | 0 | 80 | 2020 | 10% | 22% |
| Blade Dancer | 272 | 607 | 0 | 33 | 1508 | 6% | 7% |
| Iron Brawler | 621 | 661 | 8 | 65 | 1461 | 10% | 28% |

The Dancer takes **5.03 actions per battle**, the most of any foe in the file, and
converts them into 272 damage -- 54 per action against the Brawler's 133. The axis
worked: cheap fast actions do buy volume, and volume at `power` 105 against
defence 66-72 is not the same thing as damage.

---

## The Hex Weaver -- five readings, and the one that fired its own falsifier

### HP on a debuffer is an uptime knob

| `hp` | Weaver actions/battle | `wither_hex` uses | Win rate |
| --- | --- | --- | --- |
| 480 | **2.70** | 295 | 92% |
| 560 | **3.42** | 657 | 88% |
| 690 | **4.64** | 869 | 77% |

A pure debuffer's contribution *is* its uptime, so HP moves the encounter
linearly in a way it never does on a damage dealer. It needed the largest foe pool
outside Dio to survive long enough to cast, because `FOCUS_FIRE_CHANCE` makes a
non-damaging foe the lowest-HP foe and therefore the target.

### The single-variable duration result, and why the commit before it concluded the opposite

Duration was tested twice. At hp 560 it looked inert. At hp 690, with hp held
fixed and **only** `duration` changed:

| `duration` | Win rate | Jotaro damage/action | `wither_hex` share | Brawler `dealt/b` | Turns |
| --- | --- | --- | --- | --- | --- |
| **8** | **77%** | 102.0 | 62% | 763 | 26 |
| **3** | **87%** | **110.9** | **55%** | 713 | 24 |

Ten points from one field. The earlier null result was taken at an HP value where
the caster lived 3.42 actions, so no value of duration could have shown an
effect. **A null result only counts if the experiment had room to show a positive
one** -- and the second test was the same field, changed by the same amount, at a
place where it could speak.

The mechanism is `tick_statuses`, read from `battle.rs` rather than assumed: it
decrements only the **bearer's own** statuses, on the bearer's own turn. So the
coverage ceiling of an `atk_down` is `(bearer turns - 1) / bearer turns`, about
**89%**. Duration 8 reaches it; duration 3 measured about **72%**. A 17-point
coverage gap on a 31% per-hit cut is worth ten points of win rate.

### The reading that fired a declared falsifier

`618dad83` predicted 79..83% with a pre-committed rule: *a reading outside 71..83%
means duration was never saturated*. It measured **87%**. The falsifier fired, it
was honoured, and duration went back to 8 -- which is why reading 8 exists at all.
A falsifier written before the run is the only kind that can be honoured after it.

### The share moved seven points, through a branch that was called inadmissible

`status_points` prices `AtkDown` at `baseline * potency / 100 * duration`. At
duration 8 that is 208 points, `effect_points` = `208 * 2 * 85/100` = **354**, and
with one party member left 177 -- still above the 100-point baseline, so the skill
is chosen anyway. At duration 3 it is 78 -> 133, and with one member left
**66 < 100**, so the branch fires and the Weaver switches to `strike`. Measured
share 62% -> **55%**. The threshold crossing was identified from `ai.rs` before
the run and then dismissed as too rare to matter; it was worth seven points.

### Two levers rejected with arithmetic instead of with a run

- **`potency` 40 instead of 26.** `scaled()` floors the multiplier at 10%, and
  defence is subtracted **after** power scaling, so an `atk_down` bites
  super-linearly against a high-defence target: at potency 45 the cut reaches 77%,
  which is a stun with extra steps. Rejected.
- **`spd_down` instead of `atk_down`.** `tick()` adds `spd` to tempo each tick, so
  a speed debuff reduces accrual linearly -- and against the Iron Brawler it moved
  its actions per battle from 4.98 to 4.94. A wash. Rejected.

### Authoritative reading, `3b4ca816` / `14c7b14a` -- 232 won, 68 lost, 77%, turns 26 / 16 / 48

| Combatant | dealt/b | taken/b | heal/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 950 | 351 | 0 | 94 | 2792 | 8% | 76% |
| Kakyoin | 418 | 452 | 0 | 112 | 2914 | 11% | 35% |
| Hex Weaver | **40** | 688 | 0 | 52 | **524** | 7% | 1% |
| Iron Brawler | 763 | 680 | 20 | 74 | 1792 | 11% | 22% |

`rolls` 524 against 524 uses of `strike` and 869 uses of `wither_hex`: **a
status-only skill draws no accuracy roll at all**, measured four times across the
five readings at 295/295, 370/370, 524/524 and 594/594. That is a property of
skills with no damage effect, not of area skills.

---

## The Requiem Bell -- the arithmetic was right and the conclusion was wrong

`grave_toll` was shipped with a description claiming the first toll is
"unpreventable by design rather than by luck", because at `spd` 40 against
`TEMPO_THRESHOLD` 1000 the Bell's first action lands near tick 25 and the party
cannot remove 560 HP by then. The tempo arithmetic was correct. The conclusion was
not: the `aggressive` profile rolls **`AGGRESSIVE_SKILL_CHANCE = 65` before it
scores anything**, so tempo guarantees the Bell an *action*, never *which* action.
Measured: 1.24 actions per battle x 62% = **0.77 tolls per battle**, and **38% of
battles never hear the bell at all**. The claim was corrected in `f341618d`.

**This is the third time in this project that content was reasoned about with the
scorer left out of the picture**, after `riposte` at power 85 and `flurry_cut`'s
`tempo_cost`. The rule, named here: **tempo, damage and duration are properties of
the engine; whether a skill is used at all is a property of `ai.rs`, and the two
must be read together.**

### The rolls column decomposes, and it counts something no other column can

Predicted `rolls` from 372 actions; measured **578**. The reason was already in
these notes: `sim.rs::an_accuracy_roll_is_counted_for_every_target_of_every_attack`
means a damaging **area** attack draws one roll per target, so the arithmetic is
`232 x 2 + 140 = 604`. The measured 578 is therefore not an error -- it decomposes
as **206 tolls against two party members and 26 against one**, so the party was
already down to a single survivor for **11.2%** of the tolls. `miss%` 1% checks
out: 140 strikes at 95% is about 7 misses of 578.

A prediction was made against a fact already written down elsewhere in these
notes. Reading the engine does not help if the notes are not read either.

### Median `turns` is an action count, not a duration

Predicted 20..24; measured **17**, *below* `assassin_ambush`'s 21 and
`dancer_rush`'s 22. `bell_race` is not a shorter fight. `turns` counts actions,
and a foe at action rate 0.0242 contributes almost none: 11.47 party actions at a
combined rate of 0.169 still span **~68 ticks**. Summing the four combatants gives
7.23 + 4.24 + 1.24 + 4.69 = **17.4**, which is the median. The column is an
accounting identity, not a clock, and it is **not comparable across fixtures whose
speeds differ**.

### The focus-fire discount runs the wrong way against a foe that stays lowest

Both models were written down before the run. Undiscounted party output of 95.6
per action predicted the Bell's death near tick 48; the 0.6-0.65 discount taken
from `FOCUS_FIRE_CHANCE` gave ~64 per action and tick 52. The undiscounted figure
won. **When the low-HP foe stays the low-HP foe, every re-selection finds the same
target and the redirection compounds instead of averaging out**, so the discount is
too pessimistic in exactly the case it gets used for.

### The deliberate inversion held

The Bell was designed to hit hard enough to draw fire on purpose, inverting the
support-enemy problem the Hex Weaver has. Its `alive%` reads **7%**: the party did
aim at it. `sp/b` 30 is 0.77 tolls x 40 SP exactly.

### Authoritative reading, `769ffb16` and `f341618d` -- 205 won, 95 lost, 68%, turns 17 / 12 / 25

| Combatant | dealt/b | taken/b | heal/b | sp/b | rolls | miss% | alive% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jotaro | 850 | 420 | 0 | 79 | 2169 | 9% | 67% |
| Kakyoin | 343 | 471 | 0 | 82 | 2092 | 11% | 29% |
| Requiem Bell | 301 | 552 | 0 | **30** | **578** | **1%** | 7% |
| Iron Brawler | 590 | 642 | 5 | 63 | 1406 | 11% | 30% |

Actions: Jotaro `rush_barrage 1324 (61%), strike 845 (39%)`; Kakyoin
`emerald_splash 821 (65%), emerald_snare 448 (35%), strike 2 (0%)`; Requiem Bell
`grave_toll 232 (62%), strike 140 (38%)`; Iron Brawler `concussive_slam 850 (60%),
strike 529 (38%), blood_drain 27 (2%)`.

**The only encounter in the project to land inside its band on the first
reading**, and it did so on a model that had been corrected nine times first.

---

## The measurement tools this milestone fixed

### The foe-action model

The throughput model in `BALANCE-LOG.md` estimates a foe's output from speed
shares of an estimated battle length. Inverted around the quantity that is
actually stable -- party damage per party action, measured between 87 and 110 in
every reading here:

```
foe actions/battle ~ (foe HP / party damage per party action)
                   x (foe action rate / party combined action rate)

action rate = spd / mean tempo cost, weighted by the measured skill mix
```

Party combined rate **0.169**. Measured rates: Jotaro 0.091, Kakyoin 0.078, Blade
Dancer **0.137**, Flame Assassin 0.088, Hex Weaver 0.0833, Iron Brawler 0.058,
Requiem Bell **0.0242**. The Bell is 5.7 times slower than the Dancer from **two**
fields rather than one, and this is the only form of the model that predicted its
1.24 actions per battle in advance. Back-check: the Iron Brawler runs about 13%
above the model across six fixtures, consistently.

### The exchange ratio, and where it fails

```
Ratio = (foe out/b / party HP pool) / (party out/b / foe HP pool)
```

Not foe output divided by party output -- which is how this file's own axis was
misread for **three consecutive commits**, producing three wrongly sized predictions
before anyone recomputed it. Calibrated to about two points between 0.43 and 0.92,
it **over-calls by 3-4 points below 0.61 and by a full 8 points at 0.716**, and it
is **near-degenerate in any fixture the party usually wins**, because party `out/b`
is then pinned to the foe HP pool by definition: 1179 against 1180, 1256 against
1260, 1368 against 1390. The denominator carries almost no information there.

### Win-rate sensitivity was three times too small

The Weaver's duration test measured it cleanly: party damage per party action
89.8 -> 96.0, a rise of **6.9%**, moved the win rate **ten points**. That is
**~1.45 points of win rate per 1% of party output**, against the ~0.4 used in six
consecutive predictions here. Two consequences: most "wrong direction" misses in
the log were the right direction with the wrong gain, and **the minimum defensible
band width is about 25 points**, because a 1% error in a foe's effective output is
inside what any of these models can resolve. `weaver_gambit`'s 28 points is the
narrowest band in the file and close to that floor.

### Party `out/b` cannot show a debuff, and the retraction that followed

When the party wins most battles its `out/b` is pinned to the foe HP pool, so the
column is blind to mitigation. "The party is never debuffed" was asserted from it
and is **retracted**. Damage per **action** can see it:

| Fixture | Jotaro per action | Kakyoin per action |
| --- | --- | --- |
| `assassin_ambush` (no hex) | 121.0 | 87.8 |
| `dancer_rush` (no hex) | 123.8 | 84.4 |
| hex duration 3, hp 560 | 106.0 | **65.0** |
| hex duration 8, hp 560 | 100.7 | **66.5** |
| hex duration 8, hp 690 | 102.0 | 70.6 |
| hex duration 3, hp 690 | 110.9 | 73.6 |

Per-hit arithmetic, for the record: `scaled()` gives `100 - 26 = 74`, so Jotaro's
atk 120 becomes 88, and `rush_barrage` at power 195 against defence 66 goes
**201 clean to 138 debuffed -- a 31% cut**.

### And a regularity that turned out not to be one

`taken/b` equalling a foe's max HP held for several readings and broke at 688 of
689. It was a consequence of nothing surviving, not a law. Likewise the Iron
Brawler's `dealt/b` takes **eight different values** on identical stats across
these fixtures -- 531, 590, 621, 631, 656, 713, 720, 763 -- so a foe's output is
not the sum of independent contributions and cannot be carried between fixtures.

---

## Six process failures, in the order they happened

1. **A rule was written and then not applied three commits later.** "Price a foe's
   actions before pricing its skills" was written for the Dancer; the Weaver was
   then sized at "about 7 actions" and took **2.70**.
2. **A field was changed by 167% without reading the code that consumes it.**
   Corrected into a standing rule: read the code that reads a field before tuning
   it.
3. **A headline prediction landed through cancelling errors.** 77% was predicted
   and measured, on damage per action of 89.8 against ~82 predicted, Brawler 763
   against >800, and party actions 15.23 against 17.0. A right answer from wrong
   reasoning is a failure that scores as a success.
4. **A lever was tested where it could not show an effect**, then declared inert.
5. **A prediction was made against a fact already in these notes** -- the
   per-target rolls rule, which is pinned by a test in `sim.rs`.
6. **An 81 KB document was rewritten in one output and truncated**, for the second
   time, immediately after warning about the first. See the top of this file.

Also recorded: a systematic bias was named on two data points and reversed on the
third. Two data points are not a bias.

---

## Limitations to file as issues

All of these are design limits or harness gaps found here, not defects introduced
here. None is fixed in this milestone, and none should be fixed in a commit that
also changes content.

1. **`AGGRESSIVE_SKILL_CHANCE` is a fixed 65 with no per-skill override.** Content
   built around one signature action gets it about two turns in three; the Bell
   gets it in 62% of 1.24 actions, so 38% of battles never hear a toll.
2. **No column can attribute a mitigation**, and `inert_combatants` is satisfied by
   a token attack -- the Hex Weaver had to carry `skill.strike`, the one thing it is
   not for, to avoid failing `no_combatant_sits_out_the_whole_batch`.
3. **`FOCUS_FIRE_CHANCE` makes any non-damaging enemy self-defeating** at every HP
   value: the more valuable the debuff, the less of it the party ever sees.
4. **Party `out/b` cannot show a debuff** in a fixture the party wins. Report damage
   per action.
5. **Median `turns` is an action count**, not comparable across fixtures with
   differing speeds. `bell_race` reads 17 while spanning ~68 ticks.
6. **`score_action` has no term for `tempo_cost`.** The Requiem Bell is designed
   entirely around a property its own AI cannot see. Mirror image of #33.
7. **The mirror guard's coverage moved from five of eight to five of eleven** with
   no output anywhere, because none of the three new enemies appears in a probe
   fixture.
8. **The gate reports 68% and 4% with identical confidence.** No interval is
   printed beside a win rate.
9. **Six shipped matchups plus seven probe rows is ~78 KB of hand-written
   contiguous integers.** `seed_count` / `seed_base` would express it in two
   fields; overlaps issue #31.
10. **The Blade Dancer's "punishes slow parties" brief is untested** -- there is no
    slow-party fixture to test it against.

---

## What this entry does not authorise

- **No band is widened here.** Every band was declared before its encounter was
  measured, and the two encounters that failed were fixed in content -- `hp` on
  the Dancer, `hp` and `duration` on the Weaver -- never by moving a band.
- **The curve is not void.** All six rows of
  [`BALANCE-CURVE.md`](BALANCE-CURVE.md) reproduce exactly. The #14 entry listed
  #16 as likely to invalidate it and it did not: **what voids a curve is touching
  its participants, not adding content beside it.** `BALANCE-CURVE.md` does still
  need the ratio-axis definition spelled out, the near-degeneracy noted, and the
  ~1.45-point sensitivity figure added.
- **No existing encounter was retuned.** `assassin_ambush` sits on the steepest
  part of the measured curve and is the control for all three new fixtures; it
  must not be touched while they are the newest content in the game.
- **Nothing here settles issue #27.** Three new foes spanning 22 points of defence
  moved the snare's share of Kakyoin's turns 40% / 32% / 35% -- non-monotonically.
  A single foe's defence is not the mechanism.
