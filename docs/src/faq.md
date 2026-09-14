# FAQ

This page grows from real questions people actually ask, not speculation — short for now since the project is Phase 0.

**Is Foldback ready to use?**
The core library (`foldback-core`) and part of the CLI (`analyze`, `ci-check`) are shipped and tested. There's no UI yet, no engine bindings beyond raw Rust, and no crates.io release. See [How Foldback Works](getting-started/how-it-works.md) for exactly what exists today.

**Why xxHash3 and not a cryptographic hash?**
This is integrity-checking (did two peers compute the same state), not security — a non-cryptographic hash is faster and that's the only property that matters here. See the [performance spike](project/performance.md) for why speed was worth validating rather than assuming.

**Does Foldback slow down my game?**
The validated hot-path cost (hash + ring-buffer handoff) is under 200µs even at 100,000 simulated entities — well under 1.2% of a 60Hz frame budget. See [Performance](project/performance.md) for the full numbers.

**Why is field-hashing opt-in instead of opt-out?**
So a newly-added non-deterministic field (a timestamp, a debug label) can never silently become a phantom divergence source. See [RFC-0003](project/rfcs/0003-field-hashing-opt-in.md).

**Can I use this with Unity/Godot/Unreal today?**
Yes to all three — see their integration pages: [Unity](integrations/unity.md), [Godot](integrations/godot.md), [Unreal](integrations/unreal.md). All three are fully shipped, including Unreal's reflective hashing and Mass Entity integration.
