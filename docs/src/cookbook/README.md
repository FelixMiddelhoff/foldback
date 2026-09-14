# Cookbook

Short, copy-pasteable recipes in the target API shape. Recipes 1–2 (and the file-recording variant of recipe 6) reflect what's actually shipped as of Phase 0; everything else is the target shape being built toward — check the status column before assuming a recipe compiles against `main` today.

Each Rust recipe leaves out error handling you'd keep in real code (`Result` unwraps stand in for real handling).

| # | Recipe | Status |
|---|---|---|
| 1 | [Minimal integration](#1-minimal-integration) | **Shipped** (Phase 0) |
| 2 | [CI gate](#2-ci-gate) | **Shipped** (`finish()`/`Session` API); `foldback ci-check --replay` sim-orchestration mode not yet built |
| 3 | [GGRS/Bevy integration](#3-ggrsbevy-integration) | Planned, Phase 2 |
| 4 | [Per-entity hashing](#4-per-entity-hashing) | Planned, Phase 2 (Level 2) |
| 5 | [Per-field hashing](#5-per-field-hashing) | Planned, Phase 2 (Level 3) |
| 6 | [Reading a `.foldback` file programmatically](#6-reading-a-foldback-file-programmatically) | Frame reading: **shipped** (`foldback_core::format::FrameReader`). `bisect()` convenience wrapper: not yet built as shown |
| 7 | [Live mode](#7-live-mode) | Planned, Phase 2 |
| 8 | [Unity integration](#8-unity-integration) | Planned, Phase 3 |
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

`foldback-rs` hooks GGRS's own checksum callback so there's no duplicate simulation loop to maintain — Foldback rides the hash GGRS already computes for its own rollback correctness checks, upgrading it from "yes/no matched" to "here's exactly what diverged."

```rust
use foldback_rs::ggrs::FoldbackGgrsExt;

let mut ggrs_session = ggrs::SessionBuilder::<MyConfig>::new()
    .with_num_players(2)?
    .start_p2p_session(socket)?;

let mut foldback = Session::builder().tick_rate_hz(60).peer_count(2).build()?;
ggrs_session.attach_foldback(&mut foldback); // wraps the existing checksum callback

// GGRS's normal advance_frame loop is unchanged — foldback observes, doesn't intercept.
```

---

## 4. Per-entity hashing

Upgrades a divergence report from "tick 4821" to "tick 4821, entity Player#3" — worth the extra integration cost once Level 1 has actually found a bug and you need to narrow it down.

```rust
for (entity_id, component_bytes) in world.iter_entities_serialized() {
    session.hash_entity(tick, entity_id, &component_bytes)?;
}
```

Can be added incrementally alongside `hash_tick` — Level 1 and Level 2 aren't mutually exclusive; the bisection engine uses whichever levels are present in a given session and reports "enable per-entity hashing for a finer result" when only Level 1 data exists.

---

## 5. Per-field hashing

The finest level — needs a derive macro since hand-writing per-field calls for every struct doesn't scale.

```rust
#[derive(FoldbackHash)]
struct PlayerState {
    #[foldback(hash)]
    position: Vec3,
    #[foldback(hash)]
    velocity: Vec3,
    debug_name: String, // unmarked — not hashed, flagged by `foldback lint` as untracked
}

session.hash_fields(tick, entity_id, &player_state)?; // macro-generated per-field hashing + value capture
```

**Opt-in by default** — see [RFC-0003](../project/rfcs/0003-field-hashing-opt-in.md) for why. `#[derive(FoldbackHash)]` hashes nothing until a field is explicitly marked `#[foldback(hash)]`.

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

Same `Session` API, pointed at a socket instead of a file — the UI-facing half of [the protocol spec's live-mode transport](../reference/protocol-spec.md#2-live-mode-transport). Not implemented yet (Phase 2).

```rust
let mut session = Session::builder()
    .tick_rate_hz(60)
    .peer_count(2)
    .live_endpoint("127.0.0.1:9871") // starts listening, non-blocking
    .build()?;
```

Drag-and-drop a `.foldback` file onto the UI for offline analysis, or point it at `ws://127.0.0.1:9871` for live — same views either way.

---

## 8. Unity integration

Not implemented yet (Phase 3).

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
