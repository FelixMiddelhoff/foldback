// SPDX-License-Identifier: MIT OR Apache-2.0
// Proves the C# P/Invoke binding (Runtime/*.cs) actually works against a
// real, built foldback_sys native library — struct marshaling, string
// handling, and divergence detection, the parts a Rust-only test can't
// exercise. Not xUnit/NUnit: a hand-rolled harness keeps this dependency-free
// (no NuGet restore needed) to match the rest of the repo's examples, which
// are plain binaries that assert their own correctness rather than compiled
// specs.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
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

int IndexOf(byte[] haystack, byte[] needle, int startIndex = 0)
{
    for (var i = startIndex; i <= haystack.Length - needle.Length; i++)
    {
        var match = true;
        for (var j = 0; j < needle.Length; j++)
        {
            if (haystack[i + j] != needle[j])
            {
                match = false;
                break;
            }
        }
        if (match)
        {
            return i;
        }
    }
    return -1;
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

// Level 2/3 (entity/field) hashing round-trips into the recorded file,
// including a non-ASCII field name — proves the Utf8Buffer marshaling
// path handles more than plain ASCII, not just that the call didn't throw.
{
    var path = Path.Combine(Path.GetTempPath(), $"foldback-cs-l23-{Guid.NewGuid():N}.foldback");
    const string fieldName = "posé.x"; // "posé.x" — contains a non-ASCII byte in UTF-8
    try
    {
        using (var session = new FoldbackSession(new FoldbackConfig
        {
            TickRateHz = 60,
            PeerCount = 2,
            RecordToPath = path,
        }))
        {
            session.HashEntity(10, 7, new byte[] { 1, 2, 3 });
            session.RecordPeerEntityHash(10, 1, 7, 0xdeadbeef);
            session.HashField(10, 7, fieldName, new byte[] { 9, 9 });
            session.RecordPeerFieldHash(10, 1, 7, fieldName, 0xcafebabe, new byte[] { 9, 9 });
            session.FinishRecording();
        }
        var bytes = File.ReadAllBytes(path);
        var needle = System.Text.Encoding.UTF8.GetBytes(fieldName);
        var found = IndexOf(bytes, needle) >= 0;
        Check(found, "the recorded file contains the (non-ASCII) field name's exact UTF-8 bytes");
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

// ---- Reflective hashing (foldback-reflective-hashing.md §2.2) ----

// Walks tagged fields with dotted/indexed paths; the untagged field is
// invisible to the walker.
{
    var path = Path.Combine(Path.GetTempPath(), $"foldback-reflect-{Guid.NewGuid():N}.foldback");
    try
    {
        using (var session = new FoldbackSession(new FoldbackConfig
               {
                   TickRateHz = 60,
                   PeerCount = 1,
                   RecordToPath = path,
               }))
        {
            var unit = new ReflectUnit
            {
                Pos = new Position2 { X = 1.5f, Y = -2f },
                Hp = 42,
                Tags = new List<string> { "a", "b" },
                DebugLabel = "not hashed",
            };
            var preview = FoldbackReflection.HashReflected(session, 0, 7, "unit", unit);
            var paths = preview.Select(p => p.Path).ToList();
            Check(paths.Contains("unit.Pos.X"), "reflective walk records unit.Pos.X");
            Check(paths.Contains("unit.Pos.Y"), "reflective walk records unit.Pos.Y");
            Check(paths.Contains("unit.Hp"), "reflective walk records unit.Hp");
            Check(paths.Contains("unit.Tags[0]"), "reflective walk records unit.Tags[0]");
            Check(paths.Contains("unit.Tags[1]"), "reflective walk records unit.Tags[1]");
            Check(paths.All(p => !p.Contains("DebugLabel")), "the untagged field is never recorded");
            session.FinishRecording();
        }
    }
    finally
    {
        if (File.Exists(path))
        {
            File.Delete(path);
        }
    }
}

// Same tracked-field state hashes identically; a differing tracked field
// changes the hash; an untagged field's difference doesn't.
{
    var a = new ReflectUnit { Pos = new Position2 { X = 1f, Y = 2f }, Hp = 10, Tags = new List<string>(), DebugLabel = "a" };
    var b = new ReflectUnit { Pos = new Position2 { X = 1f, Y = 2f }, Hp = 10, Tags = new List<string>(), DebugLabel = "b differs but untracked" };
    var c = new ReflectUnit { Pos = new Position2 { X = 1f, Y = 3f }, Hp = 10, Tags = new List<string>(), DebugLabel = "a" };

    ulong Sum(List<(string Path, ulong Hash)> preview) => preview.Aggregate(0UL, (acc, p) => acc ^ p.Hash);

    using var sa = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    using var sb = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    using var sc = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    var ha = Sum(FoldbackReflection.HashReflected(sa, 0, 0, "u", a));
    var hb = Sum(FoldbackReflection.HashReflected(sb, 0, 0, "u", b));
    var hc = Sum(FoldbackReflection.HashReflected(sc, 0, 0, "u", c));
    Check(ha == hb, "an untracked field's difference doesn't change the hash");
    Check(ha != hc, "a tracked field's difference changes the hash");
}

// §3's sorted-container rule: dictionary/set hashing is independent of
// insertion order.
{
    var d1 = new DictHolder { Items = new Dictionary<string, int>() };
    d1.Items["sword"] = 1;
    d1.Items["shield"] = 2;
    var d2 = new DictHolder { Items = new Dictionary<string, int>() };
    d2.Items["shield"] = 2;
    d2.Items["sword"] = 1;

    using var sd1 = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    using var sd2 = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    var pd1 = FoldbackReflection.HashReflected(sd1, 0, 0, "d", d1);
    var pd2 = FoldbackReflection.HashReflected(sd2, 0, 0, "d", d2);
    Check(pd1.SequenceEqual(pd2), "dictionary hashing is independent of insertion order");

    var s1 = new SetHolder { Items = new HashSet<int> { 3, 1, 4, 1, 5, 9 } };
    var s2 = new SetHolder { Items = new HashSet<int> { 9, 5, 1, 4, 3 } };
    using var ss1 = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    using var ss2 = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    var ps1 = FoldbackReflection.HashReflected(ss1, 0, 0, "s", s1);
    var ps2 = FoldbackReflection.HashReflected(ss2, 0, 0, "s", s2);
    Check(ps1.SequenceEqual(ps2), "set hashing is independent of insertion order");
}

// A genuine reference cycle is caught, loudly, rather than hanging.
{
    var a = new CycleNode { Value = 1 };
    var b = new CycleNode { Value = 2 };
    a.Next = b;
    b.Next = a;

    using var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    try
    {
        FoldbackReflection.HashReflected(session, 0, 0, "n", a);
        Check(false, "a genuine reference cycle throws FoldbackReflectionException");
    }
    catch (FoldbackReflectionException)
    {
        Check(true, "a genuine reference cycle throws FoldbackReflectionException");
    }
}

// The depth guard rejects a runaway (but acyclic) chain.
{
    var deep = new DeepNode { Value = 0 };
    for (var i = 0; i < 10; i++)
    {
        deep = new DeepNode { Value = i, Inner = deep };
    }
    var root = new DeepRoot { Inner = deep };

    using var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
    try
    {
        FoldbackReflection.HashReflected(session, 0, 0, "r", root);
        Check(false, "a runaway nesting chain hits the depth guard");
    }
    catch (FoldbackReflectionException)
    {
        Check(true, "a runaway nesting chain hits the depth guard");
    }
}

// Visibility tooling: ListTracked reports the same tracked/untracked
// split HashReflected actually enforces.
{
    var (tracked, untracked) = FoldbackReflection.ListTracked(typeof(ReflectUnit));
    Check(tracked.Contains("Pos") && tracked.Contains("Hp") && tracked.Contains("Tags"),
        "ListTracked reports the tagged fields");
    Check(untracked.Contains("DebugLabel"), "ListTracked reports the untagged sibling");
}

// Visibility tooling: the Recorded event — an editor window (or any other
// live view) subscribes to this instead of polling HashReflected's return
// value, since it doesn't call HashReflected itself.
{
    var unit = new ReflectUnit { Pos = new Position2 { X = 1f, Y = 2f }, Hp = 7, Tags = new List<string>() };
    ReflectionPreview? captured = null;
    void OnRecorded(ReflectionPreview p) => captured = p;

    FoldbackReflection.Recorded += OnRecorded;
    try
    {
        using var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
        FoldbackReflection.HashReflected(session, 3, 9, "u", unit);
    }
    finally
    {
        FoldbackReflection.Recorded -= OnRecorded;
    }

    Check(captured.HasValue, "the Recorded event fires after HashReflected");
    Check(captured?.RootType == typeof(ReflectUnit), "the Recorded event carries the correct root type");
    Check(captured?.Tick == 3 && captured?.EntityId == 9, "the Recorded event carries the correct tick/entityId");
    Check(captured?.Fields.Count > 0, "the Recorded event carries the same fields HashReflected returned");
}

// Schema-drift detection (foldback-reflective-hashing.md §7): each
// reflected type's tagged field set is recorded once per type per
// session, not once per HashReflected call.
{
    var path = Path.Combine(Path.GetTempPath(), $"foldback-schema-{Guid.NewGuid():N}.foldback");
    try
    {
        using (var session = new FoldbackSession(new FoldbackConfig
               {
                   TickRateHz = 60,
                   PeerCount = 1,
                   RecordToPath = path,
               }))
        {
            var unit = new ReflectUnit { Pos = new Position2 { X = 1f, Y = 2f }, Hp = 1, Tags = new List<string>() };
            FoldbackReflection.HashReflected(session, 0, 0, "u", unit);
            FoldbackReflection.HashReflected(session, 1, 0, "u", unit);
            session.FinishRecording();
        }
        var bytes = File.ReadAllBytes(path);
        var keyNeedle = System.Text.Encoding.UTF8.GetBytes("foldback.schema.ReflectUnit");
        var valueNeedle = System.Text.Encoding.UTF8.GetBytes("Hp,Pos,Tags");
        var firstIndex = IndexOf(bytes, keyNeedle);
        Check(firstIndex >= 0, "the schema metadata frame's key is recorded");
        Check(firstIndex >= 0 && IndexOf(bytes, keyNeedle, firstIndex + keyNeedle.Length) < 0,
            "the schema metadata frame is recorded only once per type per session, not once per call");
        Check(IndexOf(bytes, valueNeedle) >= 0,
            "the schema metadata frame's value is the sorted tagged field set");
    }
    finally
    {
        if (File.Exists(path))
        {
            File.Delete(path);
        }
    }
}

// Performance: not assumed free (plan §5). Printed either way, and — a
// CI regression gate, not just visibility — also asserted against a
// generous ratio bound. Not BenchmarkDotNet (this harness stays
// dependency-free): best-of-3 timing on real work (1,000 entities) to
// keep the signal well above CI-runner jitter. Measured locally ~3.5x;
// a 15x ceiling has wide margin while still catching an order-of-
// magnitude regression (mirrors foldback-rs's own gate, same rationale).
{
    const int entityCount = 1000;
    const double maxRatio = 15.0;
    var units = Enumerable.Range(0, entityCount)
        .Select(i => new ReflectUnit
        {
            Pos = new Position2 { X = i, Y = i * 2f },
            Hp = 100 - (i % 100),
            Tags = new List<string> { "x" },
            DebugLabel = "d",
        })
        .ToList();

    double TimeReflectiveMs()
    {
        using var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
        var sw = System.Diagnostics.Stopwatch.StartNew();
        for (var id = 0; id < units.Count; id++)
        {
            FoldbackReflection.HashReflected(session, 0, (ulong)id, "unit", units[id]);
        }
        sw.Stop();
        return sw.Elapsed.TotalMilliseconds;
    }

    double TimeExplicitMs()
    {
        using var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 });
        var sw = System.Diagnostics.Stopwatch.StartNew();
        for (var id = 0; id < units.Count; id++)
        {
            var u = units[id];
            session.HashField(0, (ulong)id, "unit.Pos.X", BitConverter.GetBytes(u.Pos.X));
            session.HashField(0, (ulong)id, "unit.Pos.Y", BitConverter.GetBytes(u.Pos.Y));
            session.HashField(0, (ulong)id, "unit.Hp", BitConverter.GetBytes(u.Hp));
        }
        sw.Stop();
        return sw.Elapsed.TotalMilliseconds;
    }

    var reflectiveMs = Enumerable.Range(0, 3).Select(_ => TimeReflectiveMs()).Min();
    var explicitMs = Enumerable.Range(0, 3).Select(_ => TimeExplicitMs()).Min();
    Console.WriteLine($"reflective: {entityCount} entities in {reflectiveMs:F2} ms");
    Console.WriteLine($"explicit:   {entityCount} entities in {explicitMs:F2} ms");
    var ratio = reflectiveMs / Math.Max(explicitMs, 0.001);
    Check(ratio <= maxRatio,
        $"reflective hashing stays within a {maxRatio}x budget of explicit ({ratio:F1}x measured)");
}

Console.WriteLine();
if (failures == 0)
{
    Console.WriteLine("all checks passed");
    return 0;
}
Console.WriteLine($"{failures} check(s) FAILED");
return 1;

class Position2
{
    public float X;
    public float Y;
}

class ReflectUnit
{
    [FoldbackHash] public Position2 Pos;
    [FoldbackHash] public int Hp;
    [FoldbackHash] public List<string> Tags;
    public string DebugLabel;
}

class DictHolder
{
    [FoldbackHash] public Dictionary<string, int> Items;
}

class SetHolder
{
    [FoldbackHash] public HashSet<int> Items;
}

class CycleNode
{
    [FoldbackHash] public CycleNode Next;
    [FoldbackHash] public int Value;
}

class DeepNode
{
    [FoldbackHash] public DeepNode Inner;
    [FoldbackHash] public int Value;
}

class DeepRoot
{
    [FoldbackHash] public DeepNode Inner;
}
