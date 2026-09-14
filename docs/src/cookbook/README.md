# Cookbook

Short, copy-pasteable recipes in the target API shape. Recipes 1, 2, 3, 4, 5, and 7 are shipped (Phase 0/2); everything else is the target shape being built toward — check the status column before assuming a recipe compiles against `main` today.

Each Rust recipe leaves out error handling you'd keep in real code (`Result` unwraps stand in for real handling).

| # | Recipe | Status |
|---|---|---|
| 1 | [Minimal integration](#1-minimal-integration) | **Shipped** (Phase 0) |
| 2 | [CI gate](#2-ci-gate) | **Shipped** (`finish()`/`Session` API); `foldback ci-check --replay` sim-orchestration mode not yet built |
| 3 | [GGRS/Bevy integration](#3-ggrsbevy-integration) | **Shipped** (Phase 2) — `foldback_rs::ggrs::checksum`/`record_desync`, not the originally-sketched `attach_foldback` (GGRS has no hookable checksum callback to wrap) |
| 4 | [Per-entity hashing](#4-per-entity-hashing) | **Shipped** (Phase 2) |
| 5 | [Per-field hashing](#5-per-field-hashing) | **Shipped** (Phase 2) — `#[derive(FoldbackHash)]`, feature `derive` |
| 6 | [Reading a `.foldback` file programmatically](#6-reading-a-foldback-file-programmatically) | Frame reading: **shipped** (`foldback_core::format::FrameReader`). `bisect()` convenience wrapper: not yet built as shown |
| 7 | [Live mode](#7-live-mode) | **Shipped** (Phase 2) — game side (`foldback_core::live::LiveServer`) and UI side (`foldback-ui`'s "Connect live…") both built and proven against each other |
| 8 | [Unity integration](#8-unity-integration) | **Shipped** (`bindings/unity`, `HashTick`/`RecordPeerHash`/`TakePendingHashes`/`CheckDivergence`/`Finish`), Phase 3 — verified against the real native library via a .NET P/Invoke harness. Two named gaps: the IL2CPP CI leg (needs a real Unity install, not available where this was built) and Level 2/3 hashing across the FFI boundary. `[FoldbackHash]` reflection below is unbuilt, tracked separately as the reflective-hashing stretch |
| 9 | [Godot integration](#9-godot-integration) | Planned, Phase 4 |
| 10 | [Annotating the timeline](#10-annotating-the-timeline) | `Metadata` frame exists in the file format; a convenience `session.annotate()` wrapper not yet built |

---

## 1. Minimal integration

The entire integration cost for Level-1 (tick-only) divergence detection: one call per tick, one serialization of "whatever state determinism depends on."

```rust
let mut session = Session::builder()
    .tick_rate_hz(60)
    .peer_count(2)
    .build()?;

// each simulation tick:
let state_bytes = bincode::serialize(&world.deterministic_state())?;
session.hash_tick(tick, &state_bytes)?;

// exchange session.take_pending_hashes() with peers over your existing
// netcode channel (a handful of bytes per tick — piggyback on an existing
// packet, don't open a new one just for this).
```

If a peer's hash for a tick doesn't match, `session.check_divergence()` returns `Some(DivergenceTick(tick))` the moment enough peers have reported that tick — you don't need to wait for the whole match to end to know something's wrong.

---

## 2. CI gate

Single-process determinism testing: run the same simulation twice (or replay a fixed input log twice), assert identical hashes — this is `GGRS::SyncTestSession`'s trick, generalized and made engine-agnostic.

```rust
let mut a = Session::builder().tick_rate_hz(60).peer_count(1).build()?;
let mut b = Session::builder().tick_rate_hz(60).peer_count(1).build()?;

for tick in 0..NUM_TICKS {
    run_one_tick(&mut world_a, tick);
    run_one_tick(&mut world_b, tick); // fresh instance, same recorded inputs

    a.hash_tick(tick, &serialize(&world_a))?;
    b.hash_tick(tick, &serialize(&world_b))?;
}

assert_eq!(a.finish(), b.finish(), "non-determinism detected — see bisection report below");
```

Today, run `foldback ci-check <file>` against an already-recorded `.foldback` file for the CI exit-code contract (0 clean, non-zero on divergence) — see [CI Integration](../usage/ci-gate.md). The fancier `--replay inputs.log --sim-binary ./target/release/my_sim` orchestration mode (spawn your sim binary twice, diff automatically) isn't built yet.

---

## 3. GGRS/Bevy integration

GGRS has no hookable "checksum callback" to wrap — a game computes its own checksum and hands it to GGRS via `GameStateCell::save(frame, state, checksum)`, and for a real networked session GGRS emits `GgrsEvent::DesyncDetected { local_checksum, remote_checksum, .. }` when two peers' checksums for a frame disagree. `foldback_rs::ggrs` (feature `ggrs`) is the real integration point — two small helpers, not a session-wrapping extension trait:

```rust
use foldback_rs::ggrs::{checksum, record_desync};

// At the point your game already computes a checksum for GameStateCell::save:
let cs = checksum(&state_bytes); // Foldback's hash, widened to GGRS's u128 checksum type
cell.save(frame, Some(state), Some(cs));

// When GGRS's own event loop reports a desync it already detected over the network:
if let GgrsEvent::DesyncDetected { frame, local_checksum, remote_checksum, .. } = event {
    record_desync(&mut foldback_session, frame as u64, local_peer_id, remote_peer_id,
                   local_checksum, remote_checksum)?;
}
```

`SyncTestSession` (single-process — see `examples/ggrs-demo`) doesn't need this bridge: it catches a checksum mismatch internally, no cross-peer network exchange to bridge.

---

## 4. Per-entity hashing

Upgrades a divergence report from "tick 4821" to "tick 4821, entity Player#3" — worth the extra integration cost once Level 1 has actually found a bug and you need to narrow it down.

```rust
for (entity_id, component_bytes) in world.iter_entities_serialized() {
    session.hash_entity(tick, entity_id, &component_bytes)?;
}
```

Can be added incrementally alongside `hash_tick` — Level 1 and Level 2 aren't mutually exclusive; the bisection engine uses whichever levels are present in a given session and reports "enable per-entity hashing for a finer result" when only Level 1 data exists. See `examples/ggrs-demo` for a real recording using this.

---

## 5. Per-field hashing

```rust
use foldback_core::FoldbackHash;

#[derive(FoldbackHash)]
struct PlayerState {
    #[foldback(hash)]
    position: [f32; 3],
    #[foldback(hash)]
    velocity: [f32; 3],
    debug_name: String, // unmarked — not hashed, named in PlayerState::UNTRACKED_FIELDS
}

session.hash_fields(tick, entity_id, &player_state)?; // macro-generated per-field hashing + value capture
```

**Opt-in by default** — see [RFC-0003](../project/rfcs/0003-field-hashing-opt-in.md) for why. `#[derive(FoldbackHash)]` hashes nothing until a field is explicitly marked `#[foldback(hash)]`; an unmarked field's name lands in the generated `UNTRACKED_FIELDS` constant rather than silently disappearing either way. A field's type needs a `foldback_core::hashable::FieldBytes` impl to be hash-able — built for the common fixed-size numeric primitives and fixed-size arrays of them; implement it yourself for a custom vector/quaternion type.

---

## 6. Reading a `.foldback` file programmatically

For building your own tooling on top (a custom dashboard, a Slack bot that posts a divergence summary) without going through the CLI or UI.

```rust
use foldback_core::format::{FrameReader, Frame};
use std::fs::File;

let file = File::open("match_4821.foldback")?;
for frame in FrameReader::new(file) {
    match frame? {
        Frame::TickHash { tick, peer_id, hash } => { /* ... */ }
        Frame::Snapshot { tick, peer_id, compressed } => { /* ... */ }
        _ => {}
    }
}
```

A `bisect()` convenience wrapper that returns the same text report `foldback analyze` prints is not yet built — for now, feed collected `TickHashRecord`s into `foldback_core::bisect::find_first_divergence` directly (what `foldback analyze` itself does internally).

---

## 7. Live mode

A separate `LiveServer` type used alongside `Session`, not a `SessionBuilder` option — a WebSocket connection has its own hello/reconnect lifecycle ([protocol spec §2](../reference/protocol-spec.md#2-live-mode-transport)), not a plain byte sink the way a recording file is.

```rust
use foldback_core::live::LiveServer;
use foldback_core::format::Frame;

// Once, at startup — binds immediately (a port-in-use error surfaces
// synchronously), a background thread owns accept/handshake/reconnect.
let live = LiveServer::bind("127.0.0.1:9871", 60, 2, build_id)?;

let mut session = Session::builder().tick_rate_hz(60).peer_count(2).build()?;

// each tick:
session.hash_tick(tick, &state_bytes)?;
for pending in session.take_pending_hashes() {
    live.send_frame(Frame::TickHash { tick: pending.tick, peer_id: 0, hash: pending.hash });
}
```

`send_frame` never blocks the caller (an unbounded channel send) and silently drops frames if no UI is connected yet. In the UI: drag-and-drop a `.foldback` file for offline analysis, or click "Connect live…" and give it `ws://127.0.0.1:9871` — same timeline/peer-comparison/drill-down views either way. See `examples/live-demo` for a real end-to-end proof (a toy sim streaming a real injected divergence, caught live in the actual UI).

---

## 8. Unity integration

Shipped (Phase 3), Level 1 (per-tick) only — see [docs/integrations/unity.md](../integrations/unity.md) for status and the two named gaps (IL2CPP CI leg, Level 2/3 across the FFI boundary).

```csharp
using Foldback;

var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 2 });

// each FixedUpdate:
byte[] state = SerializeDeterministicState();
session.HashTick(tick, state);
```

`[FoldbackHash]` field attribute mirrors the Rust derive macro (recipe 5) via Unity's own reflection, at higher per-tick cost than the explicit-serialization path — fine for editor/dev builds, measure before shipping it in a release build's hot path.

---

## 9. Godot integration

Not implemented yet (Phase 4).

```gdscript
extends Node

var session := FoldbackSession.new({"tick_rate_hz": 60, "peer_count": 2})

func _physics_process(_delta):
    var state := serialize_deterministic_state()
    session.hash_tick(Engine.get_physics_frames(), state)
```

GDExtension binding exposes the same core through a GDScript-native `FoldbackSession` class rather than raw FFI calls.

---

## 10. Annotating the timeline

Free-form metadata frames let the UI show *why* a tick matters, not just *that* it happened. The `Metadata` frame type already exists in the file format (see [protocol spec](../reference/protocol-spec.md)); the convenience wrapper below isn't built yet — write `Frame::Metadata { key, value }` directly via `FrameReader`'s write-side counterpart in the meantime.

```rust
session.annotate(tick, "input:P1", "jump")?;
session.annotate(tick, "event", "round_start")?;
```

Shown as small tick markers in the UI timeline (once the UI exists), independent of and orthogonal to divergence markers.
