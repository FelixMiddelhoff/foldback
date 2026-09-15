# Protocol & File Format Spec

Two wire formats: the at-rest `.foldback` session file, and the live-mode transport between a running game and the UI. Both versioned from day one — external tools (CI scripts, third-party engine bindings) will depend on this contract, so breaking it silently is the one mistake to design against.

## 1. `.foldback` session file format

Chunked binary log, not a database — append-only, streaming-writable (frames are written as they happen, no rewrite-the-whole-file-per-tick cost), and tolerant of truncation (a crash mid-recording must still yield a parseable prefix). **Implemented** — `foldback_core::format`.

### 1.1 Header (fixed 32 bytes)

| Offset | Size | Field | Notes |
|---|---|---|---|
| 0 | 4 | magic | ASCII `FBK1` |
| 4 | 2 | format_version | u16, starts at `1` |
| 6 | 2 | flags | reserved, must be 0 in v1 |
| 8 | 4 | tick_rate_hz | u32 |
| 12 | 4 | peer_count | u32 |
| 16 | 16 | build_id | opaque bytes, game-supplied (e.g. a hash of the game build) — lets a reader warn "this session was recorded against a different build than the one you're diffing against" |

### 1.2 Frame stream

Each frame: `[u8 frame_type][u32 payload_len][payload bytes]`. A reader that hits EOF mid-frame (impossible `payload_len`, or a short read) discards that partial frame and stops — everything before it is still valid.

| Type | Name | Payload |
|---|---|---|
| `0x01` | `TickHash` | `u64 tick, u16 peer_id, u64 hash` |
| `0x02` | `EntityHash` | `u64 tick, u16 peer_id, u64 entity_id, u64 hash` (Level 2 bisection data) |
| `0x03` | `FieldHash` | `u64 tick, u16 peer_id, u64 entity_id, u32 field_name_len, [field_name bytes], u64 hash, u32 value_len, [value bytes]` (Level 3 — carries the actual value for the diff view, not just a hash) |
| `0x10` | `Snapshot` | `u64 tick, u16 peer_id, u32 compressed_len, [zstd bytes]` |
| `0x20` | `Metadata` | `u32 key_len, [key bytes], u32 value_len, [value bytes]` — free-form annotations, not interpreted by the core |
| `0xFF` | `EndOfStream` | empty — written on clean shutdown; its *absence* is how a reader knows a file is a truncated/crashed recording |

### 1.3 Design notes

- Field names are UTF-8, not an enum — keeps the format engine-agnostic.
- No random-access index in v1 (pure log). A v2 addition, if a real session ever needs it, is a trailing index block written at `EndOfStream` time — additive, old readers just ignore trailing bytes after the frames they understand.
- **Versioning rule**: `format_version` bumps only on a breaking change to frame layout. New frame types can be added without a version bump as long as readers skip unknown `frame_type` bytes by length rather than erroring — this is the actual forward-compatibility mechanism, more important than the version number itself. `foldback_core::format::FrameReader` implements exactly this.

Full rationale: [RFC-0001](../project/rfcs/0001-session-file-format.md).

## 2. Live-mode transport

**Shipped**: `foldback_core::live` on the game side, `foldback-ui`'s "Connect live…" on the UI side, proven against each other end to end (see `examples/live-demo`). Local loopback WebSocket (`ws://127.0.0.1:<port>`), chosen over a Unix socket/named pipe so the exact same message framing works unmodified for a genuinely remote session later, at negligible cost over loopback today.

### 2.1 Messages

JSON for control messages (rare, human-debuggable), binary frames (same layout as §1.2) for the actual hash/snapshot stream.

Control (JSON, text WS frames):
```jsonc
{ "type": "hello", "protocol_version": 1, "tick_rate_hz": 60, "peer_count": 2, "build_id": "..." }
{ "type": "goodbye", "reason": "game_exited" }
```

Data (binary WS frames): identical byte layout to a `.foldback` frame — deliberately, so the UI's parser is one code path for both live and offline mode.

### 2.2 Connection lifecycle

1. Game starts, opens a WS listen socket, waits (non-blocking — must never stall the game loop waiting for a UI to connect).
2. UI connects, receives `hello`.
3. Game streams `TickHash`/`Snapshot`/etc. frames as they occur.
4. Either side can disconnect at any time without protocol-level cleanup required — reconnect just re-sends `hello`.
5. UI can optionally request the game start recording to a `.foldback` file simultaneously — live viewing and offline capture aren't mutually exclusive.

### 2.3 Why not gRPC/protobuf

Keeps the game-side dependency footprint minimal — a lot of this audience is C/C++/C# game code that doesn't want a protobuf toolchain pulled in for a debug hook. The binary frame format is already shared with the file format, so there's no second format to maintain. Full rationale: [RFC-0004](../project/rfcs/0004-live-mode-transport.md).

## 3. C ABI surface (`foldback-sys`)

**Shipped**: `foldback-sys/include/foldback.h`, generated via `cbindgen` from the Rust crate — never hand-edited, regenerated whenever the crate's public `extern "C"` surface changes. Covers session lifecycle, Level 1/2/3 hashing, schema-drift's `foldback_record_schema`, pending-hash draining, divergence checking, and error reporting; used by the Unity and Unreal bindings (Godot calls `foldback-core` directly through `gdext` instead — see [Architecture](architecture.md)). See [C API](c-api.md) for the header itself, per this project's "link out, don't duplicate" rule for generated references — the sketch this section originally carried predated the real implementation and doesn't match its actual signatures (every fallible call returns a `FoldbackStatus` code, for one, not the return-value-doubles-as-status-and-hash shape an early sketch had), so it's not reproduced here.

All fallible calls return a status code, never throw/panic across the FFI boundary (a Rust panic unwinding into C/C++/C# calling code is undefined behavior) — every entry point is wrapped in `catch_unwind` and converts to an error code as a hard rule, not a best-effort. Full rationale: [RFC-0002](../project/rfcs/0002-c-abi-surface.md).

## Rust API reference

Generated `cargo doc` output isn't published anywhere yet (pre-crates.io). Once `foldback-core` is published, its docs.rs page becomes the canonical generated reference — this page stays hand-written and links out rather than duplicating it, per the project's "link out, don't duplicate" rule for generated references.
