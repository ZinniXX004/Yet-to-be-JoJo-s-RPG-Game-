# Development Setup (Windows-first)

Written against the exact environment this project is developed on:

```text
Windows 10.0.26200
rustc 1.94.1 (e408947bf 2026-03-25)
cargo 1.94.1 (29ea6fb6a 2026-03-24)
```

Linux and macOS work too; where a command differs, both forms are given.

---

## 1. Requirements

### 1.1 Mandatory

| Component | Version | Why it is required | Where |
| --- | --- | --- | --- |
| Rust toolchain | **1.94.1** (pinned) | Simulation and GDExtension library. `godot` 0.5 sets the floor at 1.94.0 | [rustup.rs](https://rustup.rs) |
| MSVC Build Tools | 2022, workload *Desktop development with C++* | Rust on Windows uses `link.exe`. Without it, every build fails at link time | [Visual Studio downloads](https://visualstudio.microsoft.com/downloads/) |
| Godot | **4.2 or newer**, standard build (not .NET) | Runs the GDExtension. The .NET build is unnecessary here and only adds weight | [godotengine.org/download](https://godotengine.org/download) |
| Python | 3.11+ | Content validator in CI and locally | [python.org](https://www.python.org/downloads/) |
| Git | any recent | Version control | [git-scm.com](https://git-scm.com/downloads) |
| Git LFS | any recent | `.gitattributes` routes binary assets to LFS. **Install before your first asset commit** | [git-lfs.com](https://git-lfs.com) |

The toolchain version is not a suggestion: [`rust-toolchain.toml`](../rust-toolchain.toml)
pins it, and rustup will install and use 1.94.1 automatically for any cargo
command run inside the repository.

### 1.2 Optional but recommended

| Tool | Install | What it buys you |
| --- | --- | --- |
| `cargo-watch` | `cargo install cargo-watch --locked` | Recompile on save; pair it with Godot's GDExtension hot reload |
| `cargo-deny` | `cargo install cargo-deny --locked` | Run the CI supply-chain audit locally before pushing |
| `typos-cli` | `cargo install typos-cli --locked` | Same spellcheck CI runs |
| `cargo-edit` | `cargo install cargo-edit --locked` | `cargo set-version` for release bumps |
| LLVM/clang | [releases.llvm.org](https://releases.llvm.org/) | Only needed if you switch godot-rust to `api-custom`; the default prebuilt bindings do not need it |
| VS Code + `rust-analyzer` + `godot-tools` | Marketplace | Editor integration; `.editorconfig` is already in the repo |

### 1.3 Rust dependencies

The whole tree is intentionally small. A short dependency list is a feature: it
keeps builds fast, audits meaningful, and the code honest about what it actually
needs.

| Crate | Version | Used by | Purpose |
| --- | --- | --- | --- |
| `serde` | `1` (feature `derive`) | `rpg-core` | Serialise state, content and events |
| `serde_json` | `1` | `rpg-core`, `rpg-bridge` | JSON at the content and FFI boundaries |
| `godot` | `=0.5.3` | `rpg-bridge` | Godot 4 GDExtension bindings ([gdext](https://github.com/godot-rust/gdext)) |

`rpg-core` has **zero engine dependencies** and builds and tests without Godot
installed. That is enforced by the CI job that compiles and tests it alone.

`godot` is pinned with `=` because godot-rust is pre-1.0 and ships breaking
changes in minor versions. Its licence is MPL-2.0, which is why `src/deny.toml`
allows MPL-2.0 explicitly.

Python tooling has **no** third-party dependencies
(`src/data-pipeline/requirements.txt` is intentionally empty).

---

## 2. First-time setup

Run each block from the repository root. PowerShell is assumed on Windows.

```powershell
# 1. Toolchain sanity check. rustup switches to the pinned 1.94.1 automatically.
rustc -V
cargo -V

# 2. Clone and enable LFS.
git clone https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-.git
cd Yet-to-be-JoJo-s-RPG-Game-
git lfs install
git checkout development

# 3. Build and test the simulation. This must pass before anything else matters.
cd src
cargo test -p rpg-core --all-targets
cd ..

# 4. Validate the content data.
python src/data-pipeline/validate_data.py
```

Expected result: tests green, validator exits 0. If step 3 fails, stop. The
Godot layer is meaningless if the simulation is broken.

---

## 3. Building the GDExtension

```powershell
# Build the cdylib.
cd src
cargo build -p rpg-bridge
cd ..

# Copy the library where rpg.gdextension expects it.
New-Item -ItemType Directory -Force -Path src\game\bin | Out-Null
Copy-Item src\target\debug\rpg_bridge.dll src\game\bin\ -Force

# Copy content into res:// (Godot cannot read outside the project directory).
pwsh -File tools/sync_data.ps1
```

Linux/macOS equivalent:

```sh
cd src && cargo build -p rpg-bridge && cd ..
mkdir -p src/game/bin
cp src/target/debug/librpg_bridge.* src/game/bin/
sh tools/sync_data.sh
```

Then open `src/game/project.godot` in Godot and run the main scene. `src/game/bin/`
and `src/game/data/` are gitignored: they are build output, not source.

### Iteration loop

```powershell
# Terminal 1: rebuild on save.
cd src
cargo watch -c -x "clippy -p rpg-core --all-targets" -x "test -p rpg-core" -x "build -p rpg-bridge"
```

GDExtension supports hot reload from Godot 4.2, so the editor picks up a rebuilt
library without a restart in most cases. Tuning numbers does **not** require a
rebuild at all: edit `data/*.json`, re-run the sync script, restart the scene.

---

## 4. Checks to run before pushing

These are exactly what CI runs. Running them locally turns a 6-minute red
pipeline into a 30-second local failure.

```powershell
cd src
cargo fmt --all --check
cargo clippy -p rpg-core  --all-targets --all-features -- -D warnings
cargo clippy -p rpg-bridge --all-targets -- -D warnings
cargo test  -p rpg-core --all-targets --all-features
cargo doc   -p rpg-core --no-deps
cargo deny check advisories bans licenses sources   # optional tool
cd ..
python src/data-pipeline/validate_data.py
typos                                              # optional tool
```

---

## 5. Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `error: linker 'link.exe' not found` | MSVC Build Tools missing | Install VS Build Tools 2022 with *Desktop development with C++*, then reopen the terminal |
| Godot: `Can't open dynamic library` | Library not in `src/game/bin/`, or a 32-bit/64-bit mismatch | Rebuild and copy again; confirm the path in `src/game/rpg.gdextension` |
| Godot: `Class 'BattleSession' not found` | Extension not loaded | Check `entry_symbol = "gdext_rust_init"`, that `rpg.gdextension` sits beside `project.godot`, and reopen the project |
| Godot: extension built for a newer API | `compatibility_minimum` above your Godot build | Lower it, or upgrade Godot; API version must be <= runtime version ([compatibility docs](https://godot-rust.github.io/book/toolchain/compatibility.html)) |
| `missing content file: res://data/...` | Sync script not run | `pwsh -File tools/sync_data.ps1` |
| Assets appear as small text files | Git LFS not installed before cloning | `git lfs install` then `git lfs pull` |
| CI complains about line endings | Committed CRLF | `.gitattributes` normalises to LF; re-add the file, do not disable core.autocrlf globally |
| `libclang.dll not found` | Building with the `api-custom` feature | Install LLVM and set `LIBCLANG_PATH`, or stay on the default prebuilt bindings |

---

## 6. Layering rules (enforced by review, not by the compiler)

1. `rpg-core` must never reference an engine type, a clock, a global, a thread or
   a float.
2. `rpg-bridge` must contain no game rules. Only translation.
3. The Godot layer must never recompute an outcome. Numbers reach the screen only
   through events.
4. New content must not require new code. If you find yourself matching on a
   specific skill id, the design has regressed.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the reasoning and
[ENGINE-DECISION.md](ENGINE-DECISION.md) for why this stack was chosen.
