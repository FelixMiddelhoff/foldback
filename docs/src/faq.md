# FAQ

This page grows from real questions people actually ask, not speculation.

**Where do I start?**
[How Foldback Works](getting-started/how-it-works.md) for the pieces and what each one does; the [Quickstart](getting-started/quickstart.md) for the smallest real integration. Not yet on crates.io — depend on it as a git or path dependency in the meantime.

**Why xxHash3 and not a cryptographic hash?**
This is integrity-checking (did two peers compute the same state), not security — a non-cryptographic hash is faster and that's the only property that matters here. See the [performance spike](project/performance.md) for why speed was worth validating rather than assuming.

**Does Foldback slow down my game?**
The validated hot-path cost (hash + ring-buffer handoff) is under 200µs even at 100,000 simulated entities — well under 1.2% of a 60Hz frame budget. See [Performance](project/performance.md) for the full numbers.

**Why is field-hashing opt-in instead of opt-out?**
So a newly-added non-deterministic field (a timestamp, a debug label) can never silently become a phantom divergence source. See [RFC-0003](project/rfcs/0003-field-hashing-opt-in.md).

**Can I use this with Unity/Godot/Unreal?**
Yes to all three — see their integration pages: [Unity](integrations/unity.md), [Godot](integrations/godot.md), [Unreal](integrations/unreal.md). Each has reflective hashing (see [Auto/Reflective Hashing](integrations/reflective-hashing.md)), and Unreal also has a Mass Entity integration.
