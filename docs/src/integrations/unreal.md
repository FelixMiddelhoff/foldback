# Unreal Engine

## Status

**Shipped — first slice** (Phase 5). `bindings/unreal` wraps `foldback-sys`'s C ABI: `UFoldbackSubsystem` (a `UGameInstanceSubsystem`, Blueprint-callable) delegates to `FFoldbackSession`, a plain C++ RAII wrapper covering the full Level 1/2/3 API. Verified against a real, locally-built Unreal Engine 5.8.2 — `examples/unreal-demo`'s headless Automation Test (`Foldback.Verify`) passes divergence detection, Level 2/3 recording (round-tripped through a real `.foldback` file and independently re-verified with `foldback-cli`), `finish()` agreement/disagreement, and the unconfigured-session error path.

Not yet built: `UPROPERTY(meta=(FoldbackHash))` reflective hashing, and the Mass Entity integration path — both planned as the next slices within this phase, per the binding plan's own sequencing (explicit hashing first, reflective as the immediate follow-up, Mass Entity once the basic binding is proven — which it now is).

**No hosted CI leg** — a real, disclosed gap, not an oversight: Unreal Engine has no scriptable, license-free install path the way Unity (Hub CLI installer) and Godot (plain binary download) both do; only the Epic Games Launcher (GUI, requires login). A CI leg would need a self-hosted runner with Unreal pre-installed and licensed. See `bindings/unreal/README.md` for the full story.

## Who this is for

Unreal projects using a custom lockstep implementation today; Mass Entity-based simulations once that integration path lands.

## Full detail

Design rationale — why Unreal was sequenced last, the reflection-model differences from other engines, the Mass Entity vs. custom-lockstep integration paths, and Fab distribution — lives in the project's dedicated Unreal binding plan.
