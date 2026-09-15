---
rfc: 0001
title: The .foldback session file format
status: accepted
created: 2026-09-14
supersedes:
---

# Summary

Define `.foldback` as an append-only, length-prefixed binary frame stream with a fixed 32-byte header, rather than an embedded database or other structured format.

# Motivation

Every binding, the UI, and any third-party tooling built on top of Foldback reads this format — it's externally depended on the moment a game starts recording. Getting the extensibility and failure-mode story right before the first byte is written matters more here than almost anywhere else in the project.

# Design

Full spec: [Protocol & File Format Spec §1](../../reference/protocol-spec.md#1-foldback-session-file-format). Fixed 32-byte header carrying `format_version`; a stream of length-prefixed frames after it. `format_version` is bumped only on breaking frame-layout changes. Unknown frame types are safely skippable by their length prefix — this is the actual forward-compatibility mechanism, not a version-negotiation handshake.

# Drawbacks

No random-access index in v1 — large-session scrubbing performance in the UI's read path is untested until perf-spike-style benchmarking covers it. Treated as an additive v2 concern, not a v1 blocker.

# Alternatives considered

A structured format (SQLite, an embedded DB) was rejected in favor of an append-only log. A database adds write-amplification and corruption-recovery complexity a debug tool doesn't need. A truncated log file degrading gracefully to "everything before the cut" is both simpler and exactly the right failure mode for "the game crashed mid-recording."

# Prior art

Append-only, length-prefixed framing is the same shape used by most streaming record/replay formats (e.g. PCAP-style capture files) specifically because it tolerates truncation gracefully — the same property that makes it right here.

# Unresolved questions

Whether a v2 index format should be a separate sidecar file or an in-band frame type — not decided here, deferred until real large-session performance data exists.

# History

- 2026-09-14: accepted.
