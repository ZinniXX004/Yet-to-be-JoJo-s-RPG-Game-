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
