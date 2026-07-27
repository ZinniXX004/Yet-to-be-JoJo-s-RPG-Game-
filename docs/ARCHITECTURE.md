# Architecture

## Layer rules (enforce these in review)

1. `rpg-core` must never depend on any engine crate. If `godot` appears in
   `src/core/Cargo.toml`, the design is broken.
2. `rpg-core` must never read the clock, spawn threads, use floating-point RNG,
   or iterate a `HashMap` in a way that affects outcomes. Determinism is a hard
   invariant, not a nice-to-have.
3. `rpg-bridge` contains no game rules. It only serializes, deserializes, and
   forwards.
4. `src/game` contains no game rules. It renders events and collects input.
5. No content in code. Numbers, names, and text live in `data/*.json`.

## Simulation model

### Tempo scheduler (ATB, integer-only)

Every combatant holds a `tempo` counter. Each tick, `tempo += effective_spd`.
When `tempo >= TEMPO_THRESHOLD` (1000), that combatant may act; acting subtracts
the action's `tempo_cost`. Heavy skills cost more than 1000, so they push the
actor further down the order. This yields speed-relevant, interruptible turn
order without floats and without a real-time loop.

`tempo_lock` freezes a combatant's tempo accumulation for N ticks. This is the
generic primitive behind time-stop style abilities. Note that it is expressed as
a data-level effect, not a special case in the scheduler.

### Command / event pipeline

```
Command (intent)  ->  resolve()  ->  Vec<Event> (facts)  ->  presentation
```

- A `Command` is what an actor *wants* to do. It is validated (alive, has SP,
  legal target) before resolution.
- An `Event` is an immutable record of what *happened*. The Godot layer animates
  events; it never recomputes them.
- Because the event log is the single source of truth for presentation, the same
  log can be replayed, serialized to disk for a bug report, or diffed in a
  regression test.

### Determinism

`Rng` (SplitMix64) is stored inside `BattleState`, not in a global. Combined with
integer-only math and index-based (never hash-ordered) iteration, this gives:

```
same seed + same command sequence  ==  identical event log
```

`src/core/tests/determinism.rs` asserts exactly this. Treat a failure there as a
release blocker, not a flaky test.

### Damage formula

```
base      = power * effective_atk / 100
mitigated = max(1, base - effective_def / 2)
jittered  = mitigated * rng(100 - variance ..= 100 + variance) / 100
crit      = clamp(5 + (atk_will - def_will) / 2, 1, 50) percent  -> x3/2
```

The `max(1, ...)` floor guarantees no fully immune wall, so a fight can never
deadlock through pure mitigation. Elemental resistance is a deliberate open
extension point: `Element` is already carried through every damage event, but no
resistance table is applied yet.

## FFI contract

`rpg-bridge` exposes a Godot `RefCounted` class with four methods, all exchanging
JSON strings:

| Method | Input | Output |
|---|---|---|
| `create(config_json)` | full battle config, incl. seed and content | session handle |
| `advance()` | none | `{ "phase": ..., "events": [...] }` |
| `submit(command_json)` | one command | `{ "ok": true, "events": [...] }` |
| `state_json()` | none | full `BattleState` snapshot |

Why JSON and not native structs: the schema will churn heavily during design
iteration. A struct-based ABI would require recompiling both sides in lockstep
and would risk undefined behavior on mismatch. Here, a mismatch is a parse error
with a message. Serialization happens once per player action, so the cost is
irrelevant at this scale.

## Build and wiring

```bash
cd src
cargo build -p rpg-bridge --release
# Linux
cp target/release/librpg_bridge.so   game/bin/
# Windows
cp target/release/rpg_bridge.dll     game/bin/
# macOS
cp target/release/librpg_bridge.dylib game/bin/
```

`src/game/rpg.gdextension` maps those paths per platform. `src/game/bin/` is
gitignored: it is build output.
