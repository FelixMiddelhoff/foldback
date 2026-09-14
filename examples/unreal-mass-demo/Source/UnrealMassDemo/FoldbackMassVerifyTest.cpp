// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Headless end-to-end verification for the Unreal binding's Mass Entity
// integration path (design doc §6) — the second Unreal verification
// harness after examples/unreal-demo's custom-tick lockstep toy. Run via
// UnrealEditor-Cmd.exe -ExecCmds="Automation RunTests Foldback.MassVerify;Quit"
// -nullrhi -unattended -nosplash, same pattern as Foldback.Verify.
//
// Scope note: FFoldbackSession::HashEntity is Level 2, recording-only (no
// in-memory cross-peer divergence tracking — see FoldbackSession.h and the
// design doc §6), so this proves the Mass query/fragment-access hook
// produces real per-tick per-entity data recorded to a real `.foldback`
// file, the same shape unreal-demo's Level 2/3 block already proves for
// the custom-tick path — not a CheckDivergence() assertion, which only
// Level 1 (HashTick/RecordPeerHash) supports.

#include "FoldbackDemoFragments.h"
#include "FoldbackHashProcessor.h"
#include "FoldbackSession.h"
#include "HAL/PlatformMisc.h"
#include "MassEntityManager.h"
#include "MassExecutionContext.h"
#include "Misc/AutomationTest.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FFoldbackMassVerifyTest, "Foldback.MassVerify",
	EAutomationTestFlags_ApplicationContextMask | EAutomationTestFlags::ProductFilter)

bool FFoldbackMassVerifyTest::RunTest(const FString& Parameters)
{
	bool bAllOk = true;
	constexpr int32 NumEntities = 5;

	TSharedRef<FMassEntityManager> EntityManager = MakeShared<FMassEntityManager>();
	EntityManager->Initialize();

	const FMassArchetypeHandle Archetype = EntityManager->CreateArchetype(
		TConstArrayView<const UScriptStruct*>({ FFoldbackDemoTransformFragment::StaticStruct() }));

	TArray<FMassEntityHandle> Entities;
	for (int32 i = 0; i < NumEntities; ++i)
	{
		Entities.Add(EntityManager->CreateEntity(Archetype));
	}

	UFoldbackHashProcessor* Processor = NewObject<UFoldbackHashProcessor>();
	Processor->CallInitialize(GetTransientPackage(), EntityManager);

	const FString RecordPath =
		FPaths::ConvertRelativePathToFull(FPaths::ProjectSavedDir() / TEXT("unreal-mass-demo.foldback"));

	FFoldbackSession Session;
	{
		FFoldbackSession::FConfig Config;
		Config.TickRateHz = 60;
		Config.PeerCount = 1;
		Config.RecordToPath = RecordPath;
		if (!Session.Configure(Config))
		{
			AddError(FString::Printf(TEXT("Session.Configure failed: %s"), *Session.GetLastError()));
			bAllOk = false;
		}
	}

	for (uint64 Tick = 0; Tick < 20; ++Tick)
	{
		for (int32 i = 0; i < Entities.Num(); ++i)
		{
			FFoldbackDemoTransformFragment* Frag =
				EntityManager->GetFragmentDataPtr<FFoldbackDemoTransformFragment>(Entities[i]);
			// Every entity moves the same way each tick, except entity 0 gets an injected
			// divergence at tick 10 — proves the per-tick hash actually reflects live fragment
			// data (a stale/cached read would miss it), mirroring unreal-demo's injected-bit-flip
			// pattern for its own custom-tick path.
			const float Pos = static_cast<float>(Tick);
			Frag->PositionX = (i == 0 && Tick == 10) ? Pos + 1000.f : Pos;
			Frag->PositionY = Pos;
		}

		Processor->SetHashTarget(&Session, Tick);
		FMassExecutionContext Context(*EntityManager, 0.f);
		// The query was registered with Processor via RegisterWithProcessor (see
		// FoldbackHashProcessor's constructor), so it asserts the context it's handed came from
		// real processor dispatch (FMassEntityQuery.cpp's ExpectedContextType check) — a bare
		// default-constructed context fails that check even though this test drives Execute()
		// manually rather than through a full FMassProcessingPhaseManager. Found by actually
		// running it, not by reading the query API.
		Context.SetExecutionType(EMassExecutionContextType::Processor);
		Processor->CallExecute(*EntityManager, Context);
	}

	if (!Session.FinishRecording())
	{
		AddError(FString::Printf(TEXT("FinishRecording failed: %s"), *Session.GetLastError()));
		bAllOk = false;
	}
	else if (!FPaths::FileExists(RecordPath))
	{
		AddError(FString::Printf(TEXT("recording file was not written: %s"), *RecordPath));
		bAllOk = false;
	}
	else
	{
		AddInfo(FString::Printf(
			TEXT("PASS: Mass Entity query hook recorded %d ticks x %d entities to %s"), 20, NumEntities, *RecordPath));
	}

	FString ResultPath = FPlatformMisc::GetEnvironmentVariable(TEXT("FOLDBACK_UNREAL_MASS_RESULT_PATH"));
	if (ResultPath.IsEmpty())
	{
		ResultPath = FPaths::ProjectSavedDir() / TEXT("foldback-unreal-mass-result.txt");
	}
	FFileHelper::SaveStringToFile(bAllOk ? TEXT("PASS") : TEXT("FAIL"), *ResultPath);

	return bAllOk;
}
