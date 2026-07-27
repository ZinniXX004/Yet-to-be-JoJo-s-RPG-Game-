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

### Event taxonomy and the attribution invariant

Events are serialized with `#[serde(tag = "kind", rename_all = "snake_case")]`,
so every payload carries a `kind` discriminator and the presentation layer can
match on it without positional assumptions.

Two damage-bearing variants exist, deliberately:

| Variant | Fields | Meaning |
| --- | --- | --- |
| `damaged` | `actor`, `target`, `amount`, `crit`, `element` | A combatant acted and hit something. `actor` is always a real attacker |
| `status_damaged` | `target`, `status`, `amount` | A status ticked. There is no actor, no element and no crit roll, so those fields do not exist |

This split is the single most important rule in the event schema, and it was
learned the hard way. Before it existed, damage-over-time reused `damaged` with
`actor` set to the victim's own index. The log then claimed that the bleeding
character had hit itself with a physical attack: the numbers were correct, the
story was a lie, and every future consumer of the log (animator, combat log UI,
damage-attribution statistics, replay analysis) would have needed an
`actor == target` special case to undo the damage.

The invariant to preserve: **if an event carries an `actor`, that actor chose to
do it.** Anything the simulation does on its own gets its own variant.
`battle::tests::bleed_is_attributed_to_the_status_not_to_its_victim` fails if
this regresses.

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

`rpg-bridge` exposes a Godot `RefCounted` class, `BattleSession`, whose methods
all exchange JSON strings (`GString`):

| Method | Input | Output |
|---|---|---|
| `create(config_json)` | full battle config, incl. seed and content | session handle |
| `advance()` | none | `{ "phase": ..., "events": [...] }` |
| `submit(command_json)` | one command | `{ "ok": true, "events": [...] }` |
| `step_with_ai()` | none | resolves the pending actor's turn with the built-in AI |
| `state_json()` | none | full `BattleState` snapshot |

`step_with_ai` exists so that a battle can run to completion with no UI at all.
That is what makes the headless balance harness possible, and it is also how the
bridge is smoke-tested without a scene.

Why JSON and not native structs: the schema will churn heavily during design
iteration. A struct-based ABI would require recompiling both sides in lockstep
and would risk undefined behavior on mismatch. Here, a mismatch is a parse error
with a message. Serialization happens once per player action, so the cost is
irrelevant at this scale.

### Numeric normalisation at the boundary

Godot's `JSON` class has no integer type. Every number it produces is a float64,
so a `95` authored in GDScript arrives in Rust as `95.0`, and `serde` correctly
refuses to deserialize that into an `i32`:

```
invalid type: floating point `95.0`, expected i32
```

The fix belongs at the boundary, not in the schema. Weakening `i32` to `f64` in
`rpg-core` would import floats into the one crate that must not have them and
would destroy replay determinism across architectures. Instead,
`rpg_core::json_compat::normalize_json_str` walks the parsed value and rewrites
every float whose fractional part is zero into an integer;
`rpg-bridge::normalize_incoming` applies it to every inbound payload before
deserialization. Fractional numbers are left untouched, so genuine decimals in
future schemas still work.

Two consequences worth knowing before debugging:

1. **Integers crossing the boundary must stay below 2^53**, the float64 mantissa
   limit. This is why the seed is derived from a Unix timestamp rather than a
   full `u64`: a larger value would be silently rounded on the GDScript side,
   before Rust ever sees it, and the run would not be reproducible from the seed
   it printed.
2. **The outbound direction cannot be fixed from Rust.** `rpg-core` emits
   `"seed":1785176952`, then Godot parses and re-stringifies it for printing as
   `1785176952.0`. The trailing `.0` in console output is Godot's formatting, not
   corrupted data. Use `%d` or `int()` when a value must be displayed as an
   integer.

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

`src/game/rpg.gdextension` maps those paths per platform and pins
`compatibility_minimum` to the Godot API level the `godot` crate was built
against, which is not the same as the editor version in use.

`src/game/bin/` and `src/game/data/` are both gitignored: one is build output,
the other is a copy of `data/` produced by `tools/sync_data.ps1` (or `.sh`).
After a fresh clone, neither exists, and the project will not run until both are
regenerated. See [DEVELOPMENT.md](DEVELOPMENT.md).
