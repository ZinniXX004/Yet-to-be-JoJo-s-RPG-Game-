# ADR-0001: Engine and language architecture

- **Status:** Accepted
- **Context date:** 2026-07
- **Decision:** Godot 4 for presentation, Rust for the simulation core, connected
  by one narrow GDExtension boundary.

## Context

The primary goal of this project is a **technical portfolio artifact** used in
hiring conversations, with a playable vertical slice as the proof. That goal
changes the evaluation criteria: "what demonstrates senior-level engineering
judgment" outranks "what ships a game fastest".

The original `.gitignore` implied Unity *or* Godot, plus Rust, plus Python, with
no decision recorded. That ambiguity is itself the problem this ADR resolves.

## Options considered

| Option | Iteration speed | Portfolio signal | Interop risk | Verdict |
|---|---|---|---|---|
| Godot 4, GDScript only | Highest | Low. Reviewers cannot distinguish it from a tutorial project. | None | Rejected |
| Unity + C# only | High | Medium. Very common; hard to stand out. | None | Rejected |
| Unity + Rust via raw C FFI | Low | High, but for the wrong reasons | High: manual marshalling, P/Invoke, GC pinning, opaque crashes | Rejected |
| **Godot 4 + Rust core via GDExtension** | Medium | High: demonstrates layered architecture, determinism, and testability | Medium, and confined to one file | **Accepted** |

## Rationale

1. **Rust is not here for performance.** A turn-based RPG has no compute
   bottleneck. Claiming otherwise would be dishonest engineering. Rust is here
   because it forces the simulation to be expressed as explicit state
   transitions with no hidden mutation, which makes determinism and headless
   testing achievable. That is the actual deliverable.
2. **The boundary is the risk, so the boundary is minimized.** The FFI surface is
   four functions that exchange JSON strings. No struct layouts cross the
   boundary, no ownership is transferred, and the schema can evolve without
   touching the ABI. This trades a small amount of serialization cost (per player
   action, not per frame) for a boundary that is nearly impossible to corrupt.
3. **Godot over Unity** because GDExtension is a first-class, documented native
   extension mechanism, the whole project stays under one open-source toolchain,
   the editor is scriptable from source control as plain text, and the repository
   stays free of the large binary metadata churn Unity produces.
4. **Python stays a build/CI tool only.** It validates content data and produces
   balance reports. It never runs at game runtime.

## Consequences

- Requires a working Rust toolchain plus a per-platform cdylib build step. This
  must be automated in CI or contributors will hit it immediately.
- Iteration on game *feel* is slightly slower than pure GDScript, since rule
  changes require a recompile. Mitigated by keeping all tunable numbers in
  `data/*.json`, which is hot-reloadable without recompiling.
- Anything cosmetic (animation timing, camera shake, VFX) must live in Godot. If
  cosmetic logic starts leaking into `rpg-core`, the architecture has failed.

## Verification needed

The exact `godot` crate version and the `compatibility_minimum` in
`src/game/rpg.gdextension` must be pinned against the actual Godot build used
locally. The godot-rust API is pre-1.0 and still makes breaking changes.
