# Contributing

This is a solo portfolio project, but the process still gets written down:
the rules are what keep the architecture from eroding over time, and
following them visibly is part of what this repository is meant to show.

## Setup

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). The toolchain is pinned; do not
override it locally.

## Branches

| Branch | Rule |
| --- | --- |
| `main` | Released, tagged states only. Protected: PR plus the `ci` check |
| `development` | Integration branch. Everything lands here first |
| `feat/*`, `fix/*`, `docs/*` | Short-lived topic branches off `development` |

## Commits

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/), scoped
by layer: `feat(core):`, `fix(bridge):`, `ci:`, `docs(readme):`.

The body should explain *why* a change happened — the diff already shows
*what* changed.

## Before opening a PR

Run the local checks listed in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md#4-checks-to-run-before-pushing). CI
runs the same set on Linux and Windows and clippy is `-D warnings`, so a warning
is a failure.

## Non-negotiable design rules

1. `rpg-core` stays engine-free: no Godot types, no globals, no clock, no
   threads, **no floats**. Floats break replay determinism across architectures.
2. `rpg-bridge` translates only. No game rules.
3. The Godot layer animates events; it never recomputes an outcome.
4. New content must not require new code. A `match` on a specific skill id is a
   design regression.
5. New rules need a test. Determinism is verified by
   `src/core/tests/determinism.rs`; if a change makes battles diverge for a fixed
   seed, that is a bug, not a flaky test.
6. External data requires a provenance entry. See
   [docs/DATA-SOURCES.md](docs/DATA-SOURCES.md).

## Releasing

See [docs/RELEASING.md](docs/RELEASING.md). Every fixation gets a version bump, a
changelog section and a tag.
