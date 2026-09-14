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
