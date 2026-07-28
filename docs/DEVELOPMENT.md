# Development Setup (Windows-first)

Written against the exact environment this project is developed on:

```text
Windows 10.0.26200
rustc 1.94.1 (e408947bf 2026-03-25)
cargo 1.94.1 (29ea6fb6a 2026-03-24)
Godot 4.7.1.stable.official
```

Linux and macOS work too; where a command differs, both forms are given.

Every PowerShell block below is PowerShell, not `cmd.exe`. Cmdlets such as
`New-Item`, `Copy-Item` and the call operator `&` do not exist in `cmd.exe`; if
your prompt reads `C:\...>` instead of `PS C:\...>`, run `powershell` first.

Windows PowerShell 5.1, the version that ships with Windows, is enough for
everything here. PowerShell 7 (`pwsh`) is never required, and no command in this
document assumes it.

---

## 1. Requirements

### 1.1 Mandatory

| Component | Version | Why it is required | Where |
| --- | --- | --- | --- |
| Rust toolchain | **1.94.1** (pinned) | Simulation and GDExtension library. `godot` 0.5 sets the floor at 1.94.0 | [rustup.rs](https://rustup.rs) |
| MSVC Build Tools | 2022, workload *Desktop development with C++* | Rust on Windows uses `link.exe`. Without it, every build fails at link time | [Visual Studio downloads](https://visualstudio.microsoft.com/downloads/) |
| Godot | **4.6 or newer**, standard build (not .NET) | Runs the GDExtension. `godot` 0.5.3 compiles against the Godot 4.6 API, so 4.6 is a hard floor, not a preference. The .NET build is unnecessary here and only adds weight | [godotengine.org/download](https://godotengine.org/download) |
| Python | 3.11+ | Content validator in CI and locally | [python.org](https://www.python.org/downloads/) |
| Git | any recent | Version control | [git-scm.com](https://git-scm.com/downloads) |
| Git LFS | any recent | `.gitattributes` routes binary assets to LFS. **Install before your first asset commit** | [git-lfs.com](https://git-lfs.com) |

The toolchain version is not a suggestion: [`rust-toolchain.toml`](../rust-toolchain.toml)
pins it, and rustup will install and use 1.94.1 automatically for any cargo
command run inside the repository.

Godot is **not** installed into this repository. It is a self-contained external
editor; extract it anywhere (`C:\Tools\Godot` is a reasonable choice) and never
commit the executable. The only things that enter the project tree are the built
library and the synced content, both gitignored.

### 1.2 Optional but recommended

| Tool | Install | What it buys you |
| --- | --- | --- |
| `cargo-watch` | `cargo install cargo-watch --locked` | Recompile on save; pair it with Godot's GDExtension hot reload |
| `cargo-deny` | `cargo install cargo-deny --locked` | Run the CI supply-chain audit locally before pushing |
| `typos-cli` | `cargo install typos-cli --locked` | Same spellcheck CI runs |
| `cargo-edit` | `cargo install cargo-edit --locked` | `cargo set-version` for release bumps. It rewrites `src/Cargo.toml` and `Cargo.lock` together, which hand-editing does not |
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

### 2.1 Making `godot` callable

Optional, but every command below reads better for it:

```powershell
if (-not (Test-Path $PROFILE)) { New-Item -ItemType File -Path $PROFILE -Force | Out-Null }
Add-Content $PROFILE 'Set-Alias godot "C:\Tools\Godot\Godot_v4.7.1-stable_win64.exe"'
. $PROFILE
godot --version
```

Adjust the path to your own extraction directory. If the alias reports
`is not recognized`, the path is wrong -- most often because the archive was
extracted into a subdirectory named after the archive itself. Verify with:

```powershell
Get-ChildItem -Recurse C:\Tools\Godot\*.exe
```

The `..._console.exe` variant beside it is the one to use from a terminal: it
keeps `print()` output and errors in the console instead of discarding them.

Run `Add-Content` once. Appending the same alias on every session leaves
duplicate lines in `$PROFILE`; they are harmless but make the file lie about
what it configures.

---

## 3. Building the GDExtension

Cargo commands run from `src\`. The copy and sync commands run from the
**repository root**, because their paths start with `src\`. Mixing the two is
the most common way to lose ten minutes here, so the directory change is its own
step rather than a comment.

```powershell
# 1. Build the cdylib (from src\).
cd src
cargo build -p rpg-bridge

# 2. Back to the repository root before anything that says src\.
cd ..

# 3. Copy the library where rpg.gdextension expects it.
New-Item -ItemType Directory -Force -Path src\game\bin | Out-Null
Copy-Item src\target\debug\rpg_bridge.dll src\game\bin\ -Force

# 4. Copy content into res:// (Godot cannot read outside the project directory).
.\tools\sync_data.ps1
```

If step 3 reports
`Cannot find path '...\src\src\target\debug\rpg_bridge.dll'`, you skipped step 2:
the shell is still inside `src\`, so `src\target\...` resolved one level too
deep. `cd ..` and retry; nothing is broken.

The sync script targets Windows PowerShell 5.1, so `pwsh` is not needed. If your
execution policy blocks it, run
`powershell -ExecutionPolicy Bypass -File tools\sync_data.ps1`, which affects
that one process only and changes no machine-wide setting.

The copy step is not optional bookkeeping. Godot loads the library from
`res://bin/`, never from `target/`, so skipping it means testing the previous
build and wondering why a fix had no effect.

Linux/macOS equivalent:

```sh
cd src && cargo build -p rpg-bridge && cd ..
mkdir -p src/game/bin
cp src/target/debug/librpg_bridge.* src/game/bin/
sh tools/sync_data.sh
```

Then open the project and run the main scene:

```powershell
godot --path src\game --editor          # editor
& "C:\Tools\Godot\Godot_v4.7.1-stable_win64_console.exe" --path src\game   # run headless-ish
```

A correct load prints, before any scene runs:

```text
Initialize godot-rust (API v4.6.stable.official, runtime v4.7.1.stable.official, safeguards strict)
```

`src/game/bin/` and `src/game/data/` are gitignored: they are build output, not
source.

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

One CI job cannot be reproduced locally without extra tooling: the Markdown link
check. It resolves every URL in every `.md` file over the network, so it fails
for reasons that have nothing to do with your change -- a slow academic host, a
rate-limited CDN, or a link to a Git tag that does not exist yet. Exclusions live
in [`.lycheeignore`](../.lycheeignore), one regex per line, each with the reason
it is there. Add to that list only when the failure is genuinely outside the
repository's control.

---

## 5. Troubleshooting

Every row below is a failure that actually occurred, not a hypothetical.

| Symptom | Cause | Fix |
| --- | --- | --- |
| `error: linker 'link.exe' not found` | MSVC Build Tools missing | Install VS Build Tools 2022 with *Desktop development with C++*, then reopen the terminal |
| `& was unexpected at this time.` | You are in `cmd.exe`, where `&` is not the call operator | Run `powershell`, then retry. Every block in this document is PowerShell |
| `The term 'pwsh' is not recognized` | You have Windows PowerShell 5.1, not PowerShell 7 | Run the script directly: `.\tools\sync_data.ps1`. Nothing in this project needs `pwsh` |
| `Copy-Item : Cannot find path '...\src\src\target\...'` | The command was run from inside `src\`, so `src\target\` resolved to `src\src\target\` | `cd ..` to the repository root and repeat the command |
| `where` returns a parameter-binding error | In PowerShell, `where` is an alias for `Where-Object` | Use `where.exe /r C:\ Godot*.exe`, or `Get-ChildItem -Recurse -Filter` |
| Alias `godot` reports the full path as unrecognized | The archive extracted into a directory named after the archive, so the path points at a folder | `Get-ChildItem -Recurse C:\Tools\Godot\*.exe`, flatten the directory, then `. $PROFILE` |
| Godot: `Can't open dynamic library` | Library not in `src/game/bin/`, or a 32-bit/64-bit mismatch | Rebuild and copy again; confirm the path in `src/game/rpg.gdextension` |
| Godot: `Class 'BattleSession' not found` | Extension not loaded | Check `entry_symbol = "gdext_rust_init"`, that `rpg.gdextension` sits beside `project.godot`, and reopen the project |
| Godot: extension built for a newer API | `compatibility_minimum` above your Godot build | Upgrade Godot to 4.6+, or pin a lower `api-*` feature in `src/bridge/Cargo.toml` and lower `compatibility_minimum` to match. API version must be <= runtime version ([compatibility docs](https://godot-rust.github.io/book/toolchain/compatibility.html)) |
| `GString: From<String> is not satisfied` | godot-rust takes `&str` or `&String`, never an owned `String` | Borrow it: `GString::from(&s)`. `src/bridge/src/lib.rs` funnels this through `json_to_gstring` |
| `invalid type: floating point \`95.0\`, expected i32` | Godot's JSON has no integer type, so content round-tripped through GDScript loses its integer-ness | Already handled by `rpg_core::json_compat` at the boundary. If it reappears, the library in `res://bin/` is stale -- rebuild and copy |
| Event log prints `"actor":0.0` while Rust emitted `0` | Same cause, opposite direction: `_animate` re-stringifies a parsed payload | Cosmetic only. Format with `%d` or `int()` when the real UI displays numbers |
| A fix appears to do nothing | The old `.dll` is still in `res://bin/` | `Copy-Item src\target\debug\rpg_bridge.dll src\game\bin\ -Force` from the repository root, then restart Godot |
| `missing content file: res://data/...` | Sync script not run | `.\tools\sync_data.ps1` from the repository root |
| Assets appear as small text files | Git LFS not installed before cloning | `git lfs install` then `git lfs pull` |
| CI complains about line endings | Committed CRLF | `.gitattributes` normalises to LF; re-add the file, do not disable core.autocrlf globally |
| CI `markdown links` fails on a URL that works in your browser | lychee has no JavaScript engine and no browser user agent, and unpushed tags genuinely 404 | Read the job log for the exact URL and status, then decide: fix the link, or add a justified regex to `.lycheeignore` |
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
