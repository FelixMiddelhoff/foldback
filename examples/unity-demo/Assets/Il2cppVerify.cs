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
