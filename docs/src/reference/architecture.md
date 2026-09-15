# Architecture

## Vision

A standalone, engine-agnostic desync detection and debugging toolkit for lockstep/rollback multiplayer games — the same "small embedded hook + separate visualizer app" model as Tracy Profiler or RenderDoc. Built to be the default answer to "how do I find my desync bug," rather than every studio reinventing a `SyncCoordinator`/`SyncTestSession` in-house.

Target audience: indie to mid-size multiplayer game developers (Rust/GGRS, Unity, Godot, custom engines), plus anyone building deterministic simulations (RTS, fighting games, physics-based competitive games).

## Overview

```
┌─────────────────────┐     hash stream / snapshots     ┌──────────────────────┐
│   Game process(es)   │ ───────────────────────────────▶│  .foldback file OR    │
│  (engine binding →   │        (local file OR            │  live WS connection   │
│   core lib, C ABI)   │         live socket)             │                        │
└─────────────────────┘                                   └──────────┬───────────┘
                                                                       │
                                                            ┌──────────▼───────────┐
                                                            │   Foldback UI (Tauri)  │
                                                            │  timeline + bisector  │
                                                            │  + diff viewer        │
                                                            └───────────────────────┘
                                                            ┌───────────────────────┐
                                                            │   foldback-cli         │
                                                            │  CI mode, text report  │
                                                            └───────────────────────┘
```

Three independently useful layers:

1. **Core library** (`foldback-core`, Rust) — hashing, snapshot storage, bisection algorithm, session file format.
2. **CLI** (`foldback-cli`) — thin binary over the core: `analyze`, `ci-check`, `lint`, `schema-diff` (see [CLI Reference](../usage/cli.md)).
3. **UI** (`foldback-ui`, Tauri + web frontend) — visual timeline, drill-down diff view, live and offline mode.

Engine bindings sit outside the core repo boundary conceptually but ship from the same monorepo:

- `foldback-sys` — raw C ABI header + Rust FFI crate (source of truth for the header, generated via `cbindgen`) — Level 1/2/3 hashing, plus schema-drift's `foldback_record_schema`.
- `foldback-rs` — idiomatic Rust wrapper, GGRS/Bevy integration helpers, `bevy`/`bevy-debug-panel` reflective-hashing features.
- Unity (`bindings/unity`), Godot (`bindings/godot`), and Unreal (`bindings/unreal`) bindings — each with reflective hashing (see [Auto/Reflective Hashing](../integrations/reflective-hashing.md)) and a real in-editor visibility dock; Unreal also has a Mass Entity integration. See their respective [integration pages](../integrations/rust-ggrs.md).

## Repo layout

```
foldback/
  crates/
    foldback-core/   # hashing, snapshot, bisection, session file format
    foldback-sys/    # C ABI surface (generates foldback.h via cbindgen)
    foldback-rs/     # idiomatic Rust wrapper + GGRS/Bevy helpers
    foldback-cli/    # the `foldback` binary
    foldback-derive/ # #[derive(FoldbackHash)] proc macro
    foldback-godot/  # the Godot GDExtension crate (calls foldback-core directly)
    foldback-ui/     # the Tauri app
  bindings/
    unity/           # UPM package, Runtime/ + Editor/
    godot/           # GDExtension addon (built from crates/foldback-godot)
    unreal/
  docs/              # this site
  examples/
    minimal-rust/       # smallest integration, no engine
    ggrs-demo/
    live-demo/          # live-mode transport end to end
    bevy-editor-demo/   # a real bevy_egui in-editor dock
    unity-demo/
    godot-demo/
    unreal-demo/        # custom fixed-tick lockstep integration
    unreal-mass-demo/   # Mass Entity integration
```

## Why Rust for the core

- Bit-for-bit determinism concerns already dominate this audience — Rust's tooling culture (and this project's own `determinism` CI job) matches that.
- `#[no_mangle] extern "C"` gives a C ABI for Unity/Godot/anything else without a second implementation.
- xxHash3, zstd, and the serialization ecosystem all have mature Rust crates.

## Generated API reference

Not published yet (pre-crates.io) — see the [Protocol Spec](protocol-spec.md#rust-api-reference) for the linking-out convention this site follows once it is.
