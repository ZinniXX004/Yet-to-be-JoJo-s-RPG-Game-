# Issue and defect handling

This project simulates battles deterministically. A seed plus a command sequence
reproduces a fight exactly, on any machine, forever. That has one consequence
that governs everything below: **an unreproducible bug report here is a report
that was written badly, not a hard problem.** The reproduction standard is
therefore strict, because meeting it is cheap.

---

## 1. Three classes of report

| Class | Means | Goes to |
| --- | --- | --- |
| **Defect** | The code does something other than what it says it does | An issue, `type: bug` |
| **Balance finding** | The code is correct and the numbers are wrong | An issue, `type: balance`, and eventually an entry in [BALANCE-LOG.md](BALANCE-LOG.md) |
| **Task** | Planned work from [ROADMAP.md](ROADMAP.md) | An issue, `type: task`, linked to its milestone |

The distinction matters more than it looks. "The boss is too easy" is not a
defect and must never be fixed by editing code; "the boss takes damage from its
own ally" is a defect and must never be fixed by editing `data/`. Filing one as
the other is the fastest way to a wrong fix.

---

## 2. Labels

Create these once in *Issues → Labels*; the issue forms apply them
automatically.

| Label | Use |
| --- | --- |
| `type: bug` | Behaviour contradicts documented intent |
| `type: balance` | Measured win rate, or an encounter that is not the fight it claims to be |
| `type: task` | Planned milestone work |
| `type: docs` | Documentation only |
| `area: core` | `rpg-core`: rules, scheduler, resolution, AI, RNG |
| `area: bridge` | `rpg-bridge`: the FFI boundary |
| `area: godot` | `src/game/`: scenes, GDScript, presentation |
| `area: content` | `data/*.json` |
| `area: ci` | Workflows, gates, tooling |
| `sev: blocker` | `main` is broken, or a release cannot be cut |
| `sev: major` | A feature is unusable; no acceptable workaround |
| `sev: minor` | Wrong but survivable |
| `needs: repro` | Cannot be acted on until a seed is supplied |
| `wontfix` | Closed deliberately, with the reason written in the issue |

Severity is about consequence, not annoyance. A cosmetic label that says
`Defeat` on a win is `sev: major`, because the player cannot tell what happened.

---

## 3. What a defect report must contain

A report without the first three items gets `needs: repro` and nothing else:

1. **The seed**, and the encounter or matchup id.
2. **The command sequence**, or a statement that the battle ran unattended.
3. **The version**: a tag such as `v0.3.0`, or a commit SHA.
4. Expected behaviour, quoting the document or doc-comment that states it.
5. Observed behaviour, pasted verbatim -- console output, panic message, or the
   relevant lines of the event log. Not a description of the output.

The fastest reproduction for anything below the UI is the harness itself:

```powershell
cd src
cargo run -q -p rpg-core --bin balance -- --only matchup.dio_boss --json --no-fail
```

If the harness reproduces it, the Godot layer is not involved and
`area: godot` is the wrong label.

---

## 4. What a balance finding must contain

A balance claim is a measurement or it is an opinion:

1. The `balance` report, pasted -- not summarised.
2. Which declared band in `data/matchups.json` the number contradicts.
3. The commit the numbers were produced at.

And one warning that has already caught this project out: **a rules change resets
the series.** RNG draw order is part of the rules, so after any edit to `ai.rs`,
`resolve.rs` or `battle.rs`, the same seed no longer produces the same battle.
Numbers measured across that boundary are not comparable, and comparing them
anyway produces a confident, wrong conclusion. Say which side of the boundary
your numbers come from.

Remember what the harness is and is not. It plays worse than a competent player,
and twelve seeds resolve to 8.3 percentage points, so 67% and 75% are the same
reading. A finding that depends on a difference smaller than the resolution is
not a finding.

---

## 5. Triage rules

1. **Reproduce before you diagnose.** A theory formed before the seed runs is a
   guess wearing a lab coat.
2. **Label the area from where it reproduces**, not from where it was noticed. A
   wrong damage number noticed in Godot is `area: core` if the harness shows it
   too.
3. **A defect that reaches `main` gets a regression test in the same PR.** Every
   entry in the `### Fixed` sections of [CHANGELOG.md](../CHANGELOG.md) exists
   because something was not covered; a fix without a test invites the same bug
   back under a different name.
4. **A balance finding gets a log entry, with the prediction written before the
   run.** Seven of nine predictions in `0.3.0` were wrong. That number is only
   knowable because the predictions were written down first.
5. **Do not widen a band to make CI green.** The gate exists to contradict you.
   Widening it on the day it does its job converts the harness into decoration.
6. **Close with a reason.** `wontfix` is legitimate; a silent close is not.

---

## 6. Where a report is not the right tool

- **Tooling absent on your machine** -- `pwsh`, `typos`, `cargo deny` not found
  -- is a setup step, not a defect. See
  [DEVELOPMENT.md](DEVELOPMENT.md#5-troubleshooting), which lists every one of
  these that has actually happened here.
- **A transient network failure** in `cargo deny check`, `git push` or the
  `markdown links` job is not a defect either. Re-run it. If it recurs, it
  becomes `area: ci` with the job log attached.
- **A question about how something works** belongs in a discussion or a
  documentation issue. If the answer is not in `docs/`, that absence is the
  defect.
