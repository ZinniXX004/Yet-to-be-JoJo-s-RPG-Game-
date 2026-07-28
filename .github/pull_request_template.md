## What this changes

<!-- One paragraph. The end state, not the activity. -->

## Why

<!-- Link the issue, or the roadmap item, or the measurement that forced it. -->

Closes #

## Evidence

<!--
Paste the output that proves it works. For a balance change, paste the
balance report before and after. For a defect fix, name the test that
fails without the fix.
-->

## Local gate

Run from `src/` unless noted. Tick what you actually ran; do not tick what CI
will run for you.

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy -p rpg-core --all-targets --all-features -- -D warnings`
- [ ] `cargo clippy -p rpg-bridge --all-targets -- -D warnings`
- [ ] `cargo test -p rpg-core --all-targets --all-features`
- [ ] `cargo deny check`
- [ ] `python src\data-pipeline\validate_data.py` (from the repo root)
- [ ] `typos` (from the repo root)

Not runnable locally, will run in CI: the `links` job (lychee).

## Rules boundary

- [ ] This PR touches `ai.rs`, `resolve.rs`, `battle.rs` or `rng.rs`

If ticked: RNG draw order changed, so every previously measured win rate is
void. Re-measure with the harness and say so in `docs/BALANCE-LOG.md`. Numbers
carried across this boundary are not comparable, however similar they look.

## Content changes

- [ ] Adds or edits an encounter in `data/matchups.json`

If ticked: it declares a win-rate band, and `balance_bounds` passes against it.
An encounter without a band is not finished, and widening a band to make CI
green is not a fix.

## Docs

- [ ] `CHANGELOG.md` updated under `## [Unreleased]`
- [ ] Every relative link I added points at a file that exists on this branch
- [ ] No version number left behind (`README.md`, `docs/`, workflow examples)
