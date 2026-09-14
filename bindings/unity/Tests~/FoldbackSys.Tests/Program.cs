// SPDX-License-Identifier: MIT OR Apache-2.0
// Proves the C# P/Invoke binding (Runtime/*.cs) actually works against a
// real, built foldback_sys native library — struct marshaling, string
// handling, and divergence detection, the parts a Rust-only test can't
// exercise. Not xUnit/NUnit: a hand-rolled harness keeps this dependency-free
// (no NuGet restore needed) to match the rest of the repo's examples, which
// are plain binaries that assert their own correctness rather than compiled
// specs.
using System;
using System.IO;
using Foldback;

var failures = 0;

void Check(bool condition, string what)
{
    if (condition)
    {
        Console.WriteLine($"ok   - {what}");
    }
    else
    {
        Console.WriteLine($"FAIL - {what}");
        failures++;
    }
}

// hash_tick + take_pending_hashes round-trip.
using (var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 2 }))
{
    session.HashTick(0, new byte[] { 1, 2, 3, 4 });
    var pending = session.TakePendingHashes();
    Check(pending.Length == 1, "hash_tick produces exactly one pending hash");
    Check(pending.Length == 0 || pending[0].Tick == 0, "the pending hash is for tick 0");
    Check(session.TakePendingHashes().Length == 0, "take_pending_hashes drains fully");
}

// Mismatched peer hashes are detected as a divergence.
using (var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 2 }))
{
    session.HashTick(5, new byte[] { 1 });
    session.RecordPeerHash(5, 1, 0xdeadbeef);
    var found = session.CheckDivergence(out var tick);
    Check(found, "mismatched peer hashes are detected as a divergence");
    Check(tick == 5, "the reported divergence tick is correct");
}

// finish() agrees between two clean runs, disagrees once one diverges.
using (var a = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 }))
using (var b = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 }))
{
    for (ulong tick = 0; tick < 5; tick++)
    {
        var state = new byte[] { (byte)tick };
        a.HashTick(tick, state);
        b.HashTick(tick, state);
    }
    Check(a.Finish() == b.Finish(), "two identical runs' finish() values agree");

    b.HashTick(5, new byte[] { 0xff });
    Check(a.Finish() != b.Finish(), "a diverging run's finish() value disagrees");
}

// Recording to a file round-trips through finish_recording.
{
    var path = Path.Combine(Path.GetTempPath(), $"foldback-cs-test-{Guid.NewGuid():N}.foldback");
    try
    {
        using (var session = new FoldbackSession(new FoldbackConfig
        {
            TickRateHz = 60,
            PeerCount = 1,
            RecordToPath = path,
        }))
        {
            session.HashTick(0, new byte[] { 9 });
            session.FinishRecording();
        }
        Check(File.Exists(path), "record_to_path creates the session file");
        Check(new FileInfo(path).Length > 0, "the recorded session file is non-empty");
    }
    finally
    {
        if (File.Exists(path))
        {
            File.Delete(path);
        }
    }
}

// A bad BuildId length is rejected before it ever reaches native code.
try
{
    _ = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1, BuildId = new byte[3] });
    Check(false, "a 3-byte BuildId is rejected");
}
catch (ArgumentException)
{
    Check(true, "a 3-byte BuildId is rejected");
}

// A disposed session throws instead of touching a freed native pointer.
{
    var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    session.Dispose();
    try
    {
        session.HashTick(0, Array.Empty<byte>());
        Check(false, "using a disposed session throws");
    }
    catch (ObjectDisposedException)
    {
        Check(true, "using a disposed session throws");
    }
}

Console.WriteLine();
if (failures == 0)
{
    Console.WriteLine("all checks passed");
    return 0;
}
Console.WriteLine($"{failures} check(s) FAILED");
return 1;
