# Rust / GGRS / Bevy

## Who this is for

Any Rust game using a lockstep or rollback netcode library — GGRS specifically has a first-class integration planned, since Foldback can ride its existing checksum callback instead of adding a second simulation hook.

## Status

The core `foldback-core` API (Level 1 hashing, the `Session` type) ships today and works from any Rust project with no GGRS dependency — see the [Quickstart](../getting-started/quickstart.md). The `foldback-rs` GGRS-specific extension trait (`attach_foldback`) is planned for Phase 2; `foldback-rs` currently exists as an empty crate skeleton.

## Install

```toml
[dependencies]
foldback-core = { git = "https://github.com/FelixMiddelhoff/foldback", package = "foldback-core" }
```

## Minimal example

See [Cookbook recipe 1](../cookbook/README.md#1-minimal-integration) — works today with any Rust simulation, GGRS or not.

## GGRS-specific integration (planned)

See [Cookbook recipe 3](../cookbook/README.md#3-ggrsbevy-integration) for the target shape: `foldback-rs` will hook GGRS's own checksum callback so there's no duplicate simulation loop to maintain.

## Reflective hashing

Not planned for the Rust/Bevy path in Phase 0/2 the way it is for engine bindings with a native reflection system — Bevy's own ECS query system is the natural mechanism if this is built later. See [Auto/Reflective Hashing](reflective-hashing.md).

## CI integration

`foldback ci-check <file>` works today for a file-based check. See [CI Integration](../usage/ci-gate.md).

## Troubleshooting

Nothing specific yet — file an issue if you hit something integrating against the current API; this section grows from real reports rather than speculation.
