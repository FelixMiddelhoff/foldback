# Auto/Reflective Hashing

**Status: stretch goal, Bevy done.** Sequenced per engine, after that engine's explicit-hashing binding ships. Bevy (`foldback-rs`'s `bevy`/`bevy-debug-panel` features) has the shared sorted-container rule, the walker, per-field opt-in enforcement, the depth guard, `foldback-cli lint`, an `egui` preview panel, and a benchmark all landed — see below for what's genuinely closed vs. what's a deliberate, documented non-goal. Unity/Godot follow in their existing phase order and haven't started.

## Using it today (Bevy)

```rust
use foldback_rs::bevy::hash_reflected;

// `Unit` derives both `Reflect` and `FoldbackHash`. `#[foldback(hash)]`
// marks a primitive field (hashed via `FieldBytes`, same as the manual
// API); `#[foldback(reflect)]` marks a compound field (a nested struct,
// a `Vec`, a `HashMap`) for the walker to recurse into instead — it
// doesn't need `FieldBytes`, since the walker gets bytes via reflection.
#[derive(Reflect, FoldbackHash)]
struct Unit {
    #[foldback(reflect)]
    pos: Position,
    #[foldback(hash)]
    hp: i32,
    // unmarked: invisible to both the manual API and hash_reflected.
    debug_label: String,
}

let preview: Vec<(String, u64)> = hash_reflected(&mut session, tick, entity_id, "unit", &unit)?;
```

Walks `Unit::TRACKED_FIELDS` (`pos`, `hp` — not `debug_label`) and everything reachable beneath them, recording each leaf as a `FieldHash` frame, field-path-named (`"unit.pos.x"`, `"unit.tags[1]"`). Returns the same `(path, hash)` pairs it recorded — feed that to `foldback_rs::bevy::debug::ReflectionPreview` for a live view (below).

## What it's for

Get Level 2/3 (per-entity, per-field) bisection *without* hand-writing `hash_entity`/`hash_field` calls — the binding walks the engine's own reflection system (`bevy_reflect`, C# `System.Reflection`, Godot's `get_property_list()`) and hashes what it finds. This trades integration cost for runtime cost and a real correctness hazard class (below), so it's layered on top of the explicit API, never a replacement for it.

## The one rule that survives reflection — enforced, not just documented

Opt-in, same as the [manual derive macro](../cookbook/README.md#5-per-field-hashing) — reflection is never "walk every field reachable from the root," it's "walk fields the game has tagged as hashable" via the engine's own attribute system (`[FoldbackHash]` in C#, `@export_foldback` in Godot, `#[derive(FoldbackHash)]` in Bevy). Reflection is an alternate *producer* of the same `FieldHash` frame — not a new frame type or a separate bisection code path.

**Bevy**: `hash_reflected` requires `root`'s type to implement `FoldbackHash` and only walks the root struct's fields named in `T::TRACKED_FIELDS` (`#[foldback(hash)]` or `#[foldback(reflect)]`) — an untagged top-level field is invisible to the walker, not merely undocumented as tracked. Once inside a tagged field, the walk recurses into everything reachable from it (matching the Unreal binding's model): the opt-in boundary is per top-level field, not per leaf inside it.

## Determinism hazards specific to reflection

The reason this stays a stretch goal, not a fast-follow:

- **Unordered containers**: a reflected `HashMap`/`Dictionary` has no guaranteed cross-platform iteration order. **Landed**: `foldback_core::hashable::FieldBytes` sorts `HashMap`/`HashSet` by key before hashing (with `BTreeMap`/`BTreeSet` impls for symmetry), covered by a conformance test — the one shared rule all three bindings' reflective walkers bottom out on. The Bevy walker applies the same rule directly to reflected `Map`/`Set` values (sorted by their rendered key text).
- **Cycles and shared references**: needs a fixed max depth so a runaway object graph fails loudly rather than hanging or stack-overflowing. **Landed for Bevy, deliberately without a separate identity-based visited-set**: the walker enforces a depth guard (default 8, `Error::ReflectionDepthExceeded`). An identity-based guard was tried and removed — a struct's first field shares its raw address with the struct itself (zero offset), so naive pointer-identity dedup flags *every* struct's first field as a false-positive cycle back to its own parent. More fundamentally, a plain `bevy_reflect` walk only ever sees owned value trees; Rust's ownership model makes a value structurally unable to contain itself, so a genuine reference cycle can't arise this way — it would need an actual indirection type (`Box<dyn Reflect>`, `Rc`) walked as a plain nested value, which this walker doesn't do. The depth guard alone is therefore correct and sufficient for Bevy. Unity/Godot, whose engines expose real object-reference graphs, will need an actual object-identity check (not raw-address) when their turn comes — `Error::ReflectionCycleDetected` exists in `foldback-core` for that, unused by Bevy on purpose.
- **Enum/union representation**: hash the discriminant *and* payload in a fixed encoding — never the host language's raw memory layout, which can differ across compilers/platforms. **Landed**: the Bevy walker hashes `variant_name()` plus each variant field, dotted-path-named.
- **Schema drift**: if a type's tagged-field set changes between the build that recorded a session and the build now analyzing it, the walker should detect the mismatch via the `.foldback` header's `build_id` and warn. **Not yet built for any binding** — open across the board, not Bevy-specific.

## Visibility tooling — landed for Bevy

- **`foldback lint --engine bevy <path>`**: statically scans `.rs` files (recursing into `mod` blocks) for `#[derive(FoldbackHash)]` structs and prints each one's tracked fields alongside its untagged siblings — the "did you mean to include this one too" check. Doesn't descend into function bodies (a struct local to a test or function isn't the reusable component type this tool is for). Unity/Godot scanners for their own marker syntax (`[FoldbackHash]`, `@export_foldback`) aren't built yet.
- **`foldback_rs::bevy::debug`**: `ReflectionPreview` wraps a `hash_reflected` call's own `(path, hash)` return value — the actual captured data, not a re-derived guess. Feature `bevy-debug-panel` adds `render(&egui::Context, &ReflectionPreview)`, a small table a game's own `egui`/`bevy_egui` integration draws directly (this crate doesn't depend on `bevy_egui` itself, to stay usable from any `egui`-driven host). Headless-tested (`egui::Context::run_ui` with no window).

## Performance — landed for Bevy

`crates/foldback-rs/benches/reflective_vs_explicit.rs` (Criterion, `cargo bench -p foldback-rs --features bevy`) compares explicit vs. reflective hashing cost for 100 and 1,000 entities per tick, with no `.foldback` file attached so it measures the hash/walk cost itself, not I/O. Not checked into CI as a regression gate yet — that's still open. Measured locally: reflective hashing ran roughly 3.7x the explicit path's cost at both entity counts, consistent with the plan's expectation that it's real, non-trivial overhead, not "basically free." Default posture (dev/editor-build feature, off by default in release builds) is a per-game build-configuration choice, not something this crate enforces — documented here as guidance, not code.
