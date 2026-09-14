# Core Concepts

## Divergence, not just "desync happened"

A lockstep/rollback simulation is deterministic by construction: given the same inputs, every peer's simulation should produce bit-identical state. When it doesn't, that's a **divergence** — and the actual bug is almost never visible from the symptom (a player sees their game snap or disconnect). Foldback's whole job is turning "something diverged" into "tick 4,213, entity 87's `velocity.x` field: `3.14159` on peer A vs. `3.14158` on peer B."

## Sessions

A `Session` is the one object your integration talks to. It accumulates tick hashes, tracks what each peer has reported, and answers "has anything diverged yet." See [Sessions, Files, and Live Mode](sessions.md) for the different ways a session's data can flow: in-process comparison, peer-to-peer exchange over your netcode, or written to a `.foldback` file for later analysis.

## Hashing, not diffing

Foldback never transmits or stores your actual game state on the hot path — only a hash of it (xxHash3, non-cryptographic, chosen for speed since this is integrity-checking, not security). A hash mismatch tells you *that* two peers' states differ, not *how*, at Level 1. Getting to "how" is what the bisection engine and Level 2/3 hashing exist for — see [Hashing Levels](hashing-levels.md).

## Bisection

Once a divergence tick is known, Foldback doesn't re-simulate from scratch — it narrows using whatever granularity of hash data you recorded. See [How Foldback Works](../getting-started/how-it-works.md#the-bisection-model-briefly) for the short version, and [RFC-0005](../project/rfcs/0005-bisection-granularity-model.md) for the real limitation (snapshot-restore capability) worth knowing about before you rely on it.

## Opt-in, not opt-out

Every structured-hashing mechanism in Foldback (the field-hashing derive macro, per-engine equivalents) is opt-in by default: nothing is hashed until you explicitly mark it. This is a deliberate choice — see [RFC-0003](../project/rfcs/0003-field-hashing-opt-in.md) for why the opposite default (hash everything, exclude what you don't want) was rejected.
