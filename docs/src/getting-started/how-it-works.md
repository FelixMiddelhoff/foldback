# How Foldback Works

## The pieces

```
foldback-core   hashing, session file format, bisection engine (Rust)
foldback-sys    C ABI over foldback-core, generated foldback.h (cbindgen)
foldback-rs     idiomatic Rust wrapper + GGRS/Bevy helpers
foldback-cli    the `foldback` binary: analyze, ci-check (record: planned)
foldback-ui     Tauri app — timeline, peer comparison, bisection drill-down
bindings/       Unity, Unreal (foldback-sys's C ABI), Godot (direct gdext)
```

`foldback-core` is the only place the actual logic lives. Every other surface — the CLI, the UI, every engine binding — is a thin wrapper around it, so a bug fix or a bisection-engine improvement lands everywhere at once instead of needing to be ported four times. Godot's binding calls `foldback-core` directly through `gdext` rather than through `foldback-sys`'s C ABI, since `gdext` generates its own GDExtension registration — Unity and (when built) Unreal go through the C ABI because C#/C++ have no GDExtension-equivalent of their own.

## What exists today (Phases 0–5)

- `foldback-core`: xxHash3 hashing, the `.foldback` file format (read + write), a bounded retention ring buffer, zstd snapshot compression, Level 1/2/3 (per-tick/entity/field) bisection, live-mode transport, and the `#[derive(FoldbackHash)]` opt-in field-hashing macro.
- `foldback-cli`: `analyze` and `ci-check` subcommands, reading `.foldback` files.
- `foldback-ui`: the Tauri app — offline timeline/peer-comparison/bisection drill-down, plus live mode (connect to a running game over a local WebSocket).
- `foldback-rs`: GGRS bridge (`checksum`/`record_desync`).
- `foldback-sys` + `bindings/unity`: a full C ABI (Level 1/2/3) and a real Unity UPM package, verified against IL2CPP AOT compilation in CI.
- `bindings/godot`: a GDExtension addon (`gdext`), verified against a real headless Godot 4.7.2 engine in CI.
- `bindings/unreal`: `UFoldbackSubsystem` + Blueprint wrappers over the C ABI, `UPROPERTY(meta=(FoldbackHash))` reflective hashing (Editor/Development-Editor builds only — see `bindings/unreal/README.md`), and a Mass Entity integration path (`UFoldbackHashProcessor`) — verified against a real, locally-built Unreal Engine 5.8.2 (no hosted CI leg — Unreal has no scriptable install path, see `bindings/unreal/README.md`).
- `examples/minimal-rust`, `examples/ggrs-demo`, `examples/live-demo`, `examples/unity-demo`, `examples/godot-demo`, `examples/unreal-demo`, `examples/unreal-mass-demo`: real, runnable proof for each of the above, not just compiled code.

## What's planned next

- **Ongoing**: auto/reflective hashing — done for all three engines (Bevy, Unity, Godot: each has a walker, opt-in enforcement, a depth guard, visibility tooling including a real in-editor dock, a CI-gated performance benchmark, and schema-drift detection (`foldback schema-diff`); see [Auto/Reflective Hashing](../integrations/reflective-hashing.md) for what's genuinely closed vs. deliberately different per engine vs. still open). MVP/launch-gate items still open: code signing for the UI installers, the full demo recording, external (non-maintainer) integration validation.

Full roadmap and week-by-week reasoning: the project's own planning set (linked from [Project](../project/testing.md) pages) — this site tracks `main`, so it describes what's actually shipped, not the plan for what will be.

## The bisection model, briefly

Once a divergence tick is known (peers' hashes disagree), Foldback doesn't need to search *which* tick — peers report every tick, so the first mismatch is found in one pass. The actual search, once Level 2/3 exist, is over *what part of the state* diverged within that already-known tick: re-hash at finer granularity (per-entity, then per-field) between the last known-good tick and the first bad one. It's `git bisect`'s algorithm applied to a hash tree instead of commits.

This tiered design means Level 1 alone (one hash per tick, the whole current integration cost) already tells you *which tick* — Level 2/3 integration (more work, structured hashing) buys *which entity/field* on top of that.

See [Hashing Levels](../guide/hashing-levels.md) for the integration-cost tradeoff in more depth, and [RFC-0005](../project/rfcs/0005-bisection-granularity-model.md) for a real limitation of this model worth knowing about up front (snapshot-restore capability).
