---
rfc: 0004
title: Live-mode transport (loopback WebSocket)
status: accepted
created: 2026-09-14
supersedes:
---

# Summary

Live mode uses a loopback WebSocket for its transport, with JSON control messages and binary data frames that share the exact same byte layout as `.foldback` file frames.

# Motivation

Needed a decision between a Unix socket/named pipe (simpler, desktop-only) and a WebSocket (slightly heavier, but the same framing works unmodified if a genuinely remote session — a QA machine streaming to a developer over a real network — is ever wanted).

# Design

Full spec: [Protocol & File Format Spec §2](../../reference/protocol-spec.md#2-live-mode-transport). JSON for control messages (session start/stop, metadata). Binary data frames reuse the file format's own frame layout deliberately, so the UI's parser is one code path for both live and offline mode — no separate live-mode deserializer to keep in sync.

# Drawbacks

A WebSocket is heavier than a Unix socket/named pipe for the common desktop-only case (local game process talking to a local UI process) — accepted for the optionality it buys on the remote-streaming case, and because the framing reuse benefit (one parser, not two) outweighs the marginal transport overhead.

# Alternatives considered

gRPC/protobuf was rejected specifically on dependency-footprint grounds (protocol spec §2.3): a lot of this audience's game code is C/C++/C# that doesn't want a protobuf toolchain pulled in just for a debug hook, and hand-rolled framing doesn't need schema codegen since it reuses the file format's own frame layout already.

A plain Unix socket/named pipe was considered and not chosen, specifically to keep the door open to a genuinely remote (networked) session without a later transport rewrite.

# Prior art

Reusing on-disk record format as the wire format for live streaming is a well-established pattern (e.g. many replay-capable tools serialize the same frame structure whether writing to disk or streaming live) — chosen here for exactly the same "one code path" reason.

# Unresolved questions

None outstanding for the transport choice itself; authentication/access-control for a genuinely networked (non-loopback) session is out of scope for this RFC and not yet designed.

# History

- 2026-09-14: accepted.
