# Rust / GGRS / Bevy

## Who this is for

Any Rust game using a lockstep or rollback netcode library — GGRS specifically has a first-class integration, since Foldback can ride the checksum a GGRS game already computes instead of adding a second simulation hook.

## Status

The core `foldback-core` API (Level 1 hashing, the `Session` type) ships today and works from any Rust project with no GGRS dependency — see the [Quickstart](../getting-started/quickstart.md). `foldback-rs`'s `ggrs` feature (Phase 2) ships two helpers, `checksum` and `record_desync` — see below. `examples/ggrs-demo` proves a real GGRS `SyncTestSession` end to end, recording a real two-peer `.foldback` file the UI can open.

## Install

```toml
[dependencies]
foldback-core = { git = "https://github.com/FelixMiddelhoff/foldback", package = "foldback-core" }
foldback-rs = { git = "https://github.com/FelixMiddelhoff/foldback", package = "foldback-rs", features = ["ggrs"] }
```

## Minimal example

See [Cookbook recipe 1](../cookbook/README.md#1-minimal-integration) — works today with any Rust simulation, GGRS or not.

## GGRS-specific integration

See [Cookbook recipe 3](../cookbook/README.md#3-ggrsbevy-integration) — GGRS has no hookable "checksum callback" to wrap (a game computes its own checksum and hands it to `GameStateCell::save`), so the real integration is two small helpers rather than a session-wrapping extension trait: `checksum(state_bytes)` so a game's own GGRS checksum *is* Foldback's hash, and `record_desync(..)` to feed GGRS's own `DesyncDetected` event (from a real networked `P2pSession`) into a Foldback `Session` for recording/bisection. `SyncTestSession` (single-process — see `examples/ggrs-demo`) doesn't need the bridge; it catches mismatches internally.

## Reflective hashing

Landed: `foldback-rs`'s `bevy` feature walks `bevy_reflect` component fields marked `#[foldback(hash)]`/`#[foldback(reflect)]` and hashes them without hand-written `hash_entity`/`hash_field` calls, plus a `bevy-debug-panel` feature for an in-`egui` live view of what got captured. See [Auto/Reflective Hashing](reflective-hashing.md) for the full picture, including what's a deliberate non-goal rather than unfinished.

## CI integration

`foldback ci-check <file>` works today for a file-based check. See [CI Integration](../usage/ci-gate.md).

## Troubleshooting

Nothing specific yet — file an issue if you hit something integrating against the current API; this section grows from real reports rather than speculation.
