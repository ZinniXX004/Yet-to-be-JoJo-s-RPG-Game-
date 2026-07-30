# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
as interpreted in [docs/RELEASING.md](docs/RELEASING.md).

Every released version has a git tag (`vX.Y.Z`) and a matching
[GitHub Release](https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases)
with prebuilt libraries. A section here without a tag is not a release.

## [Unreleased]

M3 in progress: process work, the accounting fix, the rules fix the whole
milestone was ordered around, and now the attribution fix. The AI no longer
picks the biggest affordable attack; it scores every option in one unit and pays
for SP at what that SP would have bought instead. A turn now returns a little SP
to whoever took it, which is the first time SP is a renewable resource in this
project. Elemental resistances now modify damage: the data schema already
carried the field; `resolve.rs` just never read it. And healing is finally
credited to whoever performed it, which turns the party's healer from a row of
zeroes into the second-largest contributor in the boss fight.

**Every win rate published in `0.3.0` is void.** Three of the four rules-level
changes below touch `ai.rs`, `battle.rs` or `state.rs`, so the same seed no
longer produces the same battle. The numbers are not worse or better than the
old ones; they are not comparable to them. The re-measured series is at the end
of this section.

**Healing attribution (#11) is not one of those changes.** It adds a field, a
variant and a column, and draws no RNG. Every figure measured after step 2
survived it digit for digit, which is the evidence that it is a reporting change
and nothing more.

### Added

- **[docs/TRIAGE.md](docs/TRIAGE.md)**: how issues and defects are handled. The
  label taxonomy, four severity levels, and the distinction that matters most
  here — a defect is fixed in code and a balance finding is fixed in `data/`,
  and filing one as the other is the fastest route to a wrong fix. Because the
  simulation is deterministic, a reproduction is a seed plus a command sequence,
  so the reproduction standard is strict: meeting it costs nothing.
- **Issue forms** in `.github/ISSUE_TEMPLATE/`: a defect report that requires
  the seed, the encounter id and the version; a balance finding that requires
  the pasted harness output, the band it contradicts, and a prediction written
  before the proposed change is run; and a task form that requires a checkable
  finish line. Blank issues are disabled, and setup problems are routed to
  `DEVELOPMENT.md` rather than to the tracker.
- **`.github/pull_request_template.md`**: the local gate as a checklist, plus
  two questions CI cannot ask — whether the change touches `ai.rs`,
  `resolve.rs`, `battle.rs` or `rng.rs` and therefore voids every previously
  measured win rate, and whether a new encounter declares a band. PR #6 went up
  without a template and both of those nearly slipped.
- **The M3 plan** in [docs/ROADMAP.md](docs/ROADMAP.md): exit criteria, ordered
  work items with the reason for the order, a planned-changes-by-file table
  written before the work so it can be scored afterwards the way M2's was, and
  a risk register naming each expected failure by the symptom it will produce.
- **`tools/setup_labels.ps1`**: creates or updates the fourteen labels the issue
  forms and the triage rules refer to, idempotently and with `-WhatIf`, because
  a form that applies a label the repository does not have silently applies
  nothing.
- **A `rolls` column in the harness table**, next to `miss%`. A percentage on
  its own cannot tell an accurate attacker from one that never attacked: both
  print 0%. It is also the diagnostic for #9 — a combatant whose rolls exceed
  its actions is using an area skill, which is exactly what the AI was believed
  never to do.
- **Per-skill action accounting** (`actions by skill` under every harness
  table). Win rates say who won; they cannot say what anyone did. Every action
  is now attributed to the skill that produced it, the shares are shares of that
  combatant's own turns, and the report raises an accounting defect if the
  attributed uses do not add up to the recorded actions. This shipped *before*
  the AI was changed, so the two runs on either side of the change are
  comparable action by action rather than only in outcome. It immediately
  disproved a prediction of its own: `skill.tempo_halt` was 8% of Jotaro's boss
  turns while being 71% of his SP, and a share of turns is not a share of SP.
- **SP recovery** (`state::SP_REGEN_PER_TURN`, `Combatant::recover_sp`). A
  combatant gets 4 SP back on its own turn, capped at the pool it started with.
  It is charged per turn rather than per tick, so the engine has exactly one
  notion of "a turn" — the same one status durations use — and a turn lost to
  stun still recovers, because the cost was paid in tempo either way. The amount
  is flat rather than a share of the pool, which is deliberately anti-boss: 4 is
  5% of Jotaro's 80 and 2% of Dio's 200.
- **`skill.emerald_splash`** (22 SP, all enemies, 105 psychic): Hierophant
  Green's own scatter attack, so Kakyoin has a second option that is his rather
  than borrowed.
- **Per-element resistance tables** on `CombatantDef` (#10). `data.rs` gains a
  `resist: HashMap<Element, i32>` field with `MAX_RESISTANCE = 100` and
  `MIN_RESISTANCE = -100`; `state::Combatant` mirrors it with `#[serde(default)]`
  and a `resistance(element)` accessor; `data/combatants.json` carries initial
  tables for four combatants; and the Python validator checks that every declared
  element is dealt by at least one skill and that every value falls within bounds.
- **`Event::StatusHealed { target, status, amount }`** (#11): regeneration and
  any future recovery that comes from a condition rather than from an actor.
  This is the exact mirror of `Event::StatusDamaged`, which was added in `0.1.0`
  for the same reason — a bleed tick logged as `Damaged` claimed a character
  attacked itself. Regen had the identical defect on the healing side and it had
  survived three releases unnoticed, because nothing was reading the actor.
- **`healing_done` on `CombatantStats`**, with `healing_done_per_battle()` and a
  `heal/b` column in the harness table beside `dealt/b`. The field is
  `#[serde(default)]` and appended last, so a report written by an older release
  still deserializes and reads zero — covered by an extended compatibility test.

### Changed

- **The AI scores actions instead of ranking damage** (#9). `ai::best_offensive`
  chose the highest-power affordable skill that dealt damage to an enemy, which
  made three skills unreachable by construction and made one dominant by
  accident. `score_action` now converts every effect into one unit — the damage
  a plain attack from this actor would deal — so a guard, a buff, a bleed and a
  tempo lock can be compared with a barrage without any of them being special
  cased. Healing is still handled by the support profile, not by the score.
- **SP is priced at its best other use** (#9, same file). A score that ignores
  cost spends a whole pool on the first expensive thing it can reach. Each
  candidate is now judged net of what the same SP would buy from the actor's own
  remaining options, so a skill has to beat not just the basic attack but the
  alternative that the SP is being taken away from. This is what makes Dio open
  with `tempo_halt` and fall back to knives once halting is no longer
  affordable, rather than emptying the pool on whichever came first.
- `stand.hierophant_green` now carries `skill.emerald_snare` and
  `skill.emerald_splash`. It previously reached `skill.blade_volley`, which is
  Dio's thrown steel and belongs to no version of Hierophant Green.
- `skill.blade_volley` 24 SP → 36 SP. At 24 it dominated every one of Dio's
  other options; at 40 it was priced out of the fight entirely and went unused
  across twelve boss battles, which traded a balance defect for dead content. 36
  is the only price at which Dio halts first at a full pool and throws knives
  once halting is unaffordable, and it is only reachable at all because the pool
  now refills past it.
- **Elemental resistances are applied in damage resolution** (#10).
  `resolve::compute_damage` now reads the target's resistance table: a positive
  value cuts damage by that percentage (floored at 1), a negative value amplifies
  it — `max(damage * (100 − resist) / 100, 1)`. Three unit tests cover the
  modifier, the immunity edge case, and element isolation. The initial tables
  in `data/combatants.json`: Jotaro (`temporal: 50`), Dio (`temporal: 50,
  psychic: 20`), Flame Assassin (`fire: 75`), Iron Brawler (`physical: 25,
  psychic: -25`). Street Thug carries no table, which is the intentional control:
  the `matchup.thug_solo` report must remain bit-identical.
- **`Event::Healed` gained an `actor` field** (#11). **This is a breaking change
  to the event wire format**, and the only one in this milestone. Anything
  matching exhaustively on `Event` must be recompiled, and anything reading the
  JSON stream sees a new key on `healed` and an unfamiliar `status_healed` kind.
  The Godot bridge survives the field addition because `battle_view.gd` reads
  every field by name rather than by position; it needed the *variant* handled,
  or every regeneration tick would have fallen to the catch-all and dumped raw
  JSON into the battle log.
- **A combatant is only reported as inert if it neither dealt damage nor healed**
  (#11). `MatchupReport::inert_combatants` tested `damage_dealt == 0`, which
  would have accused a dedicated healer of sitting out a fight it was carrying.
  The harness warning was reworded to match what the predicate actually selects.
- `README.md` now states `0.3.0` as released and `0.4.0` as in progress, and
  names the skills the AI can never choose, so every published win rate is read
  as measuring a subset of the content. Adds a contributing section and a link
  to the triage document. **That list is now two skills, not three:**
  `skill.tempo_halt` became Dio's opening move and `skill.blade_volley` became
  reachable once it was repriced.
- [docs/RELEASING.md](docs/RELEASING.md) records the two failures that occurred
  during the `0.3.0` release and were not caused by this repository: the
  advisory-db fetch aborting with a schannel error, and `git push` timing out on
  the LFS `locks/verify` endpoint. Both are retried, not worked around.
  Suppressing either one hides a real failure the next time it happens.
- [docs/TRIAGE.md](docs/TRIAGE.md) gained the label-creation procedure, an
  explanation of how each form field becomes issue text, and the `0.4.0`
  backlog as filed.

Re-measured after all of the above (steps 1 through 3), twelve seeds per
encounter:

| Encounter | Win rate | Declared band | Before #9 | Before #10 | Before #11 |
| --- | --- | --- | --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 | 100% | 100% | 100% |
| `matchup.assassin_ambush` | 50% | 45..90 | 67% | 67% | 50% |
| `matchup.dio_boss` | 42% | 35..75 | 50% | 42% | 42% |

The `matchup.thug_solo` control is **bit-identical** in every column before and
after resistances: no combatant in a three-turn fight carries a table, so the
report is a direct check that the schema change touched nothing it should not.

The ambush fell 67 → 50 when resistances landed. The Iron Brawler carries
`physical: 25` and `psychic: -25`, but the AI scorer does not read resistance
tables when pricing effects, so Kakyoin does not prefer psychic skills against
the Brawler's vulnerability and Jotaro does not discount his physical hits
against the Brawler's resistance. The band still holds at 50%. See
`docs/BALANCE-LOG.md` for the full entry and prediction scorecard.

Step 3 moved **nothing at all** — all three encounters, and every `dealt/b`,
`taken/b`, `sp/b`, `rolls`, `miss%` and `alive%` figure in all three tables, are
unchanged from the step-2 run. That was the stated pass condition, and it is the
reason #11 does not open a new comparability boundary.

The #9 figures in the "Before #10" column are coincidences of the same width,
not evidence that nothing changed: the ambush moved to 50% and back to 67% over
the four intermediate runs, and the boss passed through 8% and 0% before landing
at 42%. The thug fight is genuinely untouched, because it ends in three turns
and no SP budget binds in three turns.

**What the new column immediately showed.** In `matchup.dio_boss`, Josuke
restores **1044 HP per battle** while dealing 358 — the healer's real output is
nearly three times its damage, and against 2740 HP of incoming damage per battle
it offsets 38% of everything the enemy does. Dio self-heals **183 per battle**
through `blood_drain`, sustain that was previously invisible and that inflates
its effective pool well past the 1520 printed on the sheet. Neither figure could
be derived from anything the harness printed before.

### Fixed

- **`miss%` was misses divided by actions** (#8). An accuracy roll happens once
  per target and an action once per skill use, so the two only agree for
  single-target skills. `CombatantStats` now counts `attack_rolls` and owns
  `miss_percent()`, so the binary can no longer invent its own definition of the
  column, and the report warns loudly if misses ever exceed rolls. No RNG draw
  changed in that commit, so it was verifiable against the `0.3.0` battles
  before the rules moved.

  **The old column was already wrong on shipped content, not merely at risk.**
  This fix was written as a defence against a future area skill, on the belief
  that the AI could not reach one. Reading `ai::best_offensive` disproved that:
  it filtered on affordability, hostility and non-zero power, and nothing else,
  so `skill.blade_volley` — power 110, `all_enemies` — was Kakyoin's highest
  scoring option and was chosen whenever he could pay for it. `0.3.0` therefore
  reported Kakyoin at 20% miss against a true 11% in `matchup.assassin_ambush`.
  The arithmetic closes exactly: eight misses over roughly forty actions reads
  as 20%, and the same eight misses over seventy-two accuracy rolls is 11%. The
  numerator never moved; only the denominator was wrong. Josuke moved the other
  way, to a figure *higher* than the old formula gave, because every `restore`
  was an action that made no roll and quietly inflated the old denominator.

- **Effects denominated in HP were compared against effects denominated in
  power** (#9). The first draft of the score compared a guard's prevented damage
  directly against a skill's power number, which are different units; a guard
  worth two hits scored as if it were worth a fifth of one. Both sides now pass
  through `as_power_units`, and `Situation` carries the actor's attack so the
  conversion is possible at all.

- **SP never came back** (#9). Nothing in the engine restored SP: a pool was a
  one-off budget for the whole battle. In a 52-turn boss fight that meant Jotaro
  spent 45 on one halt, 18 on one barrage, and then threw punches for forty
  turns, and it made the party 353 HP short of Dio's 1520 no matter how well it
  chose. Recovery closes that gap without touching a single stat: Jotaro's
  damage per battle went from 1132 to 1413 and the boss fight from 0% to 42%.

- **Regeneration was logged as a self-heal** (#11). `battle::tick_statuses`
  emitted `Event::Healed { target: carrier }` for a `Regen` tick, so the log
  claimed a character healed itself through an action it never took. This is the
  same defect as the `0.1.0` bleed fix, one release later and on the opposite
  sign, and it was only found because giving `Healed` an actor left the regen
  call site with no honest value to supply. It now emits `StatusHealed`, which
  names the condition and no actor at all. A regression test asserts both halves:
  that `StatusHealed` is present, and that no `Healed` event is emitted anywhere
  in a regeneration tick.

### Known limitations

Carried forward from `0.3.0` except where noted:

- **Two skills are still unreachable.** `skill.guard_stance` at potency 60 and
  `skill.rage_focus` at potency 40 for 10 SP are both correctly declined — the
  score says they are worth less than hitting something, and two tests pin that
  conclusion at the numbers the game currently ships. This is now a content
  problem with a measurement behind it, not an AI defect, and M3's "zero
  unreachable skills" criterion is not met until those numbers change.
- **A tempo lock is still priced as a removal, not a deferral.** Locked tempo is
  delayed, not deleted, so the score overvalues `skill.tempo_halt` by however
  much of that tempo is eventually paid back. It mattered little when the skill
  was never used; Dio now uses it 44 times per batch.
- **Recovery is silent.** No event is emitted when SP returns, so a Godot HUD
  cannot show it yet. Adding a variant means touching every exhaustive match in
  the bridge, which is its own change.
- **Twelve seeds resolve to 8.3 percentage points.** One battle changing hands
  moves a reported figure by more than most of the changes in this milestone.
- **The harness plays the score, not a person.** It never sets up, never retreats
  and never saves SP for a phase change, so a reported win rate is a floor for a
  competent player rather than a forecast of their experience.
- **Healing taken is still not reported.** `heal/b` counts HP a combatant
  restored, and there is no matching column for HP a combatant received, so the
  party's damage taken still reads as if none of it was undone. The events now
  carry enough to compute it; nothing consumes them yet.
- **The AI is resistance-blind.** `score_action` prices `Effect::Damage` from
  its `power` field without consulting the target's resistance table, so the AI
  does not prefer psychic skills against the Iron Brawler's psychic vulnerability
  and does not avoid physical skills against its physical resistance. The measured
  consequence: `matchup.assassin_ambush` fell 67 → 50 after step 2; the band
  still holds. Tracked as a follow-up issue in milestone `0.4.0`.
- **The AI is also healing-blind in the same way.** `score_action` never sees
  `heal/b`; the support profile decides when to heal by its own rule, so Josuke's
  1044 HP per battle is a consequence of that rule rather than of any comparison
  against what the same SP would have bought as damage.

## [0.3.0] - 2026-07-29

M2. Balance stops being an opinion. Every encounter in the game now declares the
win rate it is supposed to produce, a headless harness measures whether it does,
and CI fails the build when it does not.

The measurement changed the content. Before the harness, two of three encounters
were outside the range their designer would have claimed for them -- one was a
100% guaranteed win presented as a real fight, and the boss was won 92% of the
time. Both are now inside their declared bands, and the reasoning behind every
number that moved is in [docs/BALANCE-LOG.md](docs/BALANCE-LOG.md).

### Added

- **Headless batch simulator** (`rpg_core::sim`). `run_matchup` plays one
  encounter over a fixed seed list with both sides driven by the existing AI;
  `run_batch` runs every encounter in the file. No Godot, no rendering, no
  wall-clock access -- the same seed produces the same battle on every machine.
- **`rpg_core::report`**: per-encounter win rate, median and min/max battle
  length, and per-combatant damage dealt, damage taken, SP spent, miss rate and
  survival rate. A report is the unit of evidence in this project; a
  playthrough is not.
- **`data/matchups.json`**: encounters as content, not as test fixtures. Each
  declares its party, its foes, twelve fixed seeds, a `min_percent..max_percent`
  win-rate band and a turn limit. The band is the design intent, written down
  where it can be checked.
- **`cargo run -p rpg-core --bin balance`**: the harness as a command. Prints a
  per-encounter breakdown, exits non-zero when any encounter leaves its band.
  Flags: `--only <id>`, `--json`, `--no-fail`, and `--combatants`/`--stands`/
  `--skills` to run against content outside `data/`.
- **`balance` CI job**, in the `ci` aggregate alongside format, clippy, tests
  and the audits. It runs the binary with no arguments, so the gate can only
  ever measure the content compiled into the crate.
- **`core/tests/balance_bounds.rs`**: the same assertion as a test, so a local
  `cargo test` catches a balance regression before a push does.
- **[docs/BALANCE-CURVE.md](docs/BALANCE-CURVE.md)**: the measured relationship
  between enemy output and win rate, swept in one run over five clones of the
  same enemy. The curve is flat up to a ratio of 0.65, bends between 0.65 and
  0.74, holds a plateau to 0.81, then falls to 8% by 0.99. Six of the seven
  changes in this milestone were sized against an interpolation that turned out
  to be wrong by 0.15; the sweep that replaced it cost a single run.
- **`tools/probe/`**: throwaway content used for that sweep, with a README
  stating plainly that it is not game content and cannot reach the game, the
  validator or the CI gate.
- The content validator now checks `data/matchups.json`: id namespacing and
  uniqueness, every combatant reference resolving, no combatant on both sides of
  a fight, duplicate seeds rejected, bands within 0..100 and correctly ordered,
  and warnings for a 0..100 band, a seed list too short to resolve a percentage,
  and any combatant that no encounter exercises.

### Changed

Seven changes, each measured before and after, each in its own commit, all
argued in [docs/BALANCE-LOG.md](docs/BALANCE-LOG.md):

- **Enemy targeting is no longer deterministic** (`FOCUS_FIRE_CHANCE = 55` in
  `ai.rs`). Unconditional lowest-HP focus fire meant Kakyoin was downed in 12 of
  12 boss battles and absorbed 63% of all enemy damage -- a guaranteed casualty
  from turn one, decided before the fight started. Hostile actors now commit to
  the weakest target 55% of the time and pick at random otherwise. This is the
  only change in the milestone that touches the rules, and it is the one that
  every earlier finding turned out to be about.
- `skill.restore` heal power 230 -> 140. Healing returned 11.5 HP per SP and
  roughly 950 HP per battle from a character nobody was attacking. (Measured
  directly for the first time in `0.4.0`: 1044 HP per battle, so the estimate
  was low by 9%.)
- `npc.dio` atk 100 -> 130. With targeting fixed, the boss could not win: the
  party out-healed and out-lasted it 100% of the time.
- `npc.flame_assassin` hp 480 -> 640, so it stops dying below Kakyoin's HP pool
  and lives long enough to act.
- **New enemy `npc.iron_brawler` and new `stand.iron_hymn`**, replacing the
  300 HP thug in the ambush. That thug dealt 13 damage per battle; the encounter
  was not a fight, and no stat on a body that small could make it one.
- `npc.iron_brawler` atk 80 -> 105 -> 135, the last step taken from the curve
  sweep rather than predicted. It reproduced the swept report digit for digit.

Measured result of all of the above:

| Encounter | Win rate | Declared band |
| --- | --- | --- |
| `matchup.thug_solo` | 100% | 85..100 |
| `matchup.assassin_ambush` | 67% | 45..90 |
| `matchup.dio_boss` | 50% | 35..75 |

### Fixed

- `sim::tests::damage_is_attributed_to_both_sides_of_every_hit` asserted a
  hero could only be downed once across a whole seed list. The test had never
  been executed before it was pushed; the expectation was wrong, not the engine.
- The duplicate-seed guard in the matchup parser was a `seen` vector filtered by
  a predicate that could never return true, so it accepted duplicates silently.
  Replaced with a real membership test and a test that covers it.
- `balance-report.json` is ignored, so a local `--json` run cannot be committed
  by accident.

### Known limitations

Recorded so a win rate in this file is not read as more than it is:

- **Twelve seeds resolve to 8.3 percentage points.** 67% and 75% are not
  distinguishable results, and a win rate near a band edge is not decisively
  inside or outside it.
- **The harness plays worse than a person.** Auto-battle takes the
  highest-power affordable skill and never sets up, so a reported win rate is a
  floor for a competent player, not a forecast of their experience.
- **Five of eleven skills are unreachable.** `ai::best_offensive` filters on
  damage, so buffs, guards and area attacks are never chosen -- Jotaro has never
  used `skill.guard_stance` in any recorded run. Every number here therefore
  measures a subset of the content.
- **Healing is not attributed.** `Event::Healed` carries a target and no source,
  so healing done has to be inferred from SP spent. (Fixed in `0.4.0`.)
- **`miss%` is inflated for area skills**, which count a miss per target and an
  action once.
- **A rules change resets the series.** RNG draw order is part of the rules, so
  after any edit to `ai.rs`, `resolve.rs` or `battle.rs` the same seed no longer
  reproduces the same battle, and numbers across that boundary are not
  comparable.
- **The ambush is still decided by one character.** Jotaro deals 76% of the
  party's damage and his survival rate tracks the encounter's win rate exactly
  across all five swept points. The band is closed; the roster imbalance behind
  it is not.
- **The Iron Brawler hits at an effective 157 against Dio's 160**, which is
  mechanically correct and fictionally awkward for a random-encounter enemy. The
  alternative was nerfing a skill shared with the boss fight.
- Elemental resistances are carried in events but not yet applied in damage.

## [0.2.0] - 2026-07-28

M1. The game is playable: one battle, start to finish, driven entirely by the
player. Verified by playing it in Godot 4.7.1 on Windows -- commands and targets
chosen by hand, events animated in order, ending on a resolution screen with a
clean console. No CI job in this repository can make that claim, because none of
them run Godot.

### Added

- `BattleView` rebuilt around a real, procedurally-built UI: per-combatant
  HP/SP bars, a tempo gauge (sorted by each combatant's current `tempo`
  value, the same key `Battle::ready_actor` uses to pick who acts next), a
  command menu (Attack / known skills / Guard / Wait), and a target picker
  for single-target actions with a cancel path back to the command menu.
- Event animator queue in `battle_view.gd`: events are queued and played one
  at a time with a fixed delay, replacing the immediate `print` dump. The
  queue is re-entrant-safe, so a call arriving mid-drain appends instead of
  racing it.
- Real target selection, replacing the stub that always attacked the first
  living foe. Single-enemy and single-ally actions list every legal living
  target; multi-target and self-only skills submit immediately, since
  `rpg_core::resolve::resolve_targets` ignores the `target` field for those
  kinds.
- `status_damaged` now renders as its own log line, distinct from `damaged`;
  the meaningless `potency` field on binary statuses (stun) is no longer
  printed.
- Victory / defeat / stalemate resolution: the command and target menus are
  torn down and a result line is shown once `Phase::Finished` is reached.

### Fixed

- The Markdown link check no longer fails on a link that is not dead. It
  reported zero errors and two timeouts, both on the same cited URL
  (`prng.di.unimi.it`, the reference SplitMix64 source), and lychee exits
  non-zero on a timeout. That host is a single unfronted academic server;
  blocking a merge on its uptime measures nothing about this repository, so
  it joined `.lycheeignore` with the reason recorded inline. The citation
  itself stays in the prose.
- The temporary `.lycheeignore` entries for `v0.1.0` URLs were removed. They
  existed only because the release did not exist yet when they were written;
  those links now resolve and are checked for real.
- The Windows quickstart invoked the content sync script through `pwsh`,
  which is PowerShell 7 and is not present on a stock Windows install. The
  script has always been 5.1 compatible, so the documented command was the
  only obstacle. The step numbering was also corrected: the `Copy-Item` line
  is relative to the repository root, and running it from `src/` looks for
  `src/src/`.

### Known limitations

- Combat is one-sided against the player. The first real playthrough ended in
  defeat with the boss on 389 of 1520 HP and three of five combatants downed.
  One battle is an anecdote, not a measurement, which is precisely why M2
  exists: no balance claim in this repository is evidence-based until the
  harness reports win rates over thousands of seeded runs.
- The event log is the only feedback channel. Damage does not appear on the
  combatants themselves, so the fight is read by scrolling text.
- The tempo gauge shows each combatant's current `tempo` value sorted the way
  the scheduler ranks them; it is not a forecast of the next several turns.
  A real forecast would need effective, status-modified speed exposed from
  the core, which does not happen yet.
- Elemental resistances are still carried in events but not applied in damage.
- SP is still not a binding constraint for most combatants.

## [0.1.0] - 2026-07-28

First fixation. The simulation is complete and tested; the presentation layer is
a runnable stub, not a game.

### Added

- **`rpg-core`**: deterministic, engine-free turn-based battle simulation.
  - Tempo (ATB) scheduler with integer accumulation and deterministic
    tie-breaking.
  - `tempo_lock` primitive for tempo-denial abilities, with no special case in
    the scheduler.
  - Command to event pipeline: validation precedes mutation, and a rejected
    command does not consume the turn.
  - SplitMix64 RNG stored inside `BattleState`; no floats, no globals, no clock
    access, so a seed plus a command sequence fully determines the event log.
  - Data-driven content: skills, stands and combatants loaded from `data/*.json`
    with reference validation at load time.
  - Three AI profiles (aggressive, support, trickster) drawing from the battle
    RNG, so unattended battles stay replayable.
- **`rpg-bridge`**: GDExtension boundary exposing five methods that exchange JSON
  strings only, so a schema mismatch produces a parse error rather than
  undefined behaviour.
- **Godot 4 project**: `BattleView` scene and script that drive the simulation
  and animate its events without recomputing any outcome.
- **Content**: 11 skills, 5 stands, 6 combatants, plus a stdlib-only Python
  validator that checks types, enums, cross-references and balance smells.
- **Documentation**: architecture, engine decision record (ADR-0001), roadmap,
  Windows development guide, release process, data provenance policy.
- **CI/CD**: rustfmt, clippy with `-D warnings`, tests and doc builds on Linux
  and Windows, content validation, `cargo-deny` supply-chain audit, spellcheck,
  Markdown link check, Dependabot, and tag-triggered releases producing
  per-platform archives with checksums.
- `rpg_core::json_compat` with five unit tests, including a guard assertion
  that an un-normalized Godot-style payload must fail; without it the test
  would pass even if the normalizer were deleted.
- Regression test asserting both halves of the damage-over-time fix: the
  presence of `status_damaged` and the absence of any self-attributed
  `Damaged`.
- Windows setup guidance covering the failures met in practice: PowerShell
  versus `cmd.exe`, `where` shadowed by `Where-Object`, and archives that
  extract into a subdirectory named after the archive.

### Fixed

These are defects found and corrected before the first tag existed. They are
recorded rather than squashed away, because the reasoning is the useful part.

- **GDExtension no longer fails to compile.** `GString` implements
  `From<&str>` and `From<&String>` but not `From<String>`, because the
  conversion copies into Godot-managed memory. Every conversion now goes
  through a single `json_to_gstring` helper.
- **Content authored in GDScript is accepted again.** Godot's JSON has no
  integer type, so numbers round-tripped through `JSON.parse_string` and
  `JSON.stringify` arrive as `95.0` where the schema declares `i32`.
  `rpg_core::json_compat` normalizes integral floats at the FFI boundary,
  leaving the simulation's integer-only schema intact.
- **`compatibility_minimum` now states the truth.** It claimed 4.2 while
  `godot` 0.5.3 compiles against the Godot 4.6 API. Lowering that number
  never widened support; it only postponed the failure from a clear version
  rejection to a missing-symbol crash. Verified against the engine's own
  startup line: `API v4.6.stable.official, runtime v4.7.1.stable.official`.
- **Damage over time is attributed to the status, not to its victim.** A bleed
  tick was logged as `Damaged { actor, target }` with both fields set to the
  victim and the element hard-coded to `physical`, so the log claimed a
  character attacked itself with a blow it never made. It now emits
  `status_damaged { target, status, amount }`, with no attacker, no element
  and no crit -- none of which exist for a condition. `Damaged` consequently
  means exactly one thing, which an animator can rely on. (The mirror defect
  on the healing side survived until `0.4.0`; see #11.)
- Removed a needless `mut` in `Database::skills_for` that would have failed
  CI, where warnings are errors.
- **CI no longer denies warnings across the dependency tree.** A
  workflow-level `RUSTFLAGS: -D warnings` applies to every crate cargo
  compiles, so a single warning from `gdext`, `glam`, `venial` or `syn` failed
  the Linux and Windows builds on commits that touched none of them. Denying
  warnings is a policy for code we own, so it is passed to clippy per job.
- **The supply-chain audit distinguishes wildcards by origin.** `cargo-deny`
  rejected `rpg-core = { path = "../core" }` as a wildcard dependency. A path
  dependency carries no version requirement by construction, but it can only
  ever resolve to a directory in this repository. Wildcards remain denied for
  registry crates, where they really do accept any future release.
- **The link checker actually checks links.** It was invoked with
  `--exclude-mail`, removed in lychee 0.24 in favour of `--include-mail`,
  which already defaults to false. The unknown argument aborted the run before
  a single link was fetched, producing a failure that looked like a dead link.
- Corrected a misspelling in `battle_view.gd` reported by the spellchecker,
  and moved every workflow to `actions/checkout@v5`, which clears the Node.js
  20 deprecation warning.

### Known limitations

- No gameplay UI yet; `battle_view.gd` auto-selects a basic attack so the loop
  can be run end to end. A battle driven by that stub is not evidence about
  balance: the party never uses a skill, never heals, and never retargets, and
  it always attacks the first living foe, so a second enemy is never touched.
- SP is not yet a meaningful constraint. In recorded runs the strongest foe
  spent 176 SP across eight uses of one skill without ever running dry.
- Elemental resistances are carried in events but not yet applied in damage.
- Combat numbers are unbalanced; only determinism and termination are tested.
- Seeds must stay below 2^53 when a battle is created from GDScript. Godot
  represents every JSON number as a 64-bit float, whose mantissa is 53 bits,
  so a larger seed would be silently rounded and the fight would not replay.
  `Time.get_unix_time_from_system()` is far below that ceiling; a full-range
  `u64` seed would have to cross the boundary as a string.

[Unreleased]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-/releases/tag/v0.1.0
