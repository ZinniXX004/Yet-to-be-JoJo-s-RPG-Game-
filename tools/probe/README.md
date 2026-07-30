# `tools/probe/`

**This directory is not game content.** The two JSON files here have the same
shape as `data/combatants.json` and `data/matchups.json`, but nothing loads them
unless a path is pointed at them by hand: the game reads `data/`,
`src/data-pipeline/validate_data.py` validates `data/`, and the blocking CI job
runs `balance` with no arguments, which can only read the content compiled into
the binary by `include_str!`. These files exist to answer measurement questions
that would otherwise need one throwaway commit per data point -- the combatants
are deliberately absurd (`atk` up to 225, roughly half again as hard-hitting as
the final boss) and the bands are `0..100` so a probe can never report a
violation. Numbers measured here are only meaningful once they are re-measured
on shipped content; the curve they produced is written up in
[`docs/BALANCE-CURVE.md`](../../docs/BALANCE-CURVE.md).

```text
cd src
cargo run -q -p rpg-core --bin balance -- \
    --combatants ../tools/probe/combatants.probe.json \
    --matchups   ../tools/probe/matchups.probe.json \
    --no-fail
```

The curve is a property of the current rules, not a constant. Any change to
`resolve.rs`, `ai.rs`, the party roster or the skill list invalidates it, so
re-run the sweep before sizing another encounter against it.

## Read this before running a sweep

Everything below was learned the hard way in issue #14, where this roster had
been quietly wrong for two milestones and nothing in the project noticed.

### These files have no validation and no test. None.

`validate_data.py` reads `data/` and only `data/`. The CI gate cannot see this
directory. Nothing here is type-checked beyond what `serde` will accept, and
`serde` accepts a great deal:

- **`resist` is `#[serde(default)]`.** A roster file written before issue #10
  loads without error, without warning, with every resistance table empty. That
  is exactly what happened here. Six Iron Brawler clones were taking full
  physical damage from Jotaro where the shipped Brawler takes 25% less, and
  ordinary psychic damage from Kakyoin where the shipped one takes 25% more.
- The same applies to any future field added with a default. The property that
  makes save-data migration painless makes drift here invisible.

### The probe roster is a hand-maintained copy, so it decays

Every entry in `combatants.probe.json` that is meant to mirror shipped content
is a copy taken at some past moment. Copies do not follow their originals. Two
separate faults were found at once in #14:

- No resistance tables at all, as above.
- `npc.iron_brawler` still had `atk: 105`. Change G shipped `atk: 135` two
  milestones earlier, so the row labelled "control" had silently stopped being
  the shipped creature.

**Diff this directory against `data/` before every sweep.** Party members, the
Flame Assassin and any shipped combatant appearing in a probe matchup must match
their `data/combatants.json` entries field for field, resistance tables
included.

### The control is `probe.curve_a135`, and it is not optional

One row of the sweep must be an exact reconstruction of a shipped encounter.
Today that is `probe.curve_a135`, because the shipped Iron Brawler has `atk`
135 and `matchup.assassin_ambush` is the encounter it appears in. **If Change G
is ever superseded, the control moves with it and this line must be updated.**

Run the control against the shipped figures first, before reading any other row:

```text
cd src
cargo run -q -p rpg-core --bin balance -- --only matchup.assassin_ambush
```

The two reports must agree in **every digit** -- win count, all three turn
figures, every combatant row, and every skill-use count. Bit-identity is
available here because the seed lists are the same and the engine is
deterministic, so there is no reason to accept less than it.

**A control that does not reproduce exactly means the sweep is void, not that
the difference is interesting.** Whatever made the control disagree also applies
to the other rows, where there is nothing to compare against and no way to
separate it from the effect being measured. Fix the roster, then re-run
everything.

### Keep the seed lists at 300 and keep them identical across rows

All six matchups use the contiguous range `1..=300`. Issue #12 established why:
twelve seeds carry about +/-14 points of sampling error on a win rate, which is
wider than most differences worth measuring, and 1800 battles cost well under a
second. Rows must share a seed list, or differences between them mix the effect
under test with a change of sample.

### Only one field may differ between probe clones

The six clones differ in `atk` and in nothing else -- same `hp`, `sp`, `def`,
`spd`, `will`, same Stand, same skills, same AI profile, same resistance table.
This is the whole reason a sweep can attribute its trend to a single stat. It is
also why issue #27 is a real question rather than a shrug: Kakyoin's skill mix
moves across the range, and because `spd` is 62 on every row, whatever is
driving that cannot be the thing `spd_down` counters.

If you add a probe, add it as a clone with one field changed, and say in the
matchup name which field that is.
