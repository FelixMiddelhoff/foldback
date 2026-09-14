# Godot

## Status

**Shipped** (Phase 4). `bindings/godot` is a GDExtension addon built directly on [`gdext`](https://godot-rust.github.io) (godot-rust's Rust bindings for Godot 4) — no separate C ABI layer, unlike the Unity binding, since `gdext` generates the GDExtension registration itself. Exposes a GDScript-native `FoldbackSession` class. Verified end to end against a real, headless Godot 4.7.2 engine in CI (Level 1 divergence detection, Level 2/3 hashing round-tripped through a real `.foldback` file, `finish()` agreement/disagreement, and the unconfigured-session error path).

## Using it

See `bindings/godot/README.md` for setup steps, and [Cookbook recipe 9](../cookbook/README.md#9-godot-integration) for the API shape and its one deviation from the original sketch (config goes through `.configure(dict)`, not `.new(dict)` — a real GDExtension constructor constraint, not a design choice).

## Who this is for

Godot projects (GDScript or C#) using a lockstep or rollback netcode approach.
