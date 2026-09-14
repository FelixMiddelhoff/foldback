# Hashing Levels (Tick / Entity / Field)

Foldback's integration cost is tiered on purpose — you get useful results from the cheapest level, and each level above it costs more integration work in exchange for finer bisection.

## Level 1 — per-tick (shipped, Phase 0)

One hash per tick, covering however much state you decide determinism depends on.

```rust
let state_bytes = bincode::serialize(&world.deterministic_state())?;
session.hash_tick(tick, &state_bytes)?;
```

**What it buys you**: exactly which tick diverged. **Integration cost**: one call per tick, one serialization function you already control. This is the entire Level 1 integration — nothing else required.

## Level 2 — per-entity (planned, Phase 2)

Instead of one hash for the whole tick, one hash per entity — the game provides an entity-keyed hash callback instead of a single opaque blob.

**What it buys you**: which *entity* diverged, not just which tick. **Integration cost**: your game needs to expose per-entity state independently, not just a single serialized blob.

## Level 3 — per-field (planned, Phase 2)

Structured field-level hashing via a derive macro (`#[derive(FoldbackHash)]` in Rust, an attribute in other languages), opt-in per field — see [RFC-0003](../project/rfcs/0003-field-hashing-opt-in.md) for why opt-in and not opt-out.

**What it buys you**: the actual diffed value at the leaf — `velocity.x: 3.14159 vs 3.14158`, copy-pasteable into a bug report. **Integration cost**: annotate the fields you want tracked; unmarked fields are invisible to bisection (and a lint surfaces them rather than leaving the gap silent).

## Choosing a level

Start at Level 1 — it's nearly free and already tells you which tick to look at by hand. Add Level 2/3 only for the parts of your simulation you're actively debugging a real desync in; there's no requirement to instrument everything up front.
