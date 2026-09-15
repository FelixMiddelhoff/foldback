# Hashing Levels (Tick / Entity / Field)

Foldback's integration cost is tiered on purpose — you get useful results from the cheapest level, and each level above it costs more integration work in exchange for finer bisection.

## Level 1 — per-tick (shipped)

One hash per tick, covering however much state you decide determinism depends on.

```rust
let state_bytes = bincode::serialize(&world.deterministic_state())?;
session.hash_tick(tick, &state_bytes)?;
```

**What it buys you**: exactly which tick diverged. **Integration cost**: one call per tick, one serialization function you already control. This is the entire Level 1 integration — nothing else required.

## Level 2 — per-entity (shipped)

Instead of one hash for the whole tick, one hash per entity — `Session::hash_entity(tick, entity_id, state_bytes)`.

```rust
for (entity_id, component_bytes) in world.iter_entities_serialized() {
    session.hash_entity(tick, entity_id, &component_bytes)?;
}
```

**What it buys you**: which *entity* diverged, not just which tick. **Integration cost**: your game needs to expose per-entity state independently, not just a single serialized blob. See `examples/ggrs-demo` for a real recording using this.

## Level 3 — per-field (shipped)

Structured field-level hashing via `#[derive(FoldbackHash)]` (Rust, feature `derive`) — every other binding has its own equivalent attribute: `[FoldbackHash]` in Unity, the `foldback_` naming convention in Godot (GDScript has no custom-attribute mechanism GDExtension can hook into — see [Auto/Reflective Hashing](../integrations/reflective-hashing.md) for why), `UPROPERTY(meta=(FoldbackHash))` in Unreal. Opt-in per field in every case — see [RFC-0003](../project/rfcs/0003-field-hashing-opt-in.md) for why opt-in and not opt-out.

```rust
#[derive(foldback_core::FoldbackHash)]
struct PlayerState {
    #[foldback(hash)]
    velocity: [f32; 3],
    debug_name: String, // unmarked — named in PlayerState::UNTRACKED_FIELDS
}
session.hash_fields(tick, entity_id, &player_state)?;
```

**What it buys you**: the actual diffed value at the leaf — `velocity.x: 3.14159 vs 3.14158`, copy-pasteable into a bug report. **Integration cost**: annotate the fields you want tracked with `#[foldback(hash)]`; an unmarked field is invisible to bisection but named in the generated `UNTRACKED_FIELDS` constant rather than the gap staying silent. `foldback-cli lint --engine bevy <path>` does the equivalent scan across a whole source tree, printing every tagged type's tracked/untracked split at once — see [CLI Reference](../usage/cli.md).

## Choosing a level

Start at Level 1 — it's nearly free and already tells you which tick to look at by hand. Add Level 2/3 only for the parts of your simulation you're actively debugging a real desync in; there's no requirement to instrument everything up front.
