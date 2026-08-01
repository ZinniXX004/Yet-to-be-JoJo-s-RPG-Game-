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

## The two kinds of probe in here

They are not interchangeable and the difference decides what a row can be
compared against.

- **Stat probes** -- `probe.curve_a075` through `probe.curve_a225`. Six
  encounters that differ only in one enemy's `atk`, fought by invented
  `npc.probe_a*` clones. These produce the response curve.
- **Composition probes** -- `probe.chariot_duel`, added in issue #15. Same
  encounter as `matchup.assassin_ambush` with **one party member swapped**:
  Polnareff in Jotaro's place. Nothing about any combatant's numbers changes.

A composition probe is the honest home for a question of the form "what does
this character contribute", and issue #15 is the reason the distinction is
written down. `chariot_duel` was first authored into `data/matchups.json` with a
`20..60` band, measured 0 wins from 300 battles, and turned the CI gate red. It
was never a shipped encounter -- it is `assassin_ambush` with one variable
changed, which is the definition of a probe -- and it would have belonged here at
53% exactly as much as it did at 0%. **Do not put an experiment in `data/`. A
band is a statement of design intent, and a fixture built to answer a question
has no design intent to declare.**

### Composition probes fight the real mirrored combatants

This is the part that changes how the rest of this file must be read.
`chariot_duel`'s foes are `npc.flame_assassin` and `npc.iron_brawler` -- the
**mirrored copies of shipped combatants**, not clones. Its party is
`pc.polnareff` and `pc.kakyoin`, likewise mirrored. A stat probe reads its
numbers from invented entries that only exist here; a composition probe reads
them from copies that are supposed to be identical to `data/`.

So for a composition probe, a stale mirror does not merely mislead a reader. It
silently produces a wrong result, in the same way the missing resistance tables
did in issue #14. The mirror check below is what prevents that, and this is a
stronger justification for it than the one issue #30 argued.

## Read this before running a sweep

Everything below was learned the hard way in issue #14, where this roster had
been quietly wrong for two milestones and nothing in the project noticed.

### The mirrored entries are checked by CI now, and only those

Issue #30 added a comparison to the content validation job. **Run it from the
repository root, not from `src`** -- unlike the `cargo` commands above and below,
the path to the script is relative to where you stand, and `cd src` first gives
you `src/src/data-pipeline/...` and a file-not-found:

```text
cd <repo root>
python src/data-pipeline/validate_data.py \
    --combatants tools/probe/combatants.probe.json \
    --matchups   tools/probe/matchups.probe.json \
    --mirror     data
```

Every combatant id that appears in **both** this directory and
`data/combatants.json` must be identical, field for field, resistance tables
included. Today that is all eight shipped combatants. Edit a shipped stat without
updating the copy here and CI goes red, naming the combatant and the field.

On an unmodified checkout the run ends like this. Ten warnings and a trailing
note are the expected state, not a problem to fix:

```text
checked 13 skills, 7 stands, 14 combatants, 7 matchups, 8 mirrored: 0 error(s), 10 warning(s)
note: paths were overridden, so this run does not describe the shipped content
```

The `8 mirrored` is the part to read. A comparison that silently matched nothing
would also report zero errors, so the count is the difference between green and
lucky. It should equal the number of combatants in `data/combatants.json`; if it
is lower, an entry here has been deleted or renamed rather than updated.

Of the ten warnings, **seven** are the `0..100` bands declaring no intent, which
is deliberate here -- one per matchup. The other **three** are combatants this
directory carries that no probe encounter uses: `pc.josuke`, `npc.dio` and
`npc.thug`.

### What the check still cannot protect, and why that is tolerable

Note what is deliberately *not* covered. The six `npc.probe_a*` clones have no
shipped counterpart, so nothing constrains them -- being invented is the whole
point of them. The probe matchups are not compared either, because they are
supposed to differ: `0..100` bands, `max_turns: 300`, their own encounter list.

There is a sharper gap, found by running the check rather than by designing it.
When #30 rejected adding machinery to detect a *deleted* mirrored entry, the
argument was that a matchup referencing an unknown combatant is already an error,
so an entry that matters cannot vanish unnoticed. When that was first measured
the reference check covered only **three of seven** mirrored entries. Adding the
composition probe moved two more into the protected set, and Polnareff arrived
already inside it:

| Mirrored entry | Referenced by | Protected |
| --- | --- | --- |
| `pc.jotaro` | all six curve probes | yes |
| `pc.kakyoin` | curve probes, `chariot_duel` | yes |
| `pc.polnareff` | `chariot_duel` | yes |
| `npc.flame_assassin` | curve probes, `chariot_duel` | yes |
| `npc.iron_brawler` | `chariot_duel` | yes |
| `pc.josuke` | nothing | **no** |
| `npc.dio` | nothing | **no** |
| `npc.thug` | nothing | **no** |

**Five of eight.** The three unprotected ones are exactly the three warnings the
run prints, which is a useful property: the warning list *is* the list of entries
that could be deleted without anything complaining.

That is tolerable, because an entry no encounter references cannot influence a
sweep -- but it is tolerable for that reason and not because the reference check
covers it. An entry moves into the protected set the moment a probe starts using
it, and `chariot_duel` is the proof that this happens in practice rather than in
theory.

> **Retracted, by `probe.chariot_duel`.** This section used to claim that **the
> mirrored `npc.iron_brawler` is inert for measurement**, on the grounds that the
> sweep reads `atk` from the clones and never from it -- and concluded that its
> staleness in issue #14 misled a reader rather than corrupting a number. That
> was true of a directory containing only stat probes. It is false now.
> `chariot_duel` fights the mirrored `npc.iron_brawler` and `npc.flame_assassin`
> directly, so their fields land in a measured result. Do not carry the "inert
> mirror" framing forward. The `#14` entry in `docs/BALANCE-LOG.md` is accurate
> as written and attributes the resistance fault to the **clones**, which is
> where it belonged.

**None of this makes the numbers here trustworthy, only current.** A clone with a
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
deterministic, so there is no reason to accept less than it. As of issue #15 the
agreement is 158 won, 142 lost, turns 21/13/32, down to Kakyoin's six uses of
`strike`.

**A control that does not reproduce exactly means the sweep is void, not that
the difference is interesting.** Whatever made the control disagree also applies
to the other rows, where there is nothing to compare against and no way to
separate it from the effect being measured. Fix the roster, then re-run
everything.

The mirror check narrows what can go wrong here but does not replace this step.
It compares the roster, not the rules: a change to `resolve.rs` or `ai.rs` moves
the control while every field still matches. It also does not constrain the
clone the control actually uses, `npc.probe_a135`, because that id exists only
here -- so the control must still be reproduced by hand.

### Keep the seed lists at 300 and keep them identical across rows

All seven matchups use the contiguous range `1..=300`. Issue #12 established why:
twelve seeds carry about +/-14 points of sampling error on a win rate, which is
wider than most differences worth measuring, and 2100 battles cost well under a
second. Rows must share a seed list, or differences between them mix the effect
under test with a change of sample.

The validator warns below 100 seeds and reports the 95% interval a list that
small would carry (issue #31). It cannot warn about the opposite mistake -- rows
that disagree with each other -- so that one stays a reading rule.

### Only one field may differ between probe clones

The six clones differ in `atk` and in nothing else -- same `hp`, `sp`, `def`,
`spd`, `will`, same Stand, same skills, same AI profile, same resistance table.
This is the whole reason a sweep can attribute its trend to a single stat. It is
also why issue #27 is a real question rather than a shrug: Kakyoin's skill mix
moves across the range, and because `spd` is 62 on every row, whatever is
driving that cannot be the thing `spd_down` counters.

Nothing enforces this. The mirror check has no opinion about the clones, so a
second field drifting between them stays exactly as invisible as the resistance
tables were.

If you add a stat probe, add it as a clone with one field changed, and say in the
matchup name which field that is.

### The same rule for composition probes, applied to the roster

A composition probe changes **one participant** and nothing else -- same foes,
same seeds, same `max_turns`, same everything on every combatant sheet. Its
value comes entirely from having a shipped encounter to compare against, so name
that encounter in the matchup description and keep the swap to one slot.

One warning from `chariot_duel`, worth carrying into any future one: a
composition probe answers "what does this character contribute **to this
party**", and that does not generalise. Polnareff reads 4% here and moved the
four-person boss fight up 21 points, because a debuff protects everyone while the
offence it forgoes is one character's. **A two-person fixture understates a
support character and a four-person one flatters it.** Size the fixture to the
party the character is actually for, and say which one you chose and why.
