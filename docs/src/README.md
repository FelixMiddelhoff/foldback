# Foldback

Foldback finds exactly where and why your lockstep/rollback simulation desynced — one hook per tick, a bisection engine that narrows a mismatch down to the field, and a UI that turns a wall of hash logs into a timeline you can point at.

> **Status**: Phase 0 (core library + CLI) is done. The UI, live mode, and every engine binding beyond a bare Rust example are not built yet — see [How Foldback Works](getting-started/how-it-works.md) for what exists today versus what's planned.

## The gap

Every serious lockstep/rollback project ends up building a version of this itself: RimWorld Multiplayer has one, O3DE has one, GGRS ships a `SyncTestSession` that does a narrow slice of it. Each is locked to one project or engine. Foldback is the reusable version — a small Rust core with a stable C ABI, so the same hashing/bisection engine works from Bevy, Unity, Godot, or Unreal instead of every team reinventing it.

## 60-second quickstart

The entire integration cost for Level-1 (tick-only) divergence detection: one call per tick.

```rust
use foldback_core::session::Session;

let mut session = Session::builder()
    .tick_rate_hz(60)
    .peer_count(2)
    .build()?;

// each simulation tick:
let state_bytes = bincode::serialize(&world.deterministic_state())?;
session.hash_tick(tick, &state_bytes)?;

// exchange session.take_pending_hashes() with peers over your existing
// netcode channel — a handful of bytes per tick.
```

If a peer's hash for a tick doesn't match, `session.check_divergence()` returns `Some(DivergenceTick(tick))` the moment enough peers have reported that tick. See the [Cookbook](cookbook/README.md) for the full recipe set.

## Pick your engine

| Engine | Status |
|---|---|
| [Rust / GGRS / Bevy](integrations/rust-ggrs.md) | Core API ships today; GGRS-specific hook is Phase 2 |
| [Unity](integrations/unity.md) | **Shipped** (Phase 3) |
| [Godot](integrations/godot.md) | **Shipped** (Phase 4) |
| [Unreal Engine](integrations/unreal.md) | Planned, Phase 5 |

## Further down

- [Architecture](reference/architecture.md) — how the core, CLI, UI, and bindings fit together.
- [Protocol & File Format Spec](reference/protocol-spec.md) — the `.foldback` session file format and live-mode transport, both versioned from day one.
- vs. hand-rolling your own sync-checksum system: Foldback gives you the bisection engine (tick → entity → field) and a UI for free, instead of a yes/no checksum match you have to build tooling around yourself.
