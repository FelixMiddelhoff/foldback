# Unity

## Status

**Phase 3 in progress.** `foldback-sys` now has a real C ABI (Level 1 / per-tick only — `foldback_session_create`, `foldback_hash_tick`, `foldback_record_peer_hash`, `foldback_take_pending_hashes`, `foldback_check_divergence`, `foldback_finish`, `foldback_finish_recording`), status-code-based per [RFC-0002](../project/rfcs/0002-c-abi-surface.md), with a `cbindgen`-generated `foldback.h`. A real C# UPM package (`bindings/unity/`) wraps it — `FoldbackSession`/`FoldbackConfig` matching [cookbook recipe 8](../cookbook/README.md#8-unity-integration) exactly.

Verified against the real, built native library (not just compiled): a standalone .NET console harness (`bindings/unity/Tests~/FoldbackSys.Tests`, a `Tests~` folder — Unity's own convention for "ignore during asset import") P/Invokes the actual `foldback_sys` binary and asserts real behavior — struct marshaling, divergence detection, `finish()` agreement/disagreement across two sessions, file recording, and error handling all pass against real native code.

**Known, deliberate gap, not silently dropped**: the IL2CPP AOT build-and-run CI leg that the risk register (P1, platform/FFI risks) calls a hard requirement for calling this phase done needs an actual Unity Editor install with the IL2CPP module — not available in the environment this binding was built in. The P/Invoke surface was designed conservatively with IL2CPP's known restrictions in mind regardless (simple `[DllImport]` signatures, no automatic string/array marshaling attributes on the config struct — a hand-managed native UTF-8 buffer instead, see `Runtime/FoldbackNative.cs`'s comments), but that design choice is unverified against real IL2CPP until someone with Unity installed adds the CI leg. Level 2/3 (entity/field) hashing across this FFI boundary is a second, separate deliberate gap — `foldback-sys` itself doesn't expose it yet.

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
