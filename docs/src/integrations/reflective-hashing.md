# Auto/Reflective Hashing

**Status: stretch goal, all three engines done.** Sequenced per engine, after that engine's explicit-hashing binding ships — Bevy, then Unity, then Godot, in that order. Every binding has the walker, opt-in enforcement, a depth guard, visibility tooling, and a benchmark landed. See below for what's genuinely closed vs. deliberate non-goals vs. still-open per engine — the three walkers aren't identical, and the differences are load-bearing design decisions, not accidents.

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

### Godot

```gdscript
class Unit extends RefCounted:
    var foldback_pos: Position    # compound — walked fully once reached,
                                   # but its own fields need the prefix too
    var foldback_hp: int = 0      # primitive — hashed directly
    var debug_label: String = ""  # unmarked — invisible to the walker

var preview: Array = session.hash_reflected(tick, entity_id, "unit", unit)
```

No attribute system in GDScript, so the marker is a `foldback_` name prefix instead — see below for why that's not a fallback here, it's the only mechanism GDExtension can actually build. `preview` is an `Array` of `{"path": String, "hash": int}` dicts, same shape in spirit as the other two walkers' return value.

## What it's for

Get Level 2/3 (per-entity, per-field) bisection *without* hand-writing `hash_entity`/`hash_field` calls — the binding walks the engine's own reflection system (`bevy_reflect`, C# `System.Reflection`, Godot's `get_property_list()`) and hashes what it finds. This trades integration cost for runtime cost and a real correctness hazard class (below), so it's layered on top of the explicit API, never a replacement for it.

## The one rule that survives reflection — enforced, not just documented, but not identical per engine

Opt-in, same as the [manual derive macro](../cookbook/README.md#5-per-field-hashing) — reflection is never "walk every field reachable from the root," it's "walk fields the game has tagged as hashable" via the engine's own attribute system (`[FoldbackHash]` in C#, a `foldback_` name prefix in GDScript, `#[derive(FoldbackHash)]` in Bevy). Reflection is an alternate *producer* of the same `FieldHash` frame — not a new frame type or a separate bisection code path.

- **Bevy and Unity**: opt-in is checked once, at the root. `hash_reflected`/`HashReflected` filters the root type's own tagged fields; everything reachable beneath a tagged field is then walked unconditionally, without needing its own type separately tagged.
- **Godot deliberately does this differently**: the `foldback_` prefix check is re-applied at *every* nested `Object` the walk reaches, not just the root. Doing what Bevy/Unity do — tag one property, walk everything beneath it unconditionally — would mean tagging a single property on a `Node` pulls in that whole node's entire engine property surface (scripts, editor metadata, internal engine state), which a plain Rust struct or C# POCO simply doesn't carry. So a nested tracked object's own properties need their own `foldback_` prefix too. This is a real, considered difference in what "opt-in" means per engine, not an inconsistency — each choice fits the noise level of that engine's actual object model.

## The marker mechanism itself — resolved per engine, not assumed

- **Bevy**: `#[derive(FoldbackHash)]`'s `#[foldback(hash)]`/`#[foldback(reflect)]`, read at compile time.
- **Unity**: `[FoldbackHash]`, a real C# attribute, read via `System.Reflection` at runtime.
- **Godot — the plan's own §7 open question, now checked, not assumed**: the plan named two options, a custom `@export_foldback` property hint or a `foldback_` naming-convention fallback, and flagged the hint approach as needing a spike before committing to it over the fallback. Checked directly against gdext 0.5's generated API: `PropertyHint` is a fixed engine enum, and nothing in GDExtension's registration surface lets an addon add a *new* hint value the editor or `@export` would recognize — that needs an engine-side (C++) change, not something buildable from GDExtension. So the naming convention isn't a fallback here; it's the only mechanism that was ever actually available from this binding's position in the stack.

## Determinism hazards specific to reflection

The reason this stays a stretch goal, not a fast-follow:

- **Unordered containers**: a reflected `HashMap`/`Dictionary` has no guaranteed cross-platform iteration order — usually. **Landed for Bevy and Unity, verified *not applicable* for Godot**: `foldback_core::hashable::FieldBytes` sorts `HashMap`/`HashSet` by key before hashing (with `BTreeMap`/`BTreeSet` impls for symmetry), covered by a conformance test. The Bevy walker applies the same rule to reflected `Map`/`Set` values; the Unity walker sorts `IDictionary`/`ISet` entries the same way. The Godot walker checked `godot-core`'s own source rather than assuming the hazard applies universally, and found it explicitly documented there that Godot's `Dictionary` is insertion-ordered by design (and `Array` is an ordered vector by construction) — so neither of Godot's two built-in compound `Variant` types has this hazard, and the walker deliberately doesn't sort them.
- **Cycles and shared references**: needs a fixed max depth so a runaway object graph fails loudly rather than hanging or stack-overflowing, plus (per the original design) an identity-based visited-set for genuine reference cycles. **Landed differently per engine, deliberately**:
  - *Bevy*: depth guard only (default 8, `Error::ReflectionDepthExceeded`). An identity-based guard was tried and removed — a struct's first field shares its raw address with the struct itself (zero offset), so naive pointer-identity dedup flags *every* struct's first field as a false-positive cycle back to its own parent. More fundamentally, a plain `bevy_reflect` walk only ever sees owned value trees; Rust's ownership model makes a value structurally unable to contain itself, so a genuine reference cycle can't arise this way. The depth guard alone is correct and sufficient for Bevy.
  - *Unity*: depth guard (default 8) **and** an identity-based visited-set (`RuntimeHelpers.GetHashCode` + reference equality, skipped for value types) — sound here, unlike Bevy, because C# reference types genuinely can hold references to each other and form a real cycle.
  - *Godot*: depth guard (default 8) **and** an identity-based visited-set using `Gd<T>::instance_id()` — Godot's own built-in, engine-provided object identity, even more direct than Unity's runtime-hash-code approach. Also sound and necessary: Godot `Object`s are reference types that can genuinely reference each other cyclically (verified live: a two-node mutual-reference test correctly throws rather than hanging).
- **Enum/union representation**: hash the discriminant *and* payload in a fixed encoding — never the host language's raw memory layout, which can differ across compilers/platforms. **Landed for Bevy** (hashes `variant_name()` plus each variant field). **Naturally absent for Unity/C# and for Godot**: a C# `enum` carries no payload (unlike a Rust enum variant), so the walker hashes the underlying integral value directly; GDScript doesn't have Rust-style payload-carrying enums at all (Variant's own type tag already IS the discriminant, handled by the walker's normal type dispatch).
- **Schema drift**: if a type's tagged-field set changes between the build that recorded a session and the build now analyzing it, a reader needs to detect the mismatch rather than silently diffing two things that no longer mean the same thing. **Landed for all three engines**: `foldback_core::session::Session::record_schema(type_name, tracked_fields)` writes a `Metadata` frame (`foldback.schema.<type_name>` → sorted tagged-field fingerprint), once per type per session — exposed to Unity/Godot as `foldback_record_schema` in `foldback-sys`'s C ABI. `foldback_core::schema::detect_drift` compares two such fingerprint maps, and `foldback schema-diff <before.foldback> <after.foldback>` exposes it from the CLI, exiting 1 on drift (same convention as `ci-check`). Not `build_id`-based in the end — `build_id` is opaque game-supplied bytes with no defined internal structure to diff. Each binding's walker supplies the type name automatically:
  - **Bevy**: `std::any::type_name::<T>()`, a real static type — always unique.
  - **Unity**: `Type.FullName` of the reflected root, also a real static type — always unique. Deduped on the C# side too (`ConditionalWeakTable<FoldbackSession, HashSet<Type>>`), so a game calling `HashReflected` every tick doesn't re-marshal the field-name array across FFI past the first call per type per session.
  - **Godot**: no static type declarations exist to draw on (GDScript). `Script.get_global_name()` (a GDScript `class_name`) is used when present — checked live: it's the one identifier that's both build-stable and genuinely unique per type. Without a `class_name`, this falls back to `Object::get_class()` (the native engine class, e.g. `"RefCounted"`), which collapses every un-named inner-class script sharing that base class onto the same schema key — a documented limitation, not a silent wrong answer. Give a tracked GDScript type a `class_name` for schema-drift to mean anything for it.

## Visibility tooling — landed for all three, differently

- **Bevy**: `foldback lint --engine bevy <path>` statically scans `.rs` files (recursing into `mod` blocks) for `#[derive(FoldbackHash)]` structs and prints each one's tracked fields alongside its untagged siblings. `foldback_rs::bevy::debug::ReflectionPreview` + `render(&egui::Context, ...)` (feature `bevy-debug-panel`) draws a live table a game's own `egui`/`bevy_egui` integration calls into — headless-tested (`egui::Context::run_ui`, no window).
- **Unity**: `FoldbackReflection.ListTracked(Type)` returns the same tracked/untracked-sibling split for a given type — pure data, unit-tested. A real Unity `EditorWindow` consuming it hasn't been built (can't verify Editor UI headlessly).
- **Godot**: `FoldbackSession.list_tracked(target)` returns the same split for a live instance (not a static type — GDScript has no compile-time type declarations to scan the way Bevy's source or Unity's `Type` reflection do), as `{"tracked": PackedStringArray, "untracked": PackedStringArray}`. Verified against a real headless Godot run. No editor dock UI built on top of it.

## Performance — landed for all three, and the numbers actually differ

- **Bevy**: `crates/foldback-rs/benches/reflective_vs_explicit.rs` (Criterion, `cargo bench -p foldback-rs --features bevy`) compares explicit vs. reflective cost for 100/1,000 entities. Measured locally: reflective ran ~3.7x the explicit path's cost at both counts.
- **Unity**: `bindings/unity/Tests~/FoldbackSys.Tests` prints (doesn't assert) an explicit-vs-reflective timing comparison for 1,000 entities against the plain .NET (Mono-equivalent) path: ~3.5x, consistent with Bevy's number.
- **Godot**: `examples/godot-demo/test.gd` prints the same comparison for 1,000 entities, run against a real headless Godot 4.7.2: reflective ran roughly **13x** explicit's cost — noticeably higher than the other two. Most likely cause (not yet profiled further): `get_property_list()` builds a full `Array` of `Dictionary` objects — one Variant-boxed dictionary per property, every call — where Bevy's `bevy_reflect` and Unity's cached `Expression`-compiled getters both avoid that per-call allocation cost.

None of the three benchmarks is wired into CI as a regression gate yet. Default posture (dev/editor-build feature, off by default in release builds) is a per-game build-configuration choice, not something any of the three crates enforces.

## Open, engine-specific risks

- **Unity + IL2CPP AOT — confirmed.** `FoldbackReflection`'s per-type accessor caching uses `System.Linq.Expressions.Expression.Compile()`, which needs JIT/codegen on most .NET runtimes; IL2CPP has no `DynamicMethod`/`Reflection.Emit`, so this could plausibly have thrown or silently misbehaved under AOT. A check was added to `examples/unity-demo/Assets/Il2cppVerify.cs` and, on the next push, actually ran in the `bindings-unity-il2cpp` CI leg against a real IL2CPP player: both reflective-hashing checks passed. `Expression.Compile()` falls back to the BCL's expression interpreter under IL2CPP as expected, not a throw. See [Unity](unity.md).
- **Godot's ~13x overhead** is a real, measured number, not a guess — but the *cause* is inferred, not profiled. Worth a closer look before recommending reflective hashing for a Godot project with tight per-tick budgets.
