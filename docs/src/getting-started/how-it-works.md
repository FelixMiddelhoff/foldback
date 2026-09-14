# How Foldback Works

## The pieces

```
foldback-core   hashing, session file format, bisection engine (Rust)
foldback-sys    C ABI over foldback-core, generated foldback.h (cbindgen)
foldback-rs     idiomatic Rust wrapper + GGRS/Bevy helpers
foldback-cli    the `foldback` binary: analyze, ci-check (record: planned)
foldback-ui     Tauri app — timeline, peer comparison, bisection drill-down
bindings/       Unity, Godot, Unreal — each wraps foldback-sys's C ABI
```

`foldback-core` is the only place the actual logic lives. Every other surface — the CLI, the UI, every engine binding — is a thin wrapper around it, so a bug fix or a bisection-engine improvement lands everywhere at once instead of needing to be ported four times.

## What exists today (Phase 0)

- `foldback-core`: xxHash3 hashing, the `.foldback` file format (read + write), a bounded retention ring buffer, zstd snapshot compression, and Level 1 (per-tick) bisection.
- `foldback-cli`: `analyze` and `ci-check` subcommands, reading `.foldback` files.
- `examples/minimal-rust`: a toy deterministic simulation proving the API end to end, no engine dependency.

## What's planned next

- **Docs site** (this site) — stood up now, filled in as each phase ships, not backfilled after the fact.
- **Phase 1 — Foldback UI v1**: the Tauri app, reading `.foldback` files offline. Design is already fully decided — see the [UI mockup](https://claude.ai/code/artifact/98a136d1-d606-4d96-bc90-04e6fe74f292).
- **Phase 2 — Rust/GGRS binding + Level 2/3 bisection**: per-entity and per-field hashing, live mode (a running game streaming to the UI over a local WebSocket).
- **Phase 3–5 — Unity, Godot, Unreal bindings**, each via the stable C ABI.

Full roadmap and week-by-week reasoning: the project's own planning set (linked from [Project](../project/testing.md) pages) — this site tracks `main`, so it describes what's actually shipped, not the plan for what will be.

## The bisection model, briefly

Once a divergence tick is known (peers' hashes disagree), Foldback doesn't need to search *which* tick — peers report every tick, so the first mismatch is found in one pass. The actual search, once Level 2/3 exist, is over *what part of the state* diverged within that already-known tick: re-hash at finer granularity (per-entity, then per-field) between the last known-good tick and the first bad one. It's `git bisect`'s algorithm applied to a hash tree instead of commits.

This tiered design means Level 1 alone (one hash per tick, the whole current integration cost) already tells you *which tick* — Level 2/3 integration (more work, structured hashing) buys *which entity/field* on top of that.

See [Hashing Levels](../guide/hashing-levels.md) for the integration-cost tradeoff in more depth, and [RFC-0005](../project/rfcs/0005-bisection-granularity-model.md) for a real limitation of this model worth knowing about up front (snapshot-restore capability).
