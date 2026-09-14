# Auto/Reflective Hashing

**Status: stretch goal, in progress.** Sequenced per engine, after that engine's explicit-hashing binding ships. The shared sorted-container rule (below) and the Bevy walker have landed (`foldback-rs`'s `bevy` feature, `crates/foldback-rs/src/bevy.rs`); Unity/Godot follow in their existing phase order. Not yet done for Bevy: the visibility tooling (`foldback lint`, an in-editor live view) and a benchmark harness in `examples/*-demo` — both still open.

## Using it today (Bevy)

```rust
use foldback_rs::bevy::hash_reflected;

// `unit` derives `Reflect` (and `FoldbackHash` if it also uses the manual
// API on some fields — the two can coexist per-type).
hash_reflected(&mut session, tick, entity_id, "unit", &unit)?;
```

Walks every field reachable from `unit` via `bevy_reflect` and records each as a `FieldHash` frame, field-path-named (`"unit.pos.x"`, `"unit.tags[1]"`) so bisection output stays readable. There's no per-field `#[foldback(hash)]` gate inside the walk itself yet — the opt-in boundary today is "did the game choose to call `hash_reflected` on this component," not a finer per-field marker within it; narrowing that to match the manual API's per-field opt-in is open (see `foldback-reflective-hashing.md` §7 in the planning repo).

## What it's for

Get Level 2/3 (per-entity, per-field) bisection *without* hand-writing `hash_entity`/`hash_field` calls — the binding walks the engine's own reflection system (`bevy_reflect`, C# `System.Reflection`, Godot's `get_property_list()`) and hashes what it finds. This trades integration cost for runtime cost and a real correctness hazard class (below), so it's layered on top of the explicit API, never a replacement for it.

## The one rule that survives reflection

Opt-in, same as the [manual derive macro](../cookbook/README.md#5-per-field-hashing) — reflection is never "walk every field reachable from the root," it's "walk fields the game has tagged as hashable" via the engine's own attribute system (`[FoldbackHash]` in C#, `@export_foldback` in Godot, `#[derive(FoldbackHash)]` in Bevy). Reflection is an alternate *producer* of the same `FieldHash` frame — not a new frame type or a separate bisection code path.

## Determinism hazards specific to reflection

The reason this stays a stretch goal, not a fast-follow:

- **Unordered containers**: a reflected `HashMap`/`Dictionary` has no guaranteed cross-platform iteration order. The walker must sort map/set entries by key before hashing, unconditionally, in every binding. **Landed**: `foldback_core::hashable::FieldBytes` now sorts `HashMap`/`HashSet` by key before hashing (with `BTreeMap`/`BTreeSet` impls for symmetry), covered by a conformance test — the one shared rule all three bindings' reflective walkers bottom out on. The Bevy walker applies the same rule directly to reflected `Map`/`Set` values (sorted by their rendered key text).
- **Cycles and shared references**: needs a visited-set guard (by identity) and a fixed max depth, so a runaway object graph fails loudly rather than hanging or stack-overflowing. **Partly landed**: the Bevy walker enforces the fixed max depth (default 8, `Error::ReflectionDepthExceeded`) — the identity-based visited-set guard for genuine reference cycles is still open (Bevy component graphs are normally acyclic value trees, so depth alone covers the common runaway case, but a true cycle through shared references isn't yet caught before it would hit the depth limit).
- **Enum/union representation**: hash the discriminant *and* payload in a fixed encoding — never the host language's raw memory layout, which can differ across compilers/platforms.
- **Schema drift**: if a type's tagged-field set changes between the build that recorded a session and the build now analyzing it, the walker must detect the mismatch via the `.foldback` header's `build_id` and warn, rather than silently comparing apples to oranges.

## Visibility tooling

Because reflection makes it easy to lose track of *what* is actually being hashed, each binding is meant to ship a preview surface before this is trustworthy: a `foldback lint --engine <x>` command listing tracked fields per type (plus untagged siblings, heuristically), and an editor-integration live view of what's actually being captured on the selected entity. This ships alongside the *first* engine's reflective hashing, not after — shipping invisible auto-hashing even once is exactly the trust-eroding failure this feature's design guards against.

## Performance

Not assumed free, unlike the manual API. Each binding's demo project is meant to carry a benchmark comparing explicit vs. reflective hashing cost per tick, checked into CI as a regression benchmark. Default posture: a dev/editor-build feature, off by default in release builds.
