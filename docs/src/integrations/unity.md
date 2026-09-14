# Unity

## Status

**Phase 3 in progress.** `foldback-sys` now has a real C ABI (Level 1 / per-tick only — `foldback_session_create`, `foldback_hash_tick`, `foldback_record_peer_hash`, `foldback_take_pending_hashes`, `foldback_check_divergence`, `foldback_finish`, `foldback_finish_recording`), status-code-based per [RFC-0002](../project/rfcs/0002-c-abi-surface.md), with a `cbindgen`-generated `foldback.h`. A real C# UPM package (`bindings/unity/`) wraps it — `FoldbackSession`/`FoldbackConfig` matching [cookbook recipe 8](../cookbook/README.md#8-unity-integration) exactly.

Verified against the real, built native library (not just compiled): a standalone .NET console harness (`bindings/unity/Tests~/FoldbackSys.Tests`, a `Tests~` folder — Unity's own convention for "ignore during asset import") P/Invokes the actual `foldback_sys` binary and asserts real behavior — struct marshaling, divergence detection, `finish()` agreement/disagreement across two sessions, file recording, and error handling all pass against real native code.

**IL2CPP AOT — verified locally, CI leg not yet proven in CI.** `examples/unity-demo` is a real Unity project (`Assets/Il2cppVerify.cs`) referencing `com.foldback.unity` as a real package dependency, built as a Standalone Windows IL2CPP player and actually run — not just compiled. With a real Unity 6000.6.0f1 + Windows Build Support (IL2CPP) install, `BuildScript.BuildIl2Cpp` produced a working IL2CPP player, and running it printed:

```
ok   - hash_tick produces exactly one pending hash
ok   - mismatched peer hashes detected as divergence at the correct tick
ok   - two identical runs' finish() values agree
ok   - a diverging run's finish() value disagrees
PASS
```

— the P/Invoke surface's conservative design (simple `[DllImport]` signatures, no automatic string/array marshaling attributes on the config struct — a hand-managed native UTF-8 buffer instead, see `Runtime/FoldbackNative.cs`'s comments) genuinely survives real IL2CPP AOT compilation, not just in theory.

The CI job `bindings-unity-il2cpp` (`.github/workflows/ci.yml`) codifies this exact sequence for regression protection, but is honestly unverified-in-CI as of this writing: it needs `UNITY_EMAIL`/`UNITY_PASSWORD`/`UNITY_SERIAL` repository secrets for a Unity license (only a maintainer with a Unity account can add these — same class of gap as the UI's code-signing secrets), and its first real run hasn't happened yet to confirm the exact `buildalon/unity-setup`/`activate-unity-license` invocation works unattended the way the manual local run did. Once those secrets exist and the job goes green once, this becomes a fully closed Phase-3 "definition of done" item per the risk register (P1); until then, treat the *binding* as proven and the *CI automation* as a good-faith implementation awaiting its first real run.

Level 2/3 (entity/field) hashing across this FFI boundary is a second, separate deliberate gap — `foldback-sys` itself doesn't expose it yet.

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
