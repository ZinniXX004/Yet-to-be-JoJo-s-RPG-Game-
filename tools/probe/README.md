# `tools/probe/`

**This directory is not game content.** The two JSON files here have the same
shape as `data/combatants.json` and `data/matchups.json`, but nothing loads them
unless a path is pointed at them by hand: the game reads `data/`, and the
blocking `balance` CI job runs with no arguments, which can only read the content
compiled into the binary by `include_str!`. These files exist to answer
measurement questions that would otherwise need one throwaway commit per data
point -- the combatants are deliberately absurd (`atk` up to 225, roughly half
again as hard-hitting as the final boss) and the bands are `0..100` so a probe
can never report a violation. Numbers measured here are only meaningful once they
are re-measured on shipped content; the curve they produced is written up in
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

### The mirrored entries are checked by CI now, and only those

Issue #30 added a comparison to the content validation job:

```text
python src/data-pipeline/validate_data.py \
    --combatants tools/probe/combatants.probe.json \
    --matchups   tools/probe/matchups.probe.json \
    --mirror     data
```

Every combatant id that appears in **both** this directory and
`data/combatants.json` must be identical, field for field, resistance tables
included. Today that is all seven shipped combatants. Edit a shipped stat without
updating the copy here and CI goes red, naming the combatant and the field.

Note what is deliberately *not* covered. The six `npc.probe_a*` clones have no
shipped counterpart, so nothing constrains them -- being invented is the whole
point of them. The probe matchups are not compared either, because they are
supposed to differ: `0..100` bands, `max_turns: 300`, their own encounter list.
That run therefore prints warnings by design, six of them about bands that
declare no intent, and warnings do not fail it.

**This does not make the numbers here trustworthy, only current.** A clone with a
plausible `atk` and a stale idea of the rules is still fiction; see the control
section below.

### Why a human diff was not good enough, and why one file check is not either

This section used to say the directory had no validation and instruct a reader to
diff it against `data/` before every sweep. Two things were wrong with that.

The first is that the instruction was never followed, which is why #14 happened.

The second is subtler and worth keeping in mind whenever a check is added here:
**validating this directory on its own could not have found either fault.**

- **`resist` is `#[serde(default)]`.** A roster file written before issue #10
  loads without error, without warning, with every resistance table empty. Six
  Iron Brawler clones were taking full physical damage from Jotaro where the
  shipped Brawler takes 25% less, and ordinary psychic damage from Kakyoin where
  the shipped one takes 25% more. An absent table is legal, and the validator
  returns immediately when it sees one -- correctly, because most combatants have
  none.
- **`npc.iron_brawler` still had `atk: 105`.** Change G shipped `atk: 135` two
  milestones earlier. 105 is a positive integer and no schema check has an
  opinion about which positive integer it ought to be.

Both files were internally consistent. They had simply stopped being descriptions
of the game, and the information needed to notice that was in another directory.
The same applies to any future field added with a default: the property that
makes save-data migration painless makes drift here invisible to anything except
a comparison.

For the record, the old instruction could not have been carried out as written
even by someone willing: `validate_data.py` looks for `skills.json`,
`stands.json`, `combatants.json` and `matchups.json`, and this directory contains
none of those names. A bare directory argument died on four missing files before
checking anything. The per-file overrides above exist because of that.

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

The mirror check narrows what can go wrong here but does not replace this step.
It compares the roster, not the rules: a change to `resolve.rs` or `ai.rs` moves
the control while every field still matches.

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
