# Security policy

This is a hobby project kept by one person. It has no security team, no
bounty and no service level. What it does have is a private channel for
reports and a promise to read them.

## Supported versions

Only the latest released minor version receives security fixes. A fix lands
on `development`, ships in the next patch of that minor, and is not
backported to older minors.

| Version | Supported |
| --- | --- |
| The latest released minor | Yes |
| Any older minor | No |
| Unreleased work on `development` | Fixed on the branch, not as a release |

The project is pre-1.0 (`0.x`), and `0.x` has no long-term support: once a
new minor is tagged, the previous one is no longer patched. The versioning
rules are in [docs/RELEASING.md](docs/RELEASING.md#1-versioning-policy), and
every release is listed on the
[releases page](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases).

## How to report

Use GitHub's private vulnerability reporting:

1. Open the repository's **Security** tab.
2. Choose **Report a vulnerability**.
3. Fill in the form. Only the maintainer can read it.

GitHub's own guide is
[Privately reporting a security vulnerability](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability).

There is no security email address. Please do not open a public issue, a
discussion or a pull request for a vulnerability before it is fixed.

A useful report says:

- the version: a release tag or a commit SHA;
- the component (see the scope below);
- how to reproduce it: the exact commands, and the content file, seed or
  archive involved. The simulation is deterministic, so a seed and a command
  sequence reproduce a battle exactly;
- what an attacker gains, as far as you can tell.

## Scope

In scope:

| Component | Where | Examples of what counts |
| --- | --- | --- |
| `rpg-core` | `src/core/` | A panic, an unbounded allocation or a hang reachable from content or a save file; arithmetic that overflows where the code assumes it cannot |
| The `rpg-bridge` FFI boundary | `src/bridge/` | Memory unsafety, or a value from GDScript that crosses the boundary unchecked |
| The content loader | `src/core/src/data.rs`, `data/*.json` | A crafted content file that the loader accepts and that then breaks an invariant, or that crashes the game instead of failing the load |
| CI and release workflows | `.github/workflows/` | A token with more permission than its job needs, a pull request from a fork that can run code with a write token, a third-party action or download taken without a pinned version or checksum |
| Release archives | The assets attached to a GitHub Release | An archive whose contents or `.sha256` file do not match what the tagged commit builds |

Out of scope, and better reported upstream:

- **The Godot engine.** Report it to the
  [Godot project](https://github.com/godotengine/godot) through its own
  security policy.
- **Upstream crates**, including `godot` (godot-rust) and `serde`. Report
  them to their maintainers; for godot-rust that is
  [godot-rust/gdext](https://github.com/godot-rust/gdext). A published
  advisory against a dependency is also tracked by Dependabot and
  `cargo deny`, and the [RustSec advisory database](https://rustsec.org/)
  is where Rust advisories are filed.
- Balance: an encounter that is too easy or too hard is a
  [balance finding](docs/TRIAGE.md), not a vulnerability.
- Anything that needs an attacker who already controls your machine, your
  Godot installation or your copy of the repository.

If a problem in this repository is caused by an upstream defect, report it
here as well, so the project can pin or work around it.

## What to expect

- An acknowledgement within about a week. It may be slower; this is a
  project kept in spare time.
- An assessment: whether it is in scope, and how serious it is.
- For a confirmed issue, a fix in the next patch release of the supported
  minor, with a GitHub security advisory and a `CHANGELOG.md` entry that
  credit you unless you ask not to be named.

There is no bounty and no payment of any kind.
