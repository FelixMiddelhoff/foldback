using System;
using System.IO;
using System.Text;
using Foldback;
using UnityEngine;

// Runs FoldbackSession end-to-end inside a real IL2CPP AOT player build —
// proves the P/Invoke binding survives IL2CPP's ahead-of-time compilation,
// which the Editor's own Mono/CoreCLR runtime can't test (risk-plan P1).
// Mirrors bindings/unity/Tests~/FoldbackSys.Tests/Program.cs's checks.
public static class Il2cppVerify
{
    [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
    private static void Run()
    {
        var failures = 0;
        var log = new StringBuilder();

        void Check(bool condition, string what)
        {
            log.AppendLine((condition ? "ok   - " : "FAIL - ") + what);
            if (!condition)
            {
                failures++;
            }
        }

        try
        {
            using (var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 2 }))
            {
                session.HashTick(0, new byte[] { 1, 2, 3, 4 });
                var pending = session.TakePendingHashes();
                Check(pending.Length == 1, "hash_tick produces exactly one pending hash");
            }

            using (var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 2 }))
            {
                session.HashTick(5, new byte[] { 1 });
                session.RecordPeerHash(5, 1, 0xdeadbeef);
                var found = session.CheckDivergence(out var tick);
                Check(found && tick == 5, "mismatched peer hashes detected as divergence at the correct tick");
            }

            using (var a = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 }))
            using (var b = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 }))
            {
                for (ulong t = 0; t < 5; t++)
                {
                    var state = new byte[] { (byte)t };
                    a.HashTick(t, state);
                    b.HashTick(t, state);
                }
                Check(a.Finish() == b.Finish(), "two identical runs' finish() values agree");

                b.HashTick(5, new byte[] { 0xff });
                Check(a.Finish() != b.Finish(), "a diverging run's finish() value disagrees");
            }

            // Level 2/3: the field-name string-marshaling path (Utf8Buffer)
            // is exactly the kind of thing IL2CPP AOT can behave
            // differently on (risk-plan P1) — worth proving here, not just
            // in the Editor-run .NET harness. Non-ASCII on purpose.
            {
                var path = Path.Combine(Application.temporaryCachePath, "il2cpp-l23-check.foldback");
                const string fieldName = "posé.x";
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
                var needle = Encoding.UTF8.GetBytes(fieldName);
                var found = false;
                for (var i = 0; i <= bytes.Length - needle.Length && !found; i++)
                {
                    found = true;
                    for (var j = 0; j < needle.Length; j++)
                    {
                        if (bytes[i + j] != needle[j])
                        {
                            found = false;
                            break;
                        }
                    }
                }
                Check(found, "Level 2/3 hashing recorded, non-ASCII field name survives IL2CPP marshaling intact");
            }

            // Reflective hashing (foldback-reflective-hashing.md §2.2):
            // the one part of this that's a genuine, not-yet-confirmed
            // IL2CPP unknown (risk-plan P1) — FoldbackReflection caches
            // per-type field/property accessors via
            // System.Linq.Expressions.Expression.Compile(), which needs
            // JIT/codegen on most .NET runtimes but is expected to fall
            // back to Unity's expression *interpreter* under IL2CPP AOT
            // (no DynamicMethod/Reflection.Emit there). This check exists
            // specifically to confirm that fallback actually works here,
            // rather than assuming it — same "verify against the real
            // engine constraint" standard the rest of this file already
            // holds itself to.
            {
                using (var session = new FoldbackSession(new FoldbackConfig { TickRateHz = 60, PeerCount = 1 }))
                {
                    var unit = new Il2cppReflectUnit
                    {
                        Pos = new Il2cppPosition { X = 1.5f, Y = -2f },
                        Hp = 42,
                        DebugLabel = "not hashed",
                    };
                    var preview = FoldbackReflection.HashReflected(session, 0, 3, "unit", unit);
                    var paths = new System.Collections.Generic.List<string>();
                    foreach (var (p, _) in preview) paths.Add(p);
                    Check(paths.Contains("unit.Pos.X") && paths.Contains("unit.Pos.Y") && paths.Contains("unit.Hp"),
                        "reflective hashing (Expression.Compile-based accessors) records tagged fields under IL2CPP");
                    Check(!paths.Contains("unit.DebugLabel"),
                        "reflective hashing's untagged field stays invisible under IL2CPP");
                }
            }
        }
        catch (Exception e)
        {
            log.AppendLine("EXCEPTION: " + e);
            failures++;
        }

        var resultPath = Environment.GetEnvironmentVariable("FOLDBACK_IL2CPP_RESULT_PATH");
        if (string.IsNullOrEmpty(resultPath))
        {
            resultPath = Path.Combine(Application.persistentDataPath, "il2cpp-verify-result.txt");
        }
        File.WriteAllText(resultPath, log.ToString() + Environment.NewLine + (failures == 0 ? "PASS" : "FAIL:" + failures));

        Application.Quit(failures == 0 ? 0 : 1);
    }
}

internal sealed class Il2cppPosition
{
    public float X;
    public float Y;
}

internal sealed class Il2cppReflectUnit
{
    [FoldbackHash] public Il2cppPosition Pos;
    [FoldbackHash] public int Hp;
    public string DebugLabel;
}
