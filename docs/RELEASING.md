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

| Milestone | Version | Fixation criterion | State |
| --- | --- | --- | --- |
| M0 scaffold + core | `0.1.0` | Simulation tested and deterministic | Released |
| M1 vertical slice | `0.2.0` | One battle playable start to finish in Godot | Released |
| M2 balance harness | `0.3.0` | Headless mass simulation reporting win rates | Released |
| M3 content pass | `0.4.0` | Full roster, statuses, elemental resistances | In progress |
| M4 polish | `1.0.0` | Exportable build with audio, art and UX | Planned |

**One tag per fixation.** If work is not worth a changelog entry, it is not worth
a tag.

A milestone is fixed by its own exit criterion and nothing else. `0.2.0` shipped
a battle that was playable and badly balanced, because M1 was about playability.
`0.3.0` fixed the measuring instrument rather than the game: every encounter now
declares the win rate it should produce and CI checks the claim, which is what
made five of the six content changes in that release arguable at all. It shipped
with five of eleven skills unreachable by the AI, because that is M3's problem
and not a reason to hold a tag. Deferring a release until everything is good is
how projects end up with one release and no history.

---

## 2. Release checklist

Automation refuses to publish if steps 2 and 3 are skipped, by design. The
commands are written for the next release, `0.4.0`; substitute the version you
are actually cutting.

1. **CI green on `development`, and the local gate green on your machine.** No
   exceptions, including the optional-looking jobs. The two are not the same
   check: a green tick on `development` describes the last commit CI saw, so a
   documentation commit pushed afterwards is unverified until it runs. Run the
   full gate from
   [DEVELOPMENT.md](DEVELOPMENT.md#4-checks-to-run-before-pushing) before opening
   the release PR. `0.3.0` learned this the direct way -- its branch failed the
   `format` job because files authored through the GitHub API had never been near
   `cargo fmt`.
2. **Update `CHANGELOG.md`.** Move items out of `Unreleased` into a
   `## [X.Y.Z] - YYYY-MM-DD` section, and update the link references at the
   bottom of the file. The release workflow greps for this exact heading and
   fails without it.
3. **Bump the workspace version.**

   ```powershell
   cd src
   cargo set-version 0.4.0    # requires cargo-edit
   cargo test -p rpg-core --all-targets   # also refreshes Cargo.lock, which is committed
   cd ..
   ```

   The workflow compares the tag against `src/Cargo.toml` and aborts on a
   mismatch. That check exists because a tag that disagrees with the manifest is
   the single most common release mistake.

   Use `cargo set-version`, not an editor. The version is declared once under
   `[workspace.package]` and inherited by `rpg-core` and `rpg-bridge`, and the
   tool rewrites `Cargo.lock` in the same pass. Hand-editing the manifest leaves
   the lockfile stale, and the mismatch surfaces later as a confusing diff.

4. **Commit and merge.**

   ```powershell
   git add src/Cargo.toml src/Cargo.lock
   git commit -m "chore(release): 0.4.0"
   git push origin development
   # open a PR into main, let CI pass, merge
   ```

5. **Tag from `main` and push the tag.**

   ```powershell
   git checkout main
   git pull origin main
   git tag -a v0.4.0 -m "v0.4.0 - content pass"
   git push origin v0.4.0
   git checkout development
   ```

   Tag `main`, never `development`. The tag is what the release archives are
   built from, so it has to point at the commit that passed the protected
   branch's checks.

6. **Verify the published release.** Confirm three archives (Windows, Linux,
   macOS), a `.sha256` beside each, and release notes matching the changelog
   section.

### Two failures that are not yours

Both appeared while cutting `0.3.0` and both cleared on an immediate retry. They
are network calls, not defects, and neither should be worked around with a flag:

- `failed to fetch advisory database ... schannel: server closed abruptly` from
  `cargo deny check`. The advisory database is cloned from GitHub on every run.
  Re-run the command. Do not pass `--offline`, which would silently audit
  yesterday's advisory list.
- `Post ".../info/lfs/locks/verify": i/o timeout` when pushing a tag. Git LFS
  probes a locking API this repository does not use. Re-run `git push`. Disabling
  `lfs.locksverify` is a real option, but it hides a real feature to silence one
  timeout.

### Why the changelog compare links are excluded from the link check

Keep a Changelog wants the heading of the version being prepared to link to
`compare/<previous>...<this one>`, and `[Unreleased]` to point at
`compare/<newest tag>...HEAD`. Both URLs 404 until the tag is pushed, which is
precisely the window a release PR lives in, so the `markdown links` job would
fail on every release by construction. `.lycheeignore` therefore excludes this
repository's own `/compare/` ranges and **only** those; `/releases/tag/` links
stay checked, so a wrong tag number is still caught, as is the tag/manifest
comparison in the release workflow.

### If it goes wrong

Do not move a published tag. Delete the release and the tag, fix forward, and
tag the next patch version. A rewritten tag breaks every checksum anyone already
downloaded.

---

## 3. What a release contains

Built by [`.github/workflows/release.yml`](../.github/workflows/release.yml) on
every `v*.*.*` tag:

```text
jojo-rpg-v0.4.0-x86_64-pc-windows-msvc.zip
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

An archive is a library and its content, not a game. Shipping something a
non-developer can double-click needs a headless Godot export, and that is M4
work scheduled for `1.0.0`.

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
4. **Issue labels:** the taxonomy in [TRIAGE.md](TRIAGE.md) has to exist in
   *Issues → Labels* before the issue forms can apply it.

---

## 5. Commit conventions

[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):
`feat`, `fix`, `docs`, `chore`, `ci`, `refactor`, `test`, `perf`. Scope with the
layer, for example `feat(core):`, `fix(bridge):`, `docs(readme):`.

This is not decoration: it is what makes `git log --oneline` readable to a
reviewer who has never seen the project, and it lets changelog sections be
assembled without archaeology.
