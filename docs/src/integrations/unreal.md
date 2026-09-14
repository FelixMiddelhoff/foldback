# Unreal Engine

## Status

**Not started.** Planned for Phase 5, sequenced last on purpose — Unreal is the heaviest of the four bindings by design (its own build system, its own reflection model, no first-party lockstep pattern to hook, unlike GGRS for Rust). Will ship as `UFoldbackSubsystem` (C++) with Blueprint wrappers, followed by `UPROPERTY(meta=(FoldbackHash))` reflective hashing, then Mass Entity integration once the basic binding is proven.

## Who this will be for

Unreal projects using a custom lockstep implementation, or (once the Mass Entity integration lands) Mass Entity-based simulations specifically.

## Full detail

The complete design — why Unreal is sequenced last, the reflection-model differences from other engines, the Mass Entity vs. custom-lockstep integration paths, and Fab distribution — lives in the project's dedicated Unreal binding plan, not duplicated here until the binding actually exists to document.
