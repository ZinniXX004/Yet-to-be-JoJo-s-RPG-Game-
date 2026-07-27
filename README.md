<div align="center">

# Yet-to-be JoJo's RPG Game

**A turn-based RPG whose combat rules live in a deterministic Rust simulation,
with Godot 4 doing nothing but showing you what happened.**

[![CI](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/actions/workflows/ci.yml/badge.svg)](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/actions/workflows/ci.yml)
[![Release](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/actions/workflows/release.yml/badge.svg)](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-?display_name=tag&sort=semver&logo=github)](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[![Rust](https://img.shields.io/badge/Rust-1.94.1-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Godot](https://img.shields.io/badge/Godot-4.6%2B-478CBF?logo=godotengine&logoColor=white)](https://godotengine.org)
[![godot-rust](https://img.shields.io/badge/godot--rust-0.5.3-8B4513?logo=rust&logoColor=white)](https://github.com/godot-rust/gdext)
[![Python](https://img.shields.io/badge/Python-3.11%2B-3776AB?logo=python&logoColor=white)](https://www.python.org)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)](docs/DEVELOPMENT.md)

[![Deterministic](https://img.shields.io/badge/simulation-deterministic-success)](src/core/tests/determinism.rs)
[![Conventional Commits](https://img.shields.io/badge/commits-conventional-FE5196?logo=conventionalcommits&logoColor=white)](https://www.conventionalcommits.org/en/v1.0.0/)
[![Keep a Changelog](https://img.shields.io/badge/changelog-keep%20a%20changelog-E05735?logo=keepachangelog&logoColor=white)](CHANGELOG.md)
[![SemVer](https://img.shields.io/badge/semver-2.0.0-3F4551?logo=semver&logoColor=white)](https://semver.org/spec/v2.0.0.html)

[Architecture](docs/ARCHITECTURE.md) ·
[Setup](docs/DEVELOPMENT.md) ·
[Roadmap](docs/ROADMAP.md) ·
[Releases](docs/RELEASING.md) ·
[Changelog](CHANGELOG.md) ·
[Data provenance](docs/DATA-SOURCES.md)

</div>

> **Status:** `v0.1.0`. The simulation is complete, tested and deterministic. The
> Godot layer is a runnable stub, not yet a playable game. See
> [ROADMAP.md](docs/ROADMAP.md).

---

## Why this exists

Most hobby RPGs put their combat rules inside engine callbacks, where the logic
cannot be tested, replayed or reasoned about. This project inverts that: the
rules are a plain Rust library with no engine, no globals, no clock and no
floating point, so a battle is a pure function of a seed plus a sequence of
commands.

That single constraint buys, for free:

- **Replays and bug reports** — one `u64` reproduces a fight exactly.
- **Real tests** — `cargo test` runs battles; no engine, no scene, no mocking.
- **Trivial saves** — serialising `BattleState` is the entire save file.
- **Balance at scale** — thousands of headless AI battles per second.
- **Portability** — the same simulation can drive a different frontend later.

---

## Architecture

```text
┌───────────────────────────────────────────────┐
│ src/game        Godot 4  ·  GDScript          │  presentation only:
│                 scenes, UI, animation         │  animates events,
└───────────────────────┬───────────────────────┘  computes nothing
                        │  JSON strings over GDExtension
┌───────────────────────▼───────────────────────┐
│ src/bridge      rpg-bridge  ·  Rust cdylib    │  translation only:
│                 5 methods, JSON in / JSON out │  no game rules
└───────────────────────┬───────────────────────┘
                        │
┌───────────────────────▼───────────────────────┐
│ src/core        rpg-core  ·  Rust library     │  every rule lives here:
│                 tempo scheduler, resolution,  │  deterministic,
│                 statuses, AI, seeded RNG      │  engine-free, tested
└───────────────────────┬───────────────────────┘
                        │  reads
┌───────────────────────▼───────────────────────┐
│ data/*.json     skills · stands · combatants  │  content, not code
│ src/data-pipeline  stdlib-only validator      │  runs in CI
└───────────────────────────────────────────────┘
```

| Concept | One-line summary |
| --- | --- |
| **Tempo scheduler** | Integer ATB: `tempo += spd` per tick, act at 1000, each action subtracts its own cost — so speed is a resource |
| **Tempo denial** | `tempo_lock` suspends accumulation; time-stop abilities are ordinary data, with no special case in the scheduler |
| **Command → Event** | Intent is validated *before* mutation; a rejected command costs no turn. Events are the only channel to the UI |
| **Determinism** | SplitMix64 RNG stored inside `BattleState`; no floats anywhere |
| **Data-driven** | A new skill is a JSON object. Zero code changes |

Full reasoning in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); the engine choice
is recorded as ADR-0001 in [docs/ENGINE-DECISION.md](docs/ENGINE-DECISION.md).

---

## Quickstart (Windows)

Requires the Rust toolchain, MSVC Build Tools, Godot 4.6+ and Python 3.11+ —
exact versions and install links in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

```powershell
git clone https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-.git
cd Yet-to-be-JoJo-s-RPG-Game-
git lfs install

# 1. The simulation must pass on its own, without Godot.
cd src
cargo test -p rpg-core --all-targets

# 2. Build the GDExtension library.
cargo build -p rpg-bridge
cd ..

# 3. Wire it into the Godot project (both directories are gitignored build output).
New-Item -ItemType Directory -Force -Path src\game\bin | Out-Null
Copy-Item src\target\debug\rpg_bridge.dll src\game\bin\ -Force
pwsh -File tools/sync_data.ps1

# 4. Open src/game/project.godot in Godot and run the main scene.
```

Prebuilt libraries for Windows, Linux and macOS are attached to every
[release](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases) with
SHA-256 checksums.

---

## Repository layout

| Path | Contents |
| --- | --- |
| `src/core/` | `rpg-core`: the simulation. No engine dependency |
| `src/bridge/` | `rpg-bridge`: GDExtension boundary, JSON in and out |
| `src/game/` | Godot 4 project: scenes, GDScript, extension descriptor |
| `src/data-pipeline/` | Stdlib-only Python content validator |
| `data/` | Canonical content: skills, stands, combatants |
| `docs/` | Architecture, ADR, roadmap, setup, release process, provenance |
| `tools/` | Content sync scripts for the Godot project |
| `.github/workflows/` | CI and release automation |

---

## Quality gates

Every push and pull request runs, on **Linux and Windows**:

| Check | Tool |
| --- | --- |
| Formatting | `cargo fmt --check` |
| Lints, warnings are errors | `cargo clippy -- -D warnings` |
| Tests, including determinism | `cargo test --all-targets` |
| Docs build | `cargo doc --no-deps` |
| Content integrity | `validate_data.py` (types, enums, cross-references, balance smells) |
| Supply chain: advisories, licences, sources | [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) |
| Spelling | [`typos`](https://github.com/crate-ci/typos) |
| Dead Markdown links | [`lychee`](https://github.com/lycheeverse/lychee) |
| Dependency updates | Dependabot, weekly and grouped |

Tagging `vX.Y.Z` builds, tests and publishes per-platform archives with release
notes taken from the changelog. The workflow refuses to publish if the tag does
not match `Cargo.toml` or the changelog has no section for it. Details in
[docs/RELEASING.md](docs/RELEASING.md).

---

## References and further reading

**Toolchain and bindings**

- [The Rust Programming Language](https://doc.rust-lang.org/book/) · [Rust API guidelines](https://rust-lang.github.io/api-guidelines/)
- [godot-rust book](https://godot-rust.github.io/book/) · [gdext API docs](https://godot-rust.github.io/docs/gdext) · [gdext repository](https://github.com/godot-rust/gdext)
- [godot-rust compatibility matrix](https://godot-rust.github.io/book/toolchain/compatibility.html) — API version must be at most the runtime version
- [Selecting a Godot version](https://godot-rust.github.io/book/toolchain/godot-version.html) — why `compatibility_minimum` is 4.6 here
- [Godot 4 documentation](https://docs.godotengine.org/en/stable/) · [The GDExtension system](https://docs.godotengine.org/en/stable/tutorials/scripting/gdextension/index.html)

**Design and algorithms**

- Steele, Lea and Flood, *Fast Splittable Pseudorandom Number Generators*, OOPSLA 2014 — [ACM](https://dl.acm.org/doi/10.1145/2714064.2660195); reference constants at [prng.di.unimi.it](https://prng.di.unimi.it/splitmix64.c)
- [Game Programming Patterns](https://gameprogrammingpatterns.com/) — command and event patterns, free online
- [Determinism in games: floating point](https://gafferongames.com/post/floating_point_determinism/) — why this project has no float math

**Process**

- [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) · [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) · [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
- [Architecture decision records](https://adr.github.io/) — the format of `docs/ENGINE-DECISION.md`
- [GitHub Actions documentation](https://docs.github.com/actions) · [Shields.io](https://shields.io) · [Simple Icons](https://simpleicons.org)

Every external resource, licence and the project's scraping policy are catalogued
in [docs/DATA-SOURCES.md](docs/DATA-SOURCES.md).

---

## Licence and IP notice

Code is [MIT](LICENSE) licensed.

Character and ability names reference *JoJo's Bizarre Adventure* by Hirohiko
Araki (Shueisha). This is an unaffiliated, non-commercial fan project; no
official assets are included. `rpg-core` contains **no** character names — the
engine is IP-agnostic, so swapping `data/*.json` produces an original game
without touching a line of code. See
[docs/DATA-SOURCES.md](docs/DATA-SOURCES.md#4-intellectual-property-notice).
