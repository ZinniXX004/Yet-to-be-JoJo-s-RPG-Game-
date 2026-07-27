# Roadmap

Milestones are ordered so that each one is independently demonstrable. For a
portfolio project, a finished vertical slice beats a half-built epic every time.

## M0 - Foundation (done)

- [x] Engine/architecture decision recorded (ADR-0001)
- [x] Correct `.gitignore`, Git LFS configured before any binary asset
- [x] Data-driven content schema in `data/`
- [x] Deterministic battle core with tempo scheduler and event pipeline
- [x] Determinism regression test
- [x] Python data validator + CI on every push

## M1 - Playable battle loop

**Exit criterion:** one full battle, start to victory screen, playable with mouse
and keyboard, no placeholder crashes.

- [ ] Pin `godot` crate version; get GDExtension loading in the editor
- [ ] `BattleView` scene: HP/SP bars, tempo order preview, command menu
- [ ] Event animator: queue events, play one at a time, never block input forever
- [ ] Target selection UI, including multi-target skills
- [ ] Victory / defeat resolution and transition

## M2 - Content depth

**Exit criterion:** three playable characters, six enemies, two boss fights, all
authored purely in `data/`.

- [ ] Status effect icons and durations surfaced in the UI
- [ ] Elemental resistance table (extension point already in the core)
- [ ] Enemy AI profiles beyond aggressive/support (scripted boss phases)
- [ ] Balance harness: run N AI-vs-AI battles, report win rates and turn counts

## M3 - Game around the battle

**Exit criterion:** a 20-30 minute slice with progression.

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
- [ ] Tag `v0.1.0` release with per-platform binaries

## Explicit non-goals

Scope discipline is the whole game. These are out until M4 ships:

- Multiplayer or netplay
- Procedural content generation
- 3D
- Mod support / plugin API
- Mobile ports
