# Versioning and Releases

A release exists so that a stranger, six months from now, can download something
that runs and know exactly what is in it. That is the whole purpose; everything
below serves it.

---

## 1. Versioning policy

The project follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html),
with the pre-1.0 reading spelled out so it is not ambiguous:

| Version | Meaning here |
| --- | --- |
| `0.MINOR.PATCH` | Pre-1.0. The public Rust API and the JSON schemas may change in any minor bump |
| MINOR | A milestone is complete, or a JSON schema or FFI method signature changed |
| PATCH | Bug fixes, balance numbers, docs, CI; no schema or API change |
| `1.0.0` | Reserved for a playable vertical slice with a stable content schema |
| `-rc.N` suffix | Published as a GitHub pre-release automatically |

Milestone mapping, from [ROADMAP.md](ROADMAP.md):

| Milestone | Version | Fixation criterion |
| --- | --- | --- |
| M0 scaffold + core | `0.1.0` | Simulation tested and deterministic |
| M1 vertical slice | `0.2.0` | One battle playable start to finish in Godot |
| M2 balance harness | `0.3.0` | Headless mass simulation reporting win rates |
| M3 content pass | `0.4.0` | Full roster, statuses, elemental resistances |
| M4 polish | `1.0.0` | Exportable build with audio, art and UX |

**One tag per fixation.** If work is not worth a changelog entry, it is not worth
a tag.

---

## 2. Release checklist

Automation refuses to publish if steps 2 and 3 are skipped, by design.

1. **CI green on `development`.** No exceptions, including the optional-looking
   jobs.
2. **Update `CHANGELOG.md`.** Move items out of `Unreleased` into a
   `## [X.Y.Z] - YYYY-MM-DD` section. The release workflow greps for this exact
   heading and fails without it.
3. **Bump the workspace version.**

   ```powershell
   cd src
   cargo set-version 0.2.0    # requires cargo-edit
   cargo check --workspace    # refreshes Cargo.lock, which is committed
   cd ..
   ```

   The workflow compares the tag against `src/Cargo.toml` and aborts on a
   mismatch. That check exists because a tag that disagrees with the manifest is
   the single most common release mistake.

4. **Commit and merge.**

   ```powershell
   git commit -am "chore(release): v0.2.0"
   git push origin development
   # open a PR into main, let CI pass, merge
   ```

5. **Tag from `main` and push the tag.**

   ```powershell
   git checkout main
   git pull
   git tag -a v0.2.0 -m "v0.2.0 - vertical slice"
   git push origin v0.2.0
   ```

6. **Verify the published release.** Confirm three archives (Windows, Linux,
   macOS), a `.sha256` beside each, and release notes matching the changelog
   section.

### If it goes wrong

Do not move a published tag. Delete the release and the tag, fix forward, and
tag the next patch version. A rewritten tag breaks every checksum anyone already
downloaded.

---

## 3. What a release contains

Built by [`.github/workflows/release.yml`](../.github/workflows/release.yml) on
every `v*.*.*` tag:

```text
jojo-rpg-v0.2.0-x86_64-pc-windows-msvc.zip
├── bin/rpg_bridge.dll        # release build, LTO enabled
├── data/*.json               # the exact content the build was tested against
├── rpg.gdextension           # descriptor, so the library is usable immediately
├── LICENSE
├── README.md
└── CHANGELOG.md
```

Plus `librpg_bridge.so` (Linux, `x86_64-unknown-linux-gnu`) and
`librpg_bridge.dylib` (macOS, `aarch64-apple-darwin`) archives, each with a
SHA-256 checksum file.

The release job re-runs `cargo test -p rpg-core --release` before packaging.
Release-profile builds enable LTO, which changes optimisation; testing only debug
builds would mean shipping an untested configuration.

**Not published to crates.io.** `rpg-core` is a game's internal crate; publishing
it would create a support obligation with no user. `publish = false` makes that
explicit rather than accidental.

---

## 4. Repository settings that make this work

These are manual, one-time, and cannot be committed to the repo:

1. **Branch protection on `main`:** require the status check named `ci` (the
   aggregate job), require a PR, disallow force pushes.
2. **Actions permissions:** allow `GITHUB_TOKEN` to write contents for releases
   (the release workflow requests `contents: write` at the job level; the
   repository setting must permit it).
3. **Dependabot:** enabled via [`.github/dependabot.yml`](../.github/dependabot.yml);
   confirm alerts are on in *Settings → Code security*.

---

## 5. Commit conventions

[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):
`feat`, `fix`, `docs`, `chore`, `ci`, `refactor`, `test`, `perf`. Scope with the
layer, for example `feat(core):`, `fix(bridge):`, `docs(readme):`.

This is not decoration: it is what makes `git log --oneline` readable to a
reviewer who has never seen the project, and it lets changelog sections be
assembled without archaeology.
