# Yet-to-be JoJo's RPG Game

A turn-based RPG built as a **deterministic simulation core in Rust** with a thin
presentation layer in **Godot 4**, driven entirely by external content data.

> Status: **pre-alpha / architecture scaffold.** The battle core is implemented
> and unit-tested. The Godot presentation layer is a headless-driven stub.

## Why this repo is structured this way

The engineering thesis is a hard separation between *rules* and *rendering*:

```
            data/*.json                 (content: stands, skills, combatants)
                 |
                 v
  +--------------------------------+
  |  src/core   (crate: rpg-core)  |   pure Rust, zero engine deps
  |  - deterministic RNG (SplitMix64)
  |  - tempo/ATB scheduler          |   same seed + same commands
  |  - command -> event pipeline    |   => byte-identical event log
  |  - unit tested, headless        |
  +--------------------------------+
                 |  JSON over one narrow FFI surface
                 v
  +--------------------------------+
  | src/bridge  (crate: rpg-bridge)|   cdylib, GDExtension (godot-rust)
  +--------------------------------+
                 |
                 v
  +--------------------------------+
  | src/game    (Godot 4 project)  |   scenes, animation, input, audio
  +--------------------------------+
                 ^
  src/data-pipeline (Python)  ---- validates/lints data/*.json in CI
```

Consequences that matter:

- **The game logic is testable without launching the engine.** `cargo test` runs
  full battles headlessly in milliseconds.
- **Battles are replayable.** State carries its own RNG, so a seed plus the
  command list fully reproduces a fight. This is the foundation for bug repro,
  regression tests, and (later) netplay or an AI-vs-AI balance harness.
- **No content is hardcoded.** Every character, stand, and skill is defined in
  `data/*.json`. The core knows nothing about any specific franchise.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and
[`docs/ENGINE-DECISION.md`](docs/ENGINE-DECISION.md) for the reasoning and the
rejected alternatives. Milestones live in [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Layout

| Path | Purpose |
|---|---|
| `src/core/` | Deterministic battle simulation. No engine types allowed. |
| `src/bridge/` | GDExtension boundary. JSON in, JSON out. Kept deliberately thin. |
| `src/game/` | Godot 4 project: scenes, UI, audio, animation. |
| `src/data-pipeline/` | Python tooling that validates and reports on content data. |
| `data/` | Content definitions (JSON). The only place content lives. |
| `docs/` | Architecture decisions, design docs, roadmap. |

## Quickstart

```bash
# 1. Run the simulation test suite (no engine required)
cd src
cargo test

# 2. Validate content data
python3 src/data-pipeline/validate_data.py data

# 3. Build the GDExtension library
cd src
cargo build -p rpg-bridge
# then copy the produced cdylib into src/game/bin/ (see docs/ARCHITECTURE.md)
```

## Content and IP notice

All source code in this repository is licensed under the terms in `LICENSE`.
That license covers **code only**.

Character names, Stand names, and related concepts referenced in `data/` are the
intellectual property of their respective rights holders (Hirohiko Araki /
Shueisha / Lucky Land Communications). This is a **non-commercial fan and
portfolio project**. No official assets are redistributed here.

The engine is deliberately content-agnostic: swapping `data/*.json` and the art
layer converts this into a fully original game without touching a line of core
logic. That is an intentional risk-mitigation design choice, not an accident.
