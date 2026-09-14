# Unreal Engine

## Status

**Shipped, all three slices** (Phase 5 complete). `bindings/unreal` wraps `foldback-sys`'s C ABI: `UFoldbackSubsystem` (a `UGameInstanceSubsystem`, Blueprint-callable) delegates to `FFoldbackSession`, a plain C++ RAII wrapper covering the full Level 1/2/3 API. Verified against a real, locally-built Unreal Engine 5.8.2 — `examples/unreal-demo`'s headless Automation Test (`Foldback.Verify`) passes divergence detection, Level 2/3 recording (round-tripped through a real `.foldback` file and independently re-verified with `foldback-cli`), `finish()` agreement/disagreement, and the unconfigured-session error path.

**Reflective hashing** (`UPROPERTY(meta=(FoldbackHash))`, design doc §5): `FFoldbackReflectiveHasher` walks tagged top-level fields via `TFieldIterator<FProperty>`, cached per `UStruct`, and records each as one `FFoldbackSession::HashField` call. Handles numeric/bool/enum fields, `FVector`/`FRotator`/`FQuat` (special-cased), `TArray` (order preserved), `TMap`/`TSet` (sorted by key bytes per the shared unordered-container rule), nested `USTRUCT`s (recursed, depth-guarded), and `UObject*` (hashed via an opt-in `IFoldbackIdentifiable` interface, never a raw pointer). **A real, load-bearing limitation found by checking the engine's own macro rather than assuming**: `UPROPERTY(meta=(...))` tags are compiled out entirely when `WITH_METADATA` (== `WITH_EDITORONLY_DATA`) is 0 — i.e. in any Shipping/cooked-without-editor-data build. This binding's reflective path therefore only works in Editor/Development-Editor builds (exactly where this project's own headless Automation-Test verification already runs); it fails loudly with a logged error outside that config rather than silently hashing nothing. A Shipping game that needs field-level bisection should call `HashField` explicitly instead — reflective hashing stays additive tooling, never a required path, matching foldback-reflective-hashing.md §1's own framing. Conformance-tested end to end (`Foldback.ReflectionVerify`, 7 checks: opt-in filtering, map/set order-independence, array order sensitivity, nested-struct recursion, the depth guard, and both `UObject*` branches).

**Mass Entity integration** (design doc §6): `examples/unreal-mass-demo`'s `UFoldbackHashProcessor` (`FMassProcessor`) queries entities via `FMassEntityQuery`/`ForEachEntityChunk` and hashes each one's fragment data as Level 2 — the natural per-entity hook for a Mass-based project, parallel to `UTickFunction` for a custom fixed-tick loop. Verified headless (`Foldback.MassVerify`) against a real `FMassEntityManager` with 5 real entities across 20 ticks, recorded to a real `.foldback` file with an injected per-entity, per-tick divergence proving the hook reads live fragment data rather than a stale snapshot.

**No hosted CI leg** — a real, disclosed gap, not an oversight: Unreal Engine has no scriptable, license-free install path the way Unity (Hub CLI installer) and Godot (plain binary download) both do; only the Epic Games Launcher (GUI, requires login). A CI leg would need a self-hosted runner with Unreal pre-installed and licensed. See `bindings/unreal/README.md` for the full story.

## Who this is for

Unreal projects using a custom lockstep implementation today; Mass Entity-based simulations once that integration path lands.

## Full detail

Design rationale — why Unreal was sequenced last, the reflection-model differences from other engines, the Mass Entity vs. custom-lockstep integration paths, and Fab distribution — lives in the project's dedicated Unreal binding plan.
