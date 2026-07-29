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
[![Balance](https://img.shields.io/badge/balance-measured%2C%20not%20asserted-success)](docs/BALANCE-LOG.md)
[![Conventional Commits](https://img.shields.io/badge/commits-conventional-FE5196?logo=conventionalcommits&logoColor=white)](https://www.conventionalcommits.org/en/v1.0.0/)
[![Keep a Changelog](https://img.shields.io/badge/changelog-keep%20a%20changelog-E05735?logo=keepachangelog&logoColor=white)](CHANGELOG.md)
[![SemVer](https://img.shields.io/badge/semver-2.0.0-3F4551?logo=semver&logoColor=white)](https://semver.org/spec/v2.0.0.html)

[Architecture](docs/ARCHITECTURE.md) ·
[Setup](docs/DEVELOPMENT.md) ·
[Roadmap](docs/ROADMAP.md) ·
[Triage](docs/TRIAGE.md) ·
[Balance log](docs/BALANCE-LOG.md) ·
[Balance curve](docs/BALANCE-CURVE.md) ·
[Releases](docs/RELEASING.md) ·
[Changelog](CHANGELOG.md) ·
[Data provenance](docs/DATA-SOURCES.md)

</div>

> **Status: `0.3.0` released. `0.4.0` (M3, content depth) in progress.**
> `0.2.0` made one battle playable start to finish in Godot 4.7.1 with a clean
> console. `0.3.0` made the numbers accountable: every encounter declares the
> win rate it is supposed to produce, a headless harness measures the real one
> over twelve fixed seeds, and CI fails the build when the two disagree. The
> first playthrough's defeat turned out to be an anecdote pointing the wrong
> way — unattended, the party won that same fight 92% of the time, and two of
> three encounters sat outside their intended range.
>
> `0.4.0` closes the hole those measurements were taken through: the AI scores
> a skill by damage alone, so three of eleven are unreachable —
> `skill.guard_stance` and `skill.rage_focus` deal none and are filtered out,
> and `skill.tempo_halt` is always beaten by a stronger option on every
> combatant that owns it. Nothing in the game has ever guarded, buffed or
> denied tempo, so every win rate published so far measures a subset of the
> game. Rules first, then a wider seed list, then new content — in that order,
> because changing the rules invalidates every number measured before it. Plan
> in [ROADMAP.md](docs/ROADMAP.md), reporting rules in
> [TRIAGE.md](docs/TRIAGE.md), evidence in
> [BALANCE-LOG.md](docs/BALANCE-LOG.md).

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

The fourth item is not aspirational. It is [a CI
job](.github/workflows/ci.yml) that has already rejected the content this
repository shipped in `0.2.0`.

---

## Architecture

```text
┌───────────────────────────────────────────────┐
│ src/game        Godot 4  ·  GDScript          │  presentation only:
│                 scenes, UI, animation         │  animates events,
└──────────────────────┬──────────────────────┘  computes nothing
                        │  JSON strings over GDExtension
┌──────────────────────▼──────────────────────┐
│ src/bridge      rpg-bridge  ·  Rust cdylib    │  translation only:
│                 5 methods, JSON in and out    │  no game rules
└──────────────────────┬──────────────────────┘
                        │
┌──────────────────────▼──────────────────────┐
│ src/core        rpg-core  ·  Rust library     │  every rule lives here:
│                 tempo scheduler, resolution,  │  deterministic,
│                 statuses, AI, seeded RNG      │  engine-free, tested
└──────────────────────┬──────────────────────┘
                        │  reads
┌──────────────────────▼──────────────────────┐
│ data/*.json     skills · stands · combatants  │  content, not code
│                 matchups: declared win rates  │
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
| **Declared balance** | An encounter states its intended win rate in `data/matchups.json`; the harness measures the real one and CI compares them |

Full reasoning in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); the engine choice
is recorded as ADR-0001 in [docs/ENGINE-DECISION.md](docs/ENGINE-DECISION.md).

---

## Quickstart (Windows)

Requires the Rust toolchain, MSVC Build Tools, Godot 4.6+ and Python 3.11+ —
exact versions and install links in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

Every block below is PowerShell, not `cmd.exe`, and paths are relative to the
directory the block says you are in.

```powershell
git clone https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-.git
cd Yet-to-be-JoJo-s-RPG-Game-
git lfs install

# 1. The simulation must pass on its own, without Godot.
cd src
cargo test -p rpg-core --all-targets

# 2. Build the GDExtension library.
cargo build -p rpg-bridge

# 3. Back to the repository root. The paths below start with src\, so running
#    them from inside src\ looks for src\src\ and fails.
cd ..

# 4. Wire it into the Godot project (both directories are gitignored output).
New-Item -ItemType Directory -Force -Path src\game\bin | Out-Null
Copy-Item src\target\debug\rpg_bridge.dll src\game\bin\ -Force
.\tools\sync_data.ps1

# 5. Open src/game/project.godot in Godot and run the main scene.
```

The sync script is Windows PowerShell 5.1 compatible, so PowerShell 7 is not
required. If your execution policy blocks the script, run
`powershell -ExecutionPolicy Bypass -File tools\sync_data.ps1` instead; that
affects one process and changes no machine-wide setting. On Linux and macOS,
run `sh tools/sync_data.sh`.

Prebuilt libraries for Windows, Linux and macOS are attached to every
[release](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases) with
SHA-256 checksums.

---

## Balance

Every encounter lives in [`data/matchups.json`](data/matchups.json) with a party,
a set of foes, twelve fixed seeds and the win rate it is *supposed* to produce.
The harness plays all of them with both sides on AI and reports the win rate it
actually produces:

```powershell
cd src
cargo run -q -p rpg-core --bin balance
```

Current measurement, over 12 seeds per encounter:

| Encounter | Win rate | Declared band | Median length |
| --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 | 3 turns |
| `matchup.assassin_ambush` | 67% | 45..90 | 19 turns |
| `matchup.dio_boss` | 50% | 35..75 | 50 turns |

The binary exits non-zero when any encounter leaves its band, so it is both a
tool and a gate. `core/tests/balance_bounds.rs` asserts the same thing under
`cargo test`, and a `balance` CI job blocks the merge.

Useful flags: `--only <matchup.id>` for one encounter, `--json` for a machine
readable report, `--no-fail` to print without a non-zero exit, and
`--combatants` / `--stands` / `--skills` to run against content outside `data/`.
That last group is how the response curve in
[docs/BALANCE-CURVE.md](docs/BALANCE-CURVE.md) was swept in a single run:

```powershell
cd src
cargo run -q -p rpg-core --bin balance -- `
  --combatants ..\tools\probe\combatants.probe.json `
  --matchups ..\tools\probe\matchups.probe.json --no-fail
```

[`tools/probe/`](tools/probe/README.md) is measurement scaffolding, not content;
it never reaches the game, the validator or the CI gate.

Two documents carry the reasoning, and they are the interesting part of this
repository rather than an appendix to it:

- **[docs/BALANCE-LOG.md](docs/BALANCE-LOG.md)** — one entry per change, each
  with the prediction written *before* the run. Seven of nine predictions were
  wrong, which is the argument for measuring rather than against it.
- **[docs/BALANCE-CURVE.md](docs/BALANCE-CURVE.md)** — how much win rate a point
  of enemy damage actually buys. Flat below a ratio of 0.65, a bend to 67%, a
  plateau to 0.81, then 8% by parity. Six changes were spent moving a number
  along the flat part of a curve nobody had plotted yet.

A win rate here is a **floor for a competent player**, not a forecast: the AI
takes the strongest affordable skill and never sets up, and twelve seeds resolve
to 8.3 percentage points. Both limits, and five more, are listed at the end of
the balance log. If you want to dispute a number, [TRIAGE.md](docs/TRIAGE.md)
says what a balance finding has to contain before it can be acted on.

---

## Repository layout

| Path | Contents |
| --- | --- |
| `src/core/` | `rpg-core`: the simulation, the batch simulator and the `balance` binary. No engine dependency |
| `src/bridge/` | `rpg-bridge`: GDExtension boundary, JSON in and out |
| `src/game/` | Godot 4 project: scenes, GDScript, extension descriptor |
| `src/data-pipeline/` | Stdlib-only Python content validator |
| `data/` | Canonical content: skills, stands, combatants, matchups |
| `docs/` | Architecture, ADR, roadmap, setup, release process, triage, provenance, balance |
| `tools/` | Content sync scripts for the Godot project |
| `tools/probe/` | Throwaway content for curve sweeps. Not game content |
| `.github/workflows/` | CI and release automation |
| `.github/ISSUE_TEMPLATE/` | Defect, balance and task forms; blank issues are disabled |

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
| Balance | `cargo run -p rpg-core --bin balance` — every encounter inside its declared win-rate band |
| Supply chain: advisories, licences, sources | [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) |
| Spelling | [`typos`](https://github.com/crate-ci/typos) |
| Dead Markdown links | [`lychee`](https://github.com/lycheeverse/lychee) |
| Dependency updates | Dependabot, weekly and grouped |

None of these run Godot. The GDScript layer is verified by playing it, which is
why every milestone's exit criterion is written as an observable playthrough
rather than a green pipeline.

Tagging `vX.Y.Z` builds, tests and publishes per-platform archives with release
notes taken from the changelog. The workflow refuses to publish if the tag does
not match `Cargo.toml` or the changelog has no section for it. Details in
[docs/RELEASING.md](docs/RELEASING.md).

---

## Contributing and reporting

Issues use three forms — defect, balance finding, task — and blank issues are
disabled, because a report this project cannot reproduce is a report it cannot
act on. A defect report needs the seed, the encounter id and the version; the
simulation is deterministic, so those three reproduce the fight exactly. A
balance finding needs the pasted harness output, not an impression. Labels,
severity and the triage rules are in [docs/TRIAGE.md](docs/TRIAGE.md).

Pull requests carry the local gate as a checklist. Run it before opening one;
the commands are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md#4-checks-to-run-before-pushing).

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
