# Roadmap

Milestones are ordered so that each one is independently demonstrable. For a
portfolio project, a finished vertical slice beats a half-built epic every time.

Each milestone maps to exactly one version tag. The mapping is authoritative in
[RELEASING.md](RELEASING.md); it is repeated here so the two documents can be
checked against each other.

| Milestone | Version | Exit criterion |
| --- | --- | --- |
| M0 Foundation | `0.1.0` | Simulation tested, deterministic, reaching Godot |
| M1 Playable battle loop | `0.2.0` | One battle playable start to finish |
| M2 Balance harness | `0.3.0` | Headless mass simulation reporting win rates |
| M3 Content depth | `0.4.0` | Full roster, statuses and elemental resistances |
| M4 Portfolio polish | `1.0.0` | A stranger can play it in under 60 seconds |

## M0 - Foundation (done)

- [x] Engine/architecture decision recorded (ADR-0001)
- [x] Correct `.gitignore`, Git LFS configured before any binary asset
- [x] Data-driven content schema in `data/`
- [x] Deterministic battle core with tempo scheduler and event pipeline
- [x] Determinism regression test
- [x] Python data validator + CI on every push
- [x] `godot` crate version pinned; GDExtension loads in the editor and in the
      exported console binary
- [x] End-to-end proof: a full battle runs from GDScript through the FFI to a
      decisive outcome

## M1 - Playable battle loop

**Exit criterion:** one full battle, start to victory screen, playable with mouse
and keyboard, no placeholder crashes.

- [ ] `BattleView` scene: HP/SP bars, tempo order preview, command menu
- [ ] Event animator: queue events, play one at a time, never block input forever
- [ ] Target selection UI, including multi-target skills
- [ ] Replace the placeholder command stub, which always attacks the first living
      foe. Until this is gone, any enemy after the first can go a whole battle
      untouched, and no balance conclusion drawn from a stub-driven run is valid
- [ ] Surface `status_damaged` separately from `damaged` in the combat log, and
      suppress the meaningless `potency` on binary statuses such as stun
- [ ] Victory / defeat resolution and transition

## M2 - Balance harness

**Exit criterion:** `N` AI-vs-AI battles run headless in CI, reporting win rate,
median turn count and damage taken per combatant.

This lands before the content pass on purpose. Authoring a roster against
numbers that are still moving means authoring it twice.

- [ ] Batch runner over `step_with_ai`, seeded from a fixed list so results are
      comparable between commits
- [ ] Report per-combatant damage dealt and received, to catch enemies the AI
      never targets
- [ ] Assert bounds in CI: a matchup that is a guaranteed win or a guaranteed
      loss fails the build
- [ ] Rebalance `data/*.json` against the harness output, not against intuition
- [ ] Re-examine the SP economy: if a skill can be spammed for an entire battle
      without running dry, SP is not a resource and the AI is not choosing

## M3 - Content depth

**Exit criterion:** three playable characters, six enemies, two boss fights, all
authored purely in `data/`.

- [ ] Status effect icons and durations surfaced in the UI
- [ ] Elemental resistance table (extension point already in the core)
- [ ] Enemy AI profiles beyond aggressive/support (scripted boss phases)
- [ ] Overworld or node-based map navigation
- [ ] Party management, equipment, leveling
- [ ] Save/load (serialize the whole state; determinism makes this cheap)
- [ ] Dialogue system fed from `data/`

## M4 - Portfolio polish

**Exit criterion:** a stranger can play it in under 60 seconds from the repo.

- [ ] Web export playable in-browser, linked from the README
- [ ] 30-second gameplay GIF in the README
- [ ] Audio: SFX per event type, one battle track
- [ ] Write-up of the determinism design with a replay demo

## Explicit non-goals

Scope discipline is the whole game. These are out until `1.0.0` ships:

- Multiplayer or netplay
- Procedural content generation
- 3D
- Mod support / plugin API
- Mobile ports
