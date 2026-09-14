---
rfc: 0002
title: The C ABI surface (foldback-sys)
status: accepted
created: 2026-09-14
supersedes:
---

# Summary

`foldback-sys` exposes a low-level, status-code-based C ABI, generated into `foldback.h` by `cbindgen` from `#[no_mangle] extern "C"` functions — never hand-written or hand-edited.

# Motivation

Every non-Rust binding (Unity, Godot, Unreal) depends on this surface being stable and safe to call across an FFI boundary from a language/runtime that may disable exceptions (Unreal) or use ahead-of-time compilation with real marshaling restrictions (Unity IL2CPP — see `foldback-risk-plan.md` P1).

# Design

Sketch in `foldback-protocol-spec.md` §3. `cbindgen`-generated header is the single source of truth. Every fallible call returns a status code rather than throwing/panicking across the boundary. `catch_unwind` at every FFI entry point is a hard rule — a Rust panic must never unwind across the FFI boundary, since that's undefined behavior on the C/C++/C# side.

# Drawbacks

Status-code-based error handling is more verbose for callers than an idiomatic wrapper would be. Accepted deliberately: the idiomatic wrapper is exactly what `foldback-rs` and the per-engine binding layers exist to provide on top of this low-level, safety-first surface — the verbosity is pushed to one place instead of leaking into every caller.

# Alternatives considered

An exception-based or panic-propagating API was not seriously considered — it's simply unsound across this FFI boundary given the target runtimes (Unreal disables exceptions in places; IL2CPP has its own marshaling constraints). Not a close call.

# Prior art

Status-code-return + `catch_unwind`-at-the-boundary is the standard, well-established pattern for Rust libraries exposing a C ABI (e.g. how most `-sys` crates in the ecosystem that wrap fallible Rust logic for C consumers behave).

# Unresolved questions

None outstanding — this is a narrow, fully-resolved RFC.

# History

- 2026-09-14: retroactively drafted and accepted during repo bootstrap, decision made during the original planning pass (see `foldback-protocol-spec.md`).
