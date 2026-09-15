# Godot

## Overview

`bindings/godot` is a GDExtension addon built directly on [`gdext`](https://godot-rust.github.io) (godot-rust's Rust bindings for Godot 4) — no separate C ABI layer, unlike the Unity binding, since `gdext` generates the GDExtension registration itself. Exposes a GDScript-native `FoldbackSession` class. Verified end to end against a real, headless Godot 4.7.2 engine in CI (Level 1 divergence detection, Level 2/3 hashing round-tripped through a real `.foldback` file, `finish()` agreement/disagreement, and the unconfigured-session error path).

## Using it

See `bindings/godot/README.md` for setup steps, and [Cookbook recipe 9](../cookbook/README.md#9-godot-integration) for the API shape and its one deviation from the original sketch (config goes through `.configure(dict)`, not `.new(dict)` — a real GDExtension constructor constraint, not a design choice).

## Reflective hashing

Landed: `FoldbackSession.hash_reflected(tick, entity_id, prefix, target)` walks every `foldback_`-prefixed property reachable from `target` — no hand-written `hash_field` calls per field. `FoldbackSession.list_tracked(target)` is the visibility-tooling data source.

Two things worth knowing, both verified against gdext 0.5's actual source rather than assumed:

- **The marker is a naming convention (`foldback_` prefix), not a custom `@export_foldback` hint** — the plan flagged this as an open question needing a spike; checked directly against `PropertyHint` (a fixed engine enum) and GDExtension's registration surface, and there's no way to add a new hint value the editor/`@export` would recognize from a GDExtension addon. That would need an engine-side (C++) change. The naming convention isn't a fallback here, it's the only mechanism GDExtension can actually build.
- **The opt-in check re-applies at every nested `Object`, not just the root** — a deliberate difference from the Bevy/Unity walkers (which filter once at the root and then walk everything beneath a tagged field unconditionally). Doing that here would mean tagging one property on a `Node` pulls in that whole node's entire engine property surface (scripts, editor metadata, internal state) — too noisy for Godot's heavier object model. So a `foldback_`-prefixed property one level down still needs its own `foldback_` prefix.

Also verified directly from `godot-core`'s own source (not assumed): Godot's `Dictionary` is explicitly insertion-ordered and `Array` is an ordered vector by construction, so — unlike the Bevy/Unity walkers — this one doesn't sort container entries before hashing; there's no unordered-iteration hazard to guard against for either of Godot's built-in compound `Variant` types.

Depth-guarded (default 8) and cycle-guarded via `Gd<T>::instance_id()` (a real identity check, same soundness reasoning as the Unity walker — Godot `Object`s are reference types that can form genuine cycles).

Verified end to end against the same real, headless Godot 4.7.2 engine as the rest of this page — `examples/godot-demo/test.gd` exercises the walker, `list_tracked`, cycle detection, the depth guard, and an explicit-vs-reflective timing comparison, CI-gated against a ratio ceiling. Reflective hashing runs roughly 13-19x explicit's cost for 1,000 entities — noticeably higher overhead than Bevy's ~3.7x or Unity's ~3.5x, most likely `get_property_list()`'s full property-array-of-dictionaries construction per call.

See [Auto/Reflective Hashing](reflective-hashing.md) for the full cross-engine picture.

## Who this is for

Godot projects (GDScript or C#) using a lockstep or rollback netcode approach.
