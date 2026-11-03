<a id="top"></a>

<div align="center">

<img src="assets/icons/screen.svg" width="56" alt="">

# Documentation

Everything that explains why this repository looks the way it does, grouped by
the question it answers. Start with the [README](../README.md) for the overview.

</div>

## How it is built

| Document | What it answers |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | The layer rules, the simulation model, the FFI contract between Rust and Godot, and how the build is wired |
| [ENGINE-DECISION.md](ENGINE-DECISION.md) | ADR-0001: why Godot 4 presents and Rust simulates, and what the alternatives were |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Setting up on Windows first, the local gate that mirrors CI, recording a playthrough, and every failure met so far |

## How it is run

| Document | What it answers |
| --- | --- |
| [ROADMAP.md](ROADMAP.md) | The milestones, each one a version tag, with what was planned and what was actually delivered |
| [RELEASING.md](RELEASING.md) | Versioning rules, the two pull requests of a release, and the checklist that keeps a tag honest |
| [TRIAGE.md](TRIAGE.md) | What a defect report or a balance finding must contain, labels and severity |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Branches, commits, what a pull request needs, and the design rules no change may break |
| [SECURITY.md](../SECURITY.md) | Which versions get security fixes, how to report a vulnerability privately, and what is in scope |

## How the balance is measured

| Document | What it answers |
| --- | --- |
| [BALANCE-LOG.md](BALANCE-LOG.md) | One entry per balance change, with the prediction written before the run and scored after it |
| [BALANCE-CURVE.md](BALANCE-CURVE.md) | How much win rate a point of enemy damage buys, swept with the probe roster |
| [BALANCE-M3-ENEMIES.md](BALANCE-M3-ENEMIES.md) | The issue #16 entry of the balance log: three enemies, three encounters, ten readings |

## Where things come from

| Document | What it answers |
| --- | --- |
| [DATA-SOURCES.md](DATA-SOURCES.md) | The provenance of every number and external dependency, the licences, and the intellectual property notice |
| [assets/README.md](assets/README.md) | How the README artwork and battle captures were made, and how to reproduce them |

<p align="right"><sub><a href="#top">Back to top</a></sub></p>
