# Cookbook

Short, copy-pasteable recipes matching the real, shipped API — every sample below compiles against `main`.

Each Rust recipe leaves out error handling you'd keep in real code (`Result` unwraps stand in for real handling).

| # | Recipe |
|---|---|
| 1 | [Minimal integration](#1-minimal-integration) |
| 2 | [CI gate](#2-ci-gate) |
| 3 | [GGRS/Bevy integration](#3-ggrsbevy-integration) |
| 4 | [Per-entity hashing](#4-per-entity-hashing) |
| 5 | [Per-field hashing](#5-per-field-hashing) |
| 6 | [Reading a `.foldback` file programmatically](#6-reading-a-foldback-file-programmatically) |
| 7 | [Live mode](#7-live-mode) |
| 8 | [Unity integration](#8-unity-integration) |
| 9 | [Godot integration](#9-godot-integration) |

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

Run `foldback ci-check <file>` against an already-recorded `.foldback` file for the CI exit-code contract (0 clean, non-zero on divergence) — see [CI Integration](../usage/ci-gate.md).

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

**Opt-in by default** — see [RFC-0003](../project/rfcs/0003-field-hashing-opt-in.md) for why. `#[derive(FoldbackHash)]` hashes nothing until a field is explicitly marked; an unmarked field's name lands in the generated `UNTRACKED_FIELDS` constant rather than silently disappearing either way. Two ways to mark a field, both landing in `TRACKED_FIELDS`:
- `#[foldback(hash)]` — hashed here, by the generated `write_hashed_fields` above, via `foldback_core::hashable::FieldBytes` (built for the common fixed-size numeric primitives and fixed-size arrays of them; implement it yourself for a custom vector/quaternion type).
- `#[foldback(reflect)]` — tracked, but left for a reflective walker (e.g. Bevy's `foldback_rs::bevy::hash_reflected`, see [Auto/Reflective Hashing](../integrations/reflective-hashing.md)) to hash instead. No `FieldBytes` bound, so a compound field (a nested struct, a `Vec`, a `HashMap`) can opt in without needing a `FieldBytes` impl it doesn't otherwise need.

`#[foldback(skip)]` marks a field as deliberately not hashed (distinct from leaving it unmarked, which still shows up in `UNTRACKED_FIELDS` for visibility).

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

Feed collected `TickHashRecord`s into `foldback_core::bisect::find_first_divergence` directly — the same function `foldback analyze` itself calls internally — to find the first divergence tick from your own tooling.

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

Shipped — Level 1 (per-tick), Level 2/3 (per-entity/per-field, `HashEntity`/`HashField` and their peer-recording counterparts), and reflective hashing, all exposed across the FFI boundary and verified under real IL2CPP AOT compilation in CI. See [docs/integrations/unity.md](../integrations/unity.md) for the full status.

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

```gdscript
extends Node

var session := FoldbackSession.new()

func _ready():
    if not session.configure({"tick_rate_hz": 60, "peer_count": 2}):
        push_error(session.get_last_error())

func _physics_process(_delta):
    var state := serialize_deterministic_state()
    session.hash_tick(Engine.get_physics_frames(), state)
```

GDExtension binding (`bindings/godot`) exposes the same core through a GDScript-native `FoldbackSession` class rather than raw FFI calls — built directly on `gdext` (godot-rust) rather than through `foldback-sys`'s C ABI, since `gdext` generates the GDExtension registration itself.

A GDExtension class's `.new()` calls its zero-argument `_init`, so there's no supported way to route constructor arguments through it directly — hence the two-step `.new()` then `.configure(dict) -> bool` shape, returning `false` and setting `get_last_error()` on failure rather than throwing. `u64` hashes cross into GDScript as `i64` via exact bit-reinterpretation (GDScript's only integer type) — a hash may print as negative, which is expected and harmless for equality-based divergence checks.

