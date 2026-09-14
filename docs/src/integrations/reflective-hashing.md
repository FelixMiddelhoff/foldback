# Auto/Reflective Hashing

**Status: stretch goal, Bevy and Unity done.** Sequenced per engine, after that engine's explicit-hashing binding ships. Bevy (`foldback-rs`'s `bevy`/`bevy-debug-panel` features) and Unity (`bindings/unity`'s `FoldbackReflection`) both have the shared sorted-container rule, a walker, per-field opt-in enforcement, a depth guard, visibility tooling, and a benchmark landed — see below for what's genuinely closed vs. deliberate non-goals vs. still-open per engine. Godot follows in its own phase order and hasn't started.

## Using it today

### Bevy

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

### Unity

```csharp
using Foldback;

class Unit
{
    [FoldbackHash] public Position Pos;   // compound — walked fully once reached
    [FoldbackHash] public int Hp;         // primitive — hashed directly
    public string DebugLabel;             // unmarked — invisible to the walker
}

var preview = FoldbackReflection.HashReflected(session, tick, entityId, "unit", unit);
```

One attribute, `[FoldbackHash]`, covers both the primitive and compound cases (no `hash`/`reflect` split like the Rust derive needs — C# reflection doesn't require a `FieldBytes`-equivalent bound to walk further). Same field-path naming and return-value shape as the Bevy walker.

## What it's for

Get Level 2/3 (per-entity, per-field) bisection *without* hand-writing `hash_entity`/`hash_field` calls — the binding walks the engine's own reflection system (`bevy_reflect`, C# `System.Reflection`, Godot's `get_property_list()`) and hashes what it finds. This trades integration cost for runtime cost and a real correctness hazard class (below), so it's layered on top of the explicit API, never a replacement for it.

## The one rule that survives reflection — enforced, not just documented

Opt-in, same as the [manual derive macro](../cookbook/README.md#5-per-field-hashing) — reflection is never "walk every field reachable from the root," it's "walk fields the game has tagged as hashable" via the engine's own attribute system (`[FoldbackHash]` in C#, `@export_foldback` in Godot, `#[derive(FoldbackHash)]` in Bevy). Reflection is an alternate *producer* of the same `FieldHash` frame — not a new frame type or a separate bisection code path. Once a tagged member is reached, everything reachable beneath it is walked without needing its own type separately tagged — the opt-in boundary is per top-level member, the same model in every engine that's landed so far, so it reads the same whichever binding a project uses.

- **Bevy**: `hash_reflected` requires `root`'s type to implement `FoldbackHash` and only walks the root struct's fields named in `T::TRACKED_FIELDS` (`#[foldback(hash)]` or `#[foldback(reflect)]`).
- **Unity**: `FoldbackReflection.HashReflected` only walks `root`'s own type's `[FoldbackHash]`-tagged fields/properties (public or non-public — the attribute check itself doesn't care about visibility).

## Determinism hazards specific to reflection

The reason this stays a stretch goal, not a fast-follow:

- **Unordered containers**: a reflected `HashMap`/`Dictionary` has no guaranteed cross-platform iteration order. **Landed for both**: `foldback_core::hashable::FieldBytes` sorts `HashMap`/`HashSet` by key before hashing (with `BTreeMap`/`BTreeSet` impls for symmetry), covered by a conformance test — the one shared rule every reflective walker bottoms out on. The Bevy walker applies the same rule to reflected `Map`/`Set` values (sorted by rendered key text); the Unity walker sorts `IDictionary`/`ISet` entries the same way.
- **Cycles and shared references**: needs a fixed max depth so a runaway object graph fails loudly rather than hanging or stack-overflowing, plus (per the original design) an identity-based visited-set for genuine reference cycles. **Landed differently per engine, deliberately**:
  - *Bevy*: depth guard only (default 8, `Error::ReflectionDepthExceeded`). An identity-based guard was tried and removed — a struct's first field shares its raw address with the struct itself (zero offset), so naive pointer-identity dedup flags *every* struct's first field as a false-positive cycle back to its own parent. More fundamentally, a plain `bevy_reflect` walk only ever sees owned value trees; Rust's ownership model makes a value structurally unable to contain itself, so a genuine reference cycle can't arise this way. The depth guard alone is correct and sufficient for Bevy.
  - *Unity*: depth guard (default 8) **and** an identity-based visited-set (`FoldbackReflectionException` for either) — sound here, unlike Bevy, because C# reference types genuinely can hold references to each other and form a real cycle (`RuntimeHelpers.GetHashCode` + reference equality, skipped for value types since a struct is copied by value and has no meaningful identity to track).
- **Enum/union representation**: hash the discriminant *and* payload in a fixed encoding — never the host language's raw memory layout, which can differ across compilers/platforms. **Landed for Bevy** (hashes `variant_name()` plus each variant field). **Naturally absent for Unity/C#**: a C# `enum` carries no payload (unlike a Rust enum variant), so there's no discriminant/payload split to encode — the walker hashes the underlying integral value directly, which is already a fixed, cross-platform-stable representation.
- **Schema drift**: if a type's tagged-field set changes between the build that recorded a session and the build now analyzing it, the walker should detect the mismatch via the `.foldback` header's `build_id` and warn. **Not yet built for any binding** — open across the board.

## Visibility tooling — landed for both

- **Bevy**: `foldback lint --engine bevy <path>` statically scans `.rs` files (recursing into `mod` blocks) for `#[derive(FoldbackHash)]` structs and prints each one's tracked fields alongside its untagged siblings. `foldback_rs::bevy::debug::ReflectionPreview` + `render(&egui::Context, ...)` (feature `bevy-debug-panel`) draws a live table a game's own `egui`/`bevy_egui` integration calls into — headless-tested (`egui::Context::run_ui`, no window).
- **Unity**: `FoldbackReflection.ListTracked(Type)` returns the same tracked/untracked-sibling split for a given type — pure data, unit-tested. A real Unity `EditorWindow` consuming it (the "editor-integration live view" the original design called for) **hasn't been built** — building and verifying one needs the actual Unity Editor UI, which this project's headless/CI-only verification approach doesn't cover; `ListTracked` is the piece that's ready for one to be built on.
- **Godot**: not started.

## Performance — landed for both

- **Bevy**: `crates/foldback-rs/benches/reflective_vs_explicit.rs` (Criterion, `cargo bench -p foldback-rs --features bevy`) compares explicit vs. reflective cost for 100/1,000 entities. Measured locally: reflective ran ~3.7x the explicit path's cost at both counts.
- **Unity**: `bindings/unity/Tests~/FoldbackSys.Tests` prints (doesn't assert — no regression gate, matching Bevy) an explicit-vs-reflective timing comparison for 1,000 entities. Measured locally against the plain .NET (Mono-equivalent) path: reflective ran ~3.5x the explicit path's cost — consistent with Bevy's number, both confirming the plan's expectation that this is real, non-trivial overhead, not "basically free."

Neither benchmark is wired into CI as a regression gate yet — open for both. Default posture (dev/editor-build feature, off by default in release builds) is a per-game build-configuration choice, not something either crate enforces.

## Open, engine-specific risks

- **Unity + IL2CPP AOT — genuinely unconfirmed.** `FoldbackReflection`'s per-type accessor caching uses `System.Linq.Expressions.Expression.Compile()`, which needs JIT/codegen on most .NET runtimes; IL2CPP has no `DynamicMethod`/`Reflection.Emit`, so this is expected to fall back to the BCL's expression *interpreter* instead — plausible, but not yet exercised against a real IL2CPP player the way the rest of the Unity binding's Level 1/2/3 surface has been (see [Unity](unity.md)). A check for this has been added to `examples/unity-demo/Assets/Il2cppVerify.cs` (the same file the existing `bindings-unity-il2cpp` CI leg runs), but confirming it actually passes needs a real Unity Editor + IL2CPP build, which wasn't available in the session that wrote it. Tracked the same way the risk register's P1 already tracks this class of unknown — verify before relying on it in a shipping IL2CPP build.
