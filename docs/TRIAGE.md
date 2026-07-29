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

Create these once in *Issues → Labels*, or run
[`tools/setup_labels.ps1`](../tools/setup_labels.ps1) (§7); the issue forms
apply them automatically.

| Label | Colour | Use |
| --- | --- | --- |
| `type: bug` | `d73a4a` | Behaviour contradicts documented intent |
| `type: balance` | `0e8a16` | Measured win rate, or an encounter that is not the fight it claims to be |
| `type: task` | `1d76db` | Planned milestone work |
| `type: docs` | `0075ca` | Documentation only |
| `area: core` | `5319e7` | `rpg-core`: rules, scheduler, resolution, AI, RNG |
| `area: bridge` | `8250df` | `rpg-bridge`: the FFI boundary |
| `area: godot` | `478cbf` | `src/game/`: scenes, GDScript, presentation |
| `area: content` | `006b75` | `data/*.json` |
| `area: ci` | `444444` | Workflows, gates, tooling |
| `sev: blocker` | `b60205` | `main` is broken, or a release cannot be cut |
| `sev: major` | `d93f0b` | A feature is unusable; no acceptable workaround |
| `sev: minor` | `fbca04` | Wrong but survivable |
| `needs: repro` | `e4e669` | Cannot be acted on until a seed is supplied |
| `wontfix` | `ffffff` | Closed deliberately, with the reason written in the issue |

Severity is about consequence, not annoyance. A cosmetic label that says
`Defeat` on a win is `sev: major`, because the player cannot tell what happened.

Every issue should end triage with **one `type:`, one `area:`, and — for a
defect — one `sev:`**. Anything else is a queue that cannot be filtered.

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

---

## 7. Creating the labels

Fourteen labels typed by hand is fourteen chances to mistype one, and a
mistyped label fails silently: the form still opens the issue, the label is
simply absent, and nothing anywhere reports it. Prefer the script:

```powershell
cd C:\Users\Jeremia\Yet-to-be-JoJo-s-RPG-Game-
.\tools\setup_labels.ps1 -WhatIf   # prints what it would do, changes nothing
.\tools\setup_labels.ps1           # applies the table in section 2
gh label list --repo ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-
```

It needs the GitHub CLI, once:

```powershell
winget install --id GitHub.cli
# restart the shell so gh lands on PATH, then:
gh auth login
```

The script is idempotent -- it uses `gh label create --force`, so running it
again repairs a label whose colour or description was edited by hand, and adds
nothing twice. It never deletes anything, so GitHub's seven default labels
(`bug`, `enhancement`, `question`, and so on) survive. **Delete those by hand.**
Leaving them means two labels mean "bug", which is how a triage query starts
missing issues.

If you would rather not install the CLI, *Issues → Labels → New label* takes
the three columns of the section 2 table directly. It is the same result, more
slowly.

---

## 8. How the issue forms actually behave

Four things about `.github/ISSUE_TEMPLATE/` are not obvious, and three of them
look like bugs the first time you meet them.

**They only take effect from the default branch.** The forms live on
`development` right now, so *New issue* will still show a blank box until
`0.4.0` is merged into `main`. Nothing is broken in the meantime; the feature
simply is not live yet. The same applies to `pull_request_template.md`.

**A form is a questionnaire, not a wrapper.** Each `id` in the YAML becomes a
`### heading` in the issue body, with the answer underneath. That is why the
fields are worth arguing about: a field that exists gets answered, and a field
that does not exist gets left out, every time. `validations: required: true`
means GitHub refuses to open the issue until the box is filled — which is the
entire mechanism forcing a seed into every defect report.

**Labels in the `labels:` list are applied on submission.** `bug_report.yml`
applies `type: bug` **and** `needs: repro`, deliberately: a report starts
unproven, and you remove `needs: repro` once you have reproduced it yourself.
The area and severity are not in the list, because only triage can decide them
honestly — the reporter says where they noticed the problem, which is often not
where it lives.

**`config.yml` disables blank issues.** Every report therefore arrives in one of
the three shapes above. The contact links route setup problems to
`DEVELOPMENT.md` and open questions to Discussions, so the tracker stays a list
of things that can be closed.

To change a form, edit the YAML and open a PR like any other change. The forms
are not link-checked (lychee reads Markdown only) and not spellchecked as prose,
so a broken relative path inside a form will not fail CI — check those by hand.

---

## 9. The `0.4.0` backlog

The milestone is eleven issues. Create a GitHub milestone named `0.4.0` first,
then file them in this order and assign each to it. The order is not
preference: **steps 1 to 3 change the rules, which voids every win rate measured
before them**, so anything that measures must come after them, and anything that
authors content must come after the measurement is trustworthy again.

| # | Title | Form | Labels beyond `type:` | Depends on |
| --- | --- | --- | --- | --- |
| 1 | `task: report miss% per action, not per target` | Task | `area: core` | — |
| 2 | `task: make every skill reachable by the AI` | Task | `area: core` | 1 |
| 3 | `task: apply elemental resistance in damage resolution` | Task | `area: core` | 2 |
| 4 | `task: give Event::Healed an actor and report healing done` | Task | `area: core` | 3 |
| 5 | `task: re-measure all three bands after the rules changes` | Task | `area: content` | 3, 4 |
| 6 | `task: widen the seed list to 24 in an isolated commit` | Task | `area: content` | 5 |
| 7 | `task: re-sweep the response curve under the new rules` | Task | `area: content` | 6 |
| 8 | `task: third playable character, authored in data/ only` | Task | `area: content` | 7 |
| 9 | `task: three more enemies, each in a declared encounter` | Task | `area: content` | 7 |
| 10 | `task: second boss fight with its own band` | Task | `area: content` | 8, 9 |
| 11 | `task: floating damage numbers, status icons, legible tempo readout` | Task | `area: godot` | — |

Why item 1 comes before the work it seems unrelated to: `miss%` currently counts
one miss per target and one action per skill, so the moment area attacks become
reachable in item 2, every miss rate in every report becomes wrong. Fixing the
accounting after the fact means re-reading a milestone's worth of reports that
quietly lied.

Item 11 is independent of all of it. It touches no rule and no number, so it can
be worked at any point, and it is the only item on the list that CI cannot
verify — it closes on a recorded playthrough with a clean console, the way M1
did.

### What each of these owes before it closes

- **Items 1 to 4** owe a unit test that fails without the change. A rules change
  with no test is a rules change nobody can defend in six months.
- **Items 5 to 10** owe a [BALANCE-LOG.md](BALANCE-LOG.md) entry with the
  prediction written **before** the run, and a passing `balance_bounds`. If a
  band has to move, the entry argues why the old band was wrong — not why the
  new number is inconvenient.
- **Item 11** owes a playthrough: the console output, and what you saw.

### Issues that will arrive on their own

The risk register in [ROADMAP.md](ROADMAP.md) lists what is expected to break,
each with the symptom it will produce. Two are near-certain and should be filed
as they happen rather than pre-emptively:

- `balance_bounds` failing on encounters nobody touched, right after items 2
  and 3. That is `type: balance`, not `type: bug`; the gate is correct and the
  content is stale.
- `BALANCE-CURVE.md` ceasing to reproduce. Same class. Mark the old table as
  describing pre-`0.4.0` rules rather than deleting it — a retracted
  measurement is still evidence, and this project has already retracted one.
