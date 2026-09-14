// SPDX-License-Identifier: MIT OR Apache-2.0
#include "FoldbackSubsystem.h"

void UFoldbackSubsystem::Deinitialize()
{
	Session = FFoldbackSession();
	Super::Deinitialize();
}

bool UFoldbackSubsystem::Configure(const FFoldbackConfigBP& Config)
{
	FFoldbackSession::FConfig Native;
	Native.TickRateHz = static_cast<uint32>(Config.TickRateHz);
	Native.PeerCount = static_cast<uint32>(Config.PeerCount);
	Native.LocalPeerId = static_cast<uint16>(Config.LocalPeerId);
	Native.Retention = static_cast<uint32>(Config.Retention);
	Native.RecordToPath = Config.RecordToPath;
	return Session.Configure(Native);
}

bool UFoldbackSubsystem::HashTick(int64 Tick, const TArray<uint8>& State)
{
	return Session.HashTick(static_cast<uint64>(Tick), State);
}

bool UFoldbackSubsystem::RecordPeerHash(int64 Tick, int32 PeerId, int64 Hash)
{
	return Session.RecordPeerHash(static_cast<uint64>(Tick), static_cast<uint16>(PeerId), static_cast<uint64>(Hash));
}

bool UFoldbackSubsystem::HashEntity(int64 Tick, int64 EntityId, const TArray<uint8>& State)
{
	return Session.HashEntity(static_cast<uint64>(Tick), static_cast<uint64>(EntityId), State);
}

bool UFoldbackSubsystem::RecordPeerEntityHash(int64 Tick, int32 PeerId, int64 EntityId, int64 Hash)
{
	return Session.RecordPeerEntityHash(static_cast<uint64>(Tick), static_cast<uint16>(PeerId),
		static_cast<uint64>(EntityId), static_cast<uint64>(Hash));
}

bool UFoldbackSubsystem::HashField(int64 Tick, int64 EntityId, const FString& FieldName, const TArray<uint8>& Value)
{
	return Session.HashField(static_cast<uint64>(Tick), static_cast<uint64>(EntityId), FieldName, Value);
}

bool UFoldbackSubsystem::RecordPeerFieldHash(
	int64 Tick, int32 PeerId, int64 EntityId, const FString& FieldName, int64 Hash, const TArray<uint8>& Value)
{
	return Session.RecordPeerFieldHash(static_cast<uint64>(Tick), static_cast<uint16>(PeerId),
		static_cast<uint64>(EntityId), FieldName, static_cast<uint64>(Hash), Value);
}

int64 UFoldbackSubsystem::PendingHashCount() const
{
	return static_cast<int64>(Session.PendingHashCount());
}

TArray<FFoldbackPendingHashBP> UFoldbackSubsystem::TakePendingHashes(int64 Max)
{
	TArray<FFoldbackPendingHashBP> Out;
	if (Max <= 0)
	{
		return Out;
	}
	for (const TPair<uint64, uint64>& Pair : Session.TakePendingHashes(static_cast<uint64>(Max)))
	{
		FFoldbackPendingHashBP& Entry = Out.AddDefaulted_GetRef();
		Entry.Tick = static_cast<int64>(Pair.Key);
		Entry.Hash = static_cast<int64>(Pair.Value);
	}
	return Out;
}

bool UFoldbackSubsystem::CheckDivergence(int64& OutTick)
{
	uint64 Tick = 0;
	const bool bFound = Session.CheckDivergence(Tick);
	if (bFound)
	{
		OutTick = static_cast<int64>(Tick);
	}
	return bFound;
}

int64 UFoldbackSubsystem::Finish() const
{
	return static_cast<int64>(Session.Finish());
}

bool UFoldbackSubsystem::FinishRecording()
{
	return Session.FinishRecording();
}
