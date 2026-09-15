# Unity

## Overview

`foldback-sys` has a real C ABI covering Level 1 (per-tick), Level 2 (per-entity), and Level 3 (per-field) hashing — status-code-based per [RFC-0002](../project/rfcs/0002-c-abi-surface.md), with a `cbindgen`-generated `foldback.h`. A real C# UPM package (`bindings/unity/`) wraps it — `FoldbackSession`/`FoldbackConfig` matching [cookbook recipe 8](../cookbook/README.md#8-unity-integration) exactly.

Verified against the real, built native library (not just compiled): a standalone .NET console harness (`bindings/unity/Tests~/FoldbackSys.Tests`, a `Tests~` folder — Unity's own convention for "ignore during asset import") P/Invokes the actual `foldback_sys` binary and asserts real behavior — struct marshaling, divergence detection, `finish()` agreement/disagreement across two sessions, file recording, and error handling all pass against real native code.

**IL2CPP AOT — verified both locally and in CI.** `examples/unity-demo` is a real Unity project (`Assets/Il2cppVerify.cs`) referencing `com.foldback.unity` as a real package dependency, built as a Standalone Windows IL2CPP player and actually run — not just compiled. With a real Unity 6000.6.0f1 + Windows Build Support (IL2CPP) install, `BuildScript.BuildIl2Cpp` produced a working IL2CPP player, and running it printed:

```
ok   - hash_tick produces exactly one pending hash
ok   - mismatched peer hashes detected as divergence at the correct tick
ok   - two identical runs' finish() values agree
ok   - a diverging run's finish() value disagrees
PASS
```

— the P/Invoke surface's conservative design (simple `[DllImport]` signatures, no automatic string/array marshaling attributes on the config struct — a hand-managed native UTF-8 buffer instead, see `Runtime/FoldbackNative.cs`'s comments) genuinely survives real IL2CPP AOT compilation, not just in theory.

The CI job `bindings-unity-il2cpp` (`.github/workflows/ci.yml`) codifies this exact sequence in GitHub Actions on `windows-latest`, with `UNITY_EMAIL`/`UNITY_PASSWORD` repository secrets for a Unity Personal license — not just locally. The Unity Hub/Editor/IL2CPP download is cached between CI runs (a cache miss — first run ever, or after 7 days of disuse — is still slow, but every run after that should be fast).

## Who this is for

Unity projects using a lockstep or rollback netcode solution, including ones targeting IL2CPP (AOT compilation).

## Usage

```csharp
using Foldback;

var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 2 });

// each FixedUpdate:
byte[] state = SerializeDeterministicState();
session.HashTick(tick, state);

// send this peer's own hashes to the others over your existing netcode:
foreach (var pending in session.TakePendingHashes())
{
    SendToPeers(pending.Tick, pending.Hash);
}

// on receiving a peer's hash over the same netcode:
session.RecordPeerHash(peerTick, peerId, peerHash);

if (session.CheckDivergence(out var divergedTick))
{
    Debug.LogWarning($"Foldback: peers disagree at tick {divergedTick}");
}
```

`session.Dispose()` (or a `using` block) frees the native session — required, since it owns unmanaged memory.

## Reflective hashing

Landed: `[FoldbackHash]` on a field or property (`Runtime/FoldbackHashAttribute.cs`), walked by `FoldbackReflection.HashReflected(session, tick, entityId, prefix, root)` (`Runtime/FoldbackReflection.cs`) — no hand-written `HashField` calls needed. Enforces opt-in: only `root`'s own `[FoldbackHash]`-tagged members are visible; once one is reached, everything beneath it is walked without needing its own type separately tagged (same model as the Bevy and Unreal bindings). Sorts `IDictionary`/`ISet` entries by key before hashing (§3's shared determinism rule), catches genuine reference cycles via an identity-based visited-set (sound here — unlike the Bevy walker, C# reference types can form real cycles), and enforces a depth guard (default 8, `FoldbackReflectionException`). `FoldbackReflection.ListTracked(Type)` is the visibility-tooling data source (§4), and `bindings/unity/Editor/FoldbackReflectionWindow.cs` (`Window > Foldback > Reflection Inspector`) is a real `EditorWindow` built on top of it — a **Live** tab subscribing to a new `FoldbackReflection.Recorded` event for a history of observed `HashReflected` calls with drill-down into captured fields, and an **Inspect** tab running `ListTracked` against any dragged-in object with no Play Mode needed. Its own `.asmdef` restricts it to `includePlatforms: ["Editor"]` so it never ships in a player build. Per-type member access is cached via compiled `System.Linq.Expressions` getters (§2.2's stated perf mitigation), built once per type.

Verified against the real, built native library the same way the rest of this page is: `bindings/unity/Tests~/FoldbackSys.Tests` exercises the walker, the sorted-container rule, cycle detection, the depth guard, and `ListTracked` — all pass, including a CI-gated explicit-vs-reflective timing comparison, ~3.5-3.9x overhead for the reflective path at 1,000 entities, consistent with the Bevy walker's own measured ~3.7x.

**Confirmed under real IL2CPP AOT.** `Expression.Compile()` needs JIT/codegen on most .NET runtimes, and IL2CPP has no `DynamicMethod`/`Reflection.Emit` — the concern was whether this would throw under AOT or silently fail. A check for exactly this (`Il2cppVerify.cs`'s reflective-hashing block) rides the same `bindings-unity-il2cpp` CI leg described above, and a real run on `windows-latest` printed:

```
ok   - reflective hashing (Expression.Compile-based accessors) records tagged fields under IL2CPP
ok   - reflective hashing's untagged field stays invisible under IL2CPP
```

`Expression.Compile()` does work under real IL2CPP — falls back to the BCL's expression interpreter as expected, not a throw.

See [Auto/Reflective Hashing](reflective-hashing.md) for the full cross-engine picture.
