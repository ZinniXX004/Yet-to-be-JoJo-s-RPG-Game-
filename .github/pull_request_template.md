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

- [ ] `tools/check.ps1` (or `sh tools/check.sh`) passes in full, not only `-Fast`

It runs every CI job except the markdown link check, including the probe mirror
check and `typos` that the old hand-written list here left out.

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
- [ ] If any win rate moved: the README table, `docs/ROADMAP.md` and
      `docs/BALANCE-LOG.md` carry the new figures **in this PR**, not a later
      `docs/*` branch (the README table was six encounters stale by #33)
- [ ] Every relative link I added points at a file that exists on this branch
- [ ] No version number left behind (`README.md`, `docs/`, workflow examples)
