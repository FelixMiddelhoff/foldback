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

Three independently useful layers, each shippable on its own:

1. **Core library** (`foldback-core`, Rust) — hashing, snapshot storage, bisection algorithm, session file format. **Shipped.**
2. **CLI** (`foldback-cli`) — thin binary over the core, produces text/JSON reports, drives CI failures. **Shipped** (`analyze`, `ci-check`, `lint`, `schema-diff`; `record` still deferred — see [CLI Reference](../usage/cli.md)).
3. **UI** (`foldback-ui`, Tauri + web frontend) — visual timeline, drill-down diff view, live or replay mode. **Shipped**, including live mode.

Engine bindings sit outside the core repo boundary conceptually but ship from the same monorepo:

- `foldback-sys` — raw C ABI header + Rust FFI crate (source of truth for the header, generated via `cbindgen`). **Shipped** (Level 1/2/3, plus schema-drift's `foldback_record_schema`).
- `foldback-rs` — idiomatic Rust wrapper, GGRS/Bevy integration helpers, `bevy`/`bevy-debug-panel` reflective-hashing features. **Shipped.**
- Unity (`bindings/unity`), Godot (`bindings/godot`), and Unreal (`bindings/unreal`) bindings — **all shipped**, including reflective hashing (every engine, including Bevy, now has a walker — see [Auto/Reflective Hashing](../integrations/reflective-hashing.md)), a real in-editor visibility dock per engine, and Unreal's Mass Entity slice. See their respective [integration pages](../integrations/rust-ggrs.md).

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
    foldback-ui/     # the Tauri app — shipped
  bindings/
    unity/           # shipped — UPM package, Runtime/ + Editor/
    godot/           # shipped — GDExtension addon (built from crates/foldback-godot)
    unreal/          # shipped
  docs/              # this site
  examples/
    minimal-rust/       # shipped — smallest integration, no engine
    ggrs-demo/          # shipped
    live-demo/          # shipped — live-mode transport end to end
    bevy-editor-demo/   # shipped — a real bevy_egui in-editor dock
    unity-demo/         # shipped
    godot-demo/         # shipped
    unreal-demo/        # shipped — custom fixed-tick lockstep integration
    unreal-mass-demo/   # shipped — Mass Entity integration
```

## Why Rust for the core

- Bit-for-bit determinism concerns already dominate this audience — Rust's tooling culture (and this project's own `determinism` CI job) matches that.
- `#[no_mangle] extern "C"` gives a C ABI for Unity/Godot/anything else without a second implementation.
- xxHash3, zstd, and the serialization ecosystem all have mature Rust crates.

## Generated API reference

Not published yet (pre-crates.io) — see the [Protocol Spec](protocol-spec.md#rust-api-reference) for the linking-out convention this site follows once it is.
