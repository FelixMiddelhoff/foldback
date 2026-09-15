---
rfc: 0003
title: Field-hashing derive macro defaults to opt-in
status: accepted
created: 2026-09-14
supersedes:
---

# Summary

`#[derive(FoldbackHash)]` (and its per-engine equivalents) hashes nothing until a field is explicitly marked `#[foldback(hash)]` — opt-in, not opt-out.

# Motivation

Get this decided once, correctly, before four different engine bindings each have to mirror it independently. A mismatch between engines here (one opt-in, one opt-out) would be a confusing, undocumented inconsistency discovered by users the hard way, long after the first binding shipped.

# Design

Full reasoning: [Cookbook §5](../../cookbook/README.md#5-per-field-hashing). `#[derive(FoldbackHash)]` marks no fields for hashing by default; a field must be explicitly annotated (`#[foldback(hash)]` in Rust, `[FoldbackHash]` in C#, the `foldback_` name prefix in Godot — GDScript has no custom-attribute mechanism GDExtension can hook into, see [Auto/Reflective Hashing](../../integrations/reflective-hashing.md), `UPROPERTY(meta=(FoldbackHash))` in Unreal) to be included.

# Drawbacks

More integration friction per field than an opt-out default would have — every field a user cares about tracking needs an explicit annotation. Mitigated, not eliminated, by `foldback lint`'s unmarked-field-surfacing (see the Cookbook) that surfaces unmarked fields rather than leaving the gap invisible.

# Alternatives considered

Opt-out (hash everything by default, `#[skip]` to exclude) was rejected specifically because it silently turns any newly-added non-deterministic field (a timestamp, a debug name) into a permanent phantom divergence source with no signal that it happened. That's the one failure mode most likely to teach users to distrust the tool's own bisection results.

# Prior art

Opt-in field selection for serialization/hashing purposes is the more conservative and more common choice in determinism-sensitive tooling generally (compare: explicit `#[derive(Hash)]` field inclusion conventions in other Rust serialization ecosystems) — erring toward "silently include too little, loudly lint it" over "silently include too much."

# Unresolved questions

None outstanding for the default itself — the `foldback lint` unmarked-field-surfacing mechanism referenced above has its own design detail in the cookbook, not repeated here.

# History

- 2026-09-14: accepted.
