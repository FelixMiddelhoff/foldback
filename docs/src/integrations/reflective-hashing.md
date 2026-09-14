# Auto/Reflective Hashing

**Status: stretch goal, not started.** Sequenced per engine, after that engine's explicit-hashing binding ships — Bevy first once Phase 2 lands, then Unity/Godot in their existing phase order.

## What it's for

Get Level 2/3 (per-entity, per-field) bisection *without* hand-writing `hash_entity`/`hash_field` calls — the binding walks the engine's own reflection system (`bevy_reflect`, C# `System.Reflection`, Godot's `get_property_list()`) and hashes what it finds. This trades integration cost for runtime cost and a real correctness hazard class (below), so it's layered on top of the explicit API, never a replacement for it.

## The one rule that survives reflection

Opt-in, same as the [manual derive macro](../cookbook/README.md#5-per-field-hashing) — reflection is never "walk every field reachable from the root," it's "walk fields the game has tagged as hashable" via the engine's own attribute system (`[FoldbackHash]` in C#, `@export_foldback` in Godot, `#[derive(FoldbackHash)]` in Bevy). Reflection is an alternate *producer* of the same `FieldHash` frame — not a new frame type or a separate bisection code path.

## Determinism hazards specific to reflection

The reason this stays a stretch goal, not a fast-follow:

- **Unordered containers**: a reflected `HashMap`/`Dictionary` has no guaranteed cross-platform iteration order. The walker must sort map/set entries by key before hashing, unconditionally, in every binding — a shared rule worth one conformance test in `foldback-core`, not three separate ad-hoc implementations.
- **Cycles and shared references**: needs a visited-set guard (by identity) and a fixed max depth, so a runaway object graph fails loudly rather than hanging or stack-overflowing.
- **Enum/union representation**: hash the discriminant *and* payload in a fixed encoding — never the host language's raw memory layout, which can differ across compilers/platforms.
- **Schema drift**: if a type's tagged-field set changes between the build that recorded a session and the build now analyzing it, the walker must detect the mismatch via the `.foldback` header's `build_id` and warn, rather than silently comparing apples to oranges.

## Visibility tooling

Because reflection makes it easy to lose track of *what* is actually being hashed, each binding is meant to ship a preview surface before this is trustworthy: a `foldback lint --engine <x>` command listing tracked fields per type (plus untagged siblings, heuristically), and an editor-integration live view of what's actually being captured on the selected entity. This ships alongside the *first* engine's reflective hashing, not after — shipping invisible auto-hashing even once is exactly the trust-eroding failure this feature's design guards against.

## Performance

Not assumed free, unlike the manual API. Each binding's demo project is meant to carry a benchmark comparing explicit vs. reflective hashing cost per tick, checked into CI as a regression benchmark. Default posture: a dev/editor-build feature, off by default in release builds.
