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
| M3 content pass | `0.4.0` | Full roster, statuses, elemental resistances | Released |
| Scorer and schema | `0.5.0` | One unit in `score_action`; the loader refuses unknown keys | Next |
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

Automation refuses to publish if steps 2 and 3 are skipped, or if the tag from
step 7 is not on `main`, by design. The commands are for `cmd.exe` and written
for `0.4.1`; substitute the version you are actually cutting.

Neither `development` nor `main` accepts a direct push (section 4), so steps 2
to 5 happen on a branch of their own. Create it from the remote `development`,
so that a local copy which is behind cannot leak into the release:

```bat
git fetch origin
git switch -c chore/release-0.4.1 origin/development
```

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

   ```bat
   cd src
   cargo set-version 0.4.1
   cargo test -p rpg-core --all-targets
   cd ..
   ```

   `cargo set-version` comes from cargo-edit (`cargo install cargo-edit`); the
   test run also refreshes `Cargo.lock`, which is committed. `cmd.exe` has no
   trailing `#` comments -- it passes them to the command as arguments -- so
   the block carries none.

   The workflow compares the tag against `src/Cargo.toml` and aborts on a
   mismatch. That check exists because a tag that disagrees with the manifest is
   the single most common release mistake.

   Use `cargo set-version`, not an editor. The version is declared once under
   `[workspace.package]` and inherited by `rpg-core` and `rpg-bridge`, and the
   tool rewrites `Cargo.lock` in the same pass. Hand-editing the manifest leaves
   the lockfile stale, and the mismatch surfaces later as a confusing diff.

4. **Commit.**

   ```bat
   git add src/Cargo.toml src/Cargo.lock CHANGELOG.md
   git commit -m "chore(release): 0.4.1"
   ```

5. **Merge `main` back into the branch.**

   ```bat
   git merge --no-ff origin/main -m "chore(release): bring main's release merges back into development"
   git diff --stat HEAD~1 HEAD
   ```

   The second command must print nothing. `main` only ever receives
   `development`, so this merge changes no file; it adds history. Each release
   reaches `main` as a merge commit that `development` never gets, so without
   this step `development` falls one more commit behind `main` with every
   release -- three by `0.4.0`. A pull request whose head is behind its base
   can be held by `protect-main`'s up-to-date rule, and *Update branch* would
   then try to push to `development`, which only takes pull requests.

   If the diff is not empty, `main` holds a change `development` does not.
   Find out why before going further.

6. **Push, then merge twice.**

   ```bat
   git push -u origin chore/release-0.4.1
   ```

   Open a pull request from `chore/release-0.4.1` into `development` and merge
   it once `ci` is green. Then open a pull request from `development` into
   `main`. GitHub proposes `development` as the base, because it is the
   default branch, so pick `main` by hand. That pull request needs `ci` and
   `main accepts development only` green before it can merge.

7. **Tag `main` and push the tag.**

   ```bat
   git fetch origin
   git log -1 --oneline origin/main
   git tag -a v0.4.1 origin/main -m "v0.4.1 - M3 patch"
   git push origin v0.4.1
   ```

   `git log` should show the merge of the pull request from `development`. Tag
   `origin/main` directly rather than checking `main` out: a local `main` from
   before `0.4.0` still tracks `src/game/data/`, and checking it out overwrites
   the synced content with that stale copy. The tag is what the archives are
   built from, so it has to point at the commit that passed the protected
   branch's checks, and the release workflow refuses a tag whose commit is not
   on `main`.

8. **Verify the published release.** Confirm three archives (Windows, Linux,
   macOS), a `.sha256` beside each, and release notes matching the changelog
   section. To check a download on Windows, run
   `certutil -hashfile <archive> SHA256` and compare the hash it prints with
   the one in the archive's `.sha256` file.

9. **Clean up.**

   ```bat
   git switch development
   git pull
   git branch -d chore/release-0.4.1
   git push origin --delete chore/release-0.4.1
   git fetch origin main:main
   ```

   The last line moves the local `main` up to the release without checking it
   out, so a later `git switch main` cannot restore files `main` no longer
   tracks. Then close the milestone in *Issues → Milestones* and create the
   next one.

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

A tag the release workflow refused -- not on `main`, or not matching the
manifest -- published nothing, so it can be deleted and cut again at the right
commit:

```bat
git push origin --delete refs/tags/v0.4.1
git tag -d v0.4.1
```

---

## 3. What a release contains

Built by [`.github/workflows/release.yml`](../.github/workflows/release.yml) on
every `v*.*.*` tag:

```text
jojo-rpg-v0.4.1-x86_64-pc-windows-msvc.zip
├── bin/rpg_bridge.dll        # release build, LTO enabled
├── data/*.json               # the exact content the build was tested against
├── rpg.gdextension           # descriptor, so the library is usable immediately
├── LICENSE
├── README.md
└── CHANGELOG.md
```

Plus `librpg_bridge.so` (Linux, `x86_64-unknown-linux-gnu`) and
`librpg_bridge.dylib` (macOS, `aarch64-apple-darwin`) archives, each with a
SHA-256 checksum file beside it. There is no Intel macOS build. The macOS
library is not signed with a Developer ID or notarised, so macOS may refuse a
copy downloaded through a browser until its quarantine flag is cleared with
`xattr -dr com.apple.quarantine bin`; the release notes say so.

An archive is a library and its content, not a game. Its `bin/` and `data/` go
into `src/game/` of a checkout of the same tag, and its descriptor is the same
file as `src/game/rpg.gdextension`. Shipping something a non-developer can
double-click needs a headless Godot export, and that is M4 work scheduled for
`1.0.0`.

The release job re-runs `cargo test -p rpg-core --release` before packaging.
Release-profile builds enable LTO, which changes optimisation; testing only debug
builds would mean shipping an untested configuration.

**Not published to crates.io.** `rpg-core` is a game's internal crate; publishing
it would create a support obligation with no user. `publish = false` makes that
explicit rather than accidental.

---

## 4. Repository settings that make this work

These are manual, one-time, and cannot be committed to the repo:

1. **Rulesets** (*Settings → Rules → Rulesets*), both targeting a branch by
   name rather than "the default branch", so changing the default branch moves
   neither:
   - `protect-main` on `refs/heads/main`: restrict deletions, block force
     pushes, require a pull request, require the status checks `ci` and
     `main accepts development only`, the latter reported by
     [`.github/workflows/pr-guard.yml`](../.github/workflows/pr-guard.yml),
     because a ruleset cannot restrict which branch a pull request comes from.
     Requiring branches to be up to date is on, which is why step 5 of
     section 2 merges `main` back before the release pull request.
   - `protect-development` on `refs/heads/development`: the same, minus the
     pull-request source check, and with up-to-date branches **off**, so a
     topic branch does not need rebasing every time another one merges.
   - *Default branch*: `development`, so new pull requests target it, a fresh
     clone starts on it and Dependabot reads its configuration from it.
2. **Actions permissions:** allow `GITHUB_TOKEN` to write contents for releases
   (the release workflow requests `contents: write` at the job level; the
   repository setting must permit it).
3. **Dependabot:** enabled via [`.github/dependabot.yml`](../.github/dependabot.yml);
   confirm alerts are on in *Settings → Code security*. GitHub reads that file
   from the **default branch** only, and opens security updates against the
   default branch whatever `target-branch` says. While `main` is the default,
   Dependabot therefore proposes changes to `main` and bypasses `development`
   (PRs #48 and #52). Setting *Settings → General → Default branch* to
   `development` fixes both; it also makes `development` the base a new pull
   request proposes by default. Branch protection on `main`, tags and releases
   are unaffected.
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
