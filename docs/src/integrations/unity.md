# Unity

## Status

**Shipped** (Phase 3). `foldback-sys` has a real C ABI covering Level 1 (per-tick), Level 2 (per-entity), and Level 3 (per-field) hashing — status-code-based per [RFC-0002](../project/rfcs/0002-c-abi-surface.md), with a `cbindgen`-generated `foldback.h`. A real C# UPM package (`bindings/unity/`) wraps it — `FoldbackSession`/`FoldbackConfig` matching [cookbook recipe 8](../cookbook/README.md#8-unity-integration) exactly.

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

The CI job `bindings-unity-il2cpp` (`.github/workflows/ci.yml`) codifies this exact sequence and has now passed a real run in GitHub Actions on `windows-latest`, with `UNITY_EMAIL`/`UNITY_PASSWORD` repository secrets for a Unity Personal license — not just locally. This closes the risk register's P1 "definition of done" requirement for Phase 3. The Unity Hub/Editor/IL2CPP download is now cached between CI runs (a cache miss — first run ever, or after 7 days of disuse — is still slow, but every run after that should be fast).

## Who this is for

Unity projects using a lockstep or rollback netcode solution, including ones targeting IL2CPP (AOT compilation) — see the gap above for what's unverified there specifically.

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

## Planning detail

The IL2CPP AOT marshaling risk and its mitigation plan: see the project's risk register (P1, platform/FFI risks).
