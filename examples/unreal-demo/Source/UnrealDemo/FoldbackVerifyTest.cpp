// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Headless end-to-end verification for bindings/unreal, run via
// UnrealEditor-Cmd.exe -ExecCmds="Automation RunTests Foldback.Verify;Quit"
// -nullrhi -unattended -nosplash (the design doc's §8 testing pattern,
// matching Unity's batch mode / Godot's --headless). Not a unit-test
// framework exercise — a straight-line pass/fail check per binding
// feature, writing PASS/FAIL to a result file so CI can gate on it the
// same way bindings-unity-il2cpp does.

#include "FoldbackSession.h"
#include "HAL/PlatformMisc.h"
#include "Misc/AutomationTest.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
	FFoldbackVerifyTest, "Foldback.Verify", EAutomationTestFlags_ApplicationContextMask | EAutomationTestFlags::ProductFilter)

bool FFoldbackVerifyTest::RunTest(const FString& Parameters)
{
	bool bAllOk = true;

	// --- Level 1: cross-peer divergence detection ---
	FFoldbackSession SessionA;
	FFoldbackSession SessionB;
	{
		FFoldbackSession::FConfig ConfigA;
		ConfigA.TickRateHz = 60;
		ConfigA.PeerCount = 2;
		ConfigA.LocalPeerId = 0;
		if (!SessionA.Configure(ConfigA))
		{
			AddError(FString::Printf(TEXT("SessionA.Configure failed: %s"), *SessionA.GetLastError()));
			bAllOk = false;
		}

		FFoldbackSession::FConfig ConfigB;
		ConfigB.TickRateHz = 60;
		ConfigB.PeerCount = 2;
		ConfigB.LocalPeerId = 1;
		if (!SessionB.Configure(ConfigB))
		{
			AddError(FString::Printf(TEXT("SessionB.Configure failed: %s"), *SessionB.GetLastError()));
			bAllOk = false;
		}
	}

	for (uint64 Tick = 0; Tick < 20; ++Tick)
	{
		TArray<uint8> State;
		State.Add(static_cast<uint8>(Tick));
		SessionA.HashTick(Tick, State);
		SessionB.HashTick(Tick, State);
	}
	// Injected divergence at tick 10.
	{
		TArray<uint8> Divergent;
		Divergent.Add(255);
		SessionB.HashTick(10, Divergent);
	}

	// Toy in-process hash exchange — exercises the API surface, not a real transport.
	for (const TPair<uint64, uint64>& Pending : SessionA.TakePendingHashes(1000))
	{
		SessionB.RecordPeerHash(Pending.Key, 0, Pending.Value);
	}
	for (const TPair<uint64, uint64>& Pending : SessionB.TakePendingHashes(1000))
	{
		SessionA.RecordPeerHash(Pending.Key, 1, Pending.Value);
	}

	uint64 DivergedTick = 0;
	const bool bFound = SessionA.CheckDivergence(DivergedTick);
	if (!bFound || DivergedTick != 10)
	{
		AddError(FString::Printf(TEXT("expected divergence at tick 10, found=%d tick=%llu"), bFound, DivergedTick));
		bAllOk = false;
	}
	else
	{
		AddInfo(TEXT("PASS: divergence correctly detected at tick 10"));
	}

	// --- Level 2/3: entity + field hashing, recorded to a real .foldback file ---
	const FString RecordPath = FPaths::ConvertRelativePathToFull(FPaths::ProjectSavedDir() / TEXT("unreal-demo.foldback"));
	{
		FFoldbackSession RecSession;
		FFoldbackSession::FConfig RecConfig;
		RecConfig.TickRateHz = 60;
		RecConfig.PeerCount = 1;
		RecConfig.RecordToPath = RecordPath;

		if (!RecSession.Configure(RecConfig))
		{
			AddError(FString::Printf(TEXT("RecSession.Configure failed: %s"), *RecSession.GetLastError()));
			bAllOk = false;
		}
		else
		{
			TArray<uint8> EntityState = { 1, 2, 3 };
			if (!RecSession.HashEntity(5, 7, EntityState))
			{
				AddError(FString::Printf(TEXT("HashEntity failed: %s"), *RecSession.GetLastError()));
				bAllOk = false;
			}

			TArray<uint8> FieldValue;
			const float FieldFloat = 3.0f;
			FieldValue.Append(reinterpret_cast<const uint8*>(&FieldFloat), sizeof(FieldFloat));
			if (!RecSession.HashField(5, 7, TEXT("position.x"), FieldValue))
			{
				AddError(FString::Printf(TEXT("HashField failed: %s"), *RecSession.GetLastError()));
				bAllOk = false;
			}

			if (!RecSession.FinishRecording())
			{
				AddError(FString::Printf(TEXT("FinishRecording failed: %s"), *RecSession.GetLastError()));
				bAllOk = false;
			}
			else if (!FPaths::FileExists(RecordPath))
			{
				AddError(FString::Printf(TEXT("recording file was not written: %s"), *RecordPath));
				bAllOk = false;
			}
			else
			{
				AddInfo(FString::Printf(TEXT("PASS: Level 2/3 hashing recorded to %s"), *RecordPath));
			}
		}
	}

	// --- finish() agreement / disagreement across independent sessions ---
	{
		FFoldbackSession S1;
		FFoldbackSession S2;
		FFoldbackSession::FConfig Config1x1;
		Config1x1.TickRateHz = 60;
		Config1x1.PeerCount = 1;
		S1.Configure(Config1x1);
		S2.Configure(Config1x1);

		for (uint64 Tick = 0; Tick < 5; ++Tick)
		{
			TArray<uint8> State;
			State.Add(static_cast<uint8>(Tick));
			S1.HashTick(Tick, State);
			S2.HashTick(Tick, State);
		}
		if (S1.Finish() != S2.Finish())
		{
			AddError(TEXT("finish() should agree for identical input"));
			bAllOk = false;
		}

		TArray<uint8> ExtraState;
		ExtraState.Add(255);
		S2.HashTick(5, ExtraState);
		if (S1.Finish() == S2.Finish())
		{
			AddError(TEXT("finish() should disagree after a divergent extra tick"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: finish() agreement/disagreement correct"));
		}
	}

	// --- error path: hashing before Configure() is a clean no-crash error ---
	{
		FFoldbackSession Unconfigured;
		TArray<uint8> Dummy;
		Dummy.Add(1);
		const bool bHashOk = Unconfigured.HashTick(0, Dummy);
		if (bHashOk)
		{
			AddError(TEXT("HashTick on an unconfigured session should not succeed"));
			bAllOk = false;
		}
		else if (Unconfigured.GetLastError().IsEmpty())
		{
			AddError(TEXT("unconfigured session should set GetLastError()"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: unconfigured session reports a clean error, not a crash"));
		}
	}

	FString ResultPath = FPlatformMisc::GetEnvironmentVariable(TEXT("FOLDBACK_UNREAL_RESULT_PATH"));
	if (ResultPath.IsEmpty())
	{
		ResultPath = FPaths::ProjectSavedDir() / TEXT("foldback-unreal-result.txt");
	}
	FFileHelper::SaveStringToFile(bAllOk ? TEXT("PASS") : TEXT("FAIL"), *ResultPath);

	return bAllOk;
}
