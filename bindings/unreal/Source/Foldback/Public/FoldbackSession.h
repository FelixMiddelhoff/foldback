// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include "CoreMinimal.h"

struct FoldbackHandle;

/**
 * RAII wrapper around foldback-sys's C ABI (see
 * Source/ThirdParty/Foldback/include/foldback.h) — owns one session
 * handle, not copyable, movable. Plain C++, not a `UObject`: usable both
 * from `UFoldbackSubsystem` (the Blueprint-facing wrapper) and directly
 * from C++, including this plugin's own Automation Test, without needing
 * a `GameInstance`.
 *
 * Every method here maps 1:1 onto one `foldback_*` C call and folds its
 * `FoldbackStatus` into a `bool` + `GetLastError()`, matching how the
 * Unity (`FoldbackSession.cs`) and Godot (`FoldbackSession` GDExtension
 * class) bindings both already surface the same status-code convention.
 */
class FOLDBACK_API FFoldbackSession
{
public:
	FFoldbackSession() = default;
	~FFoldbackSession();

	FFoldbackSession(const FFoldbackSession&) = delete;
	FFoldbackSession& operator=(const FFoldbackSession&) = delete;
	FFoldbackSession(FFoldbackSession&& Other) noexcept;
	FFoldbackSession& operator=(FFoldbackSession&& Other) noexcept;

	struct FOLDBACK_API FConfig
	{
		uint32 TickRateHz = 60;
		uint32 PeerCount = 1;
		uint16 LocalPeerId = 0;
		/** Ticks of ring-buffer retention. 0 = library default. */
		uint32 Retention = 0;
		/** Empty = don't record to a file. */
		FString RecordToPath;
	};

	/** Builds the underlying session. Returns false and sets GetLastError() on failure. */
	bool Configure(const FConfig& Config);

	bool IsConfigured() const { return Handle != nullptr; }

	/** Most recent error from Configure() or a failed call below, empty if none. */
	const FString& GetLastError() const { return LastError; }

	bool HashTick(uint64 Tick, TArrayView<const uint8> State);
	bool RecordPeerHash(uint64 Tick, uint16 PeerId, uint64 Hash);

	/** Level 2. Recording-only — matches foldback_core::Session::hash_entity's own scope. */
	bool HashEntity(uint64 Tick, uint64 EntityId, TArrayView<const uint8> State);
	bool RecordPeerEntityHash(uint64 Tick, uint16 PeerId, uint64 EntityId, uint64 Hash);

	/** Level 3. Recording-only, same caveat as HashEntity. */
	bool HashField(uint64 Tick, uint64 EntityId, const FString& FieldName, TArrayView<const uint8> Value);
	bool RecordPeerFieldHash(uint64 Tick, uint16 PeerId, uint64 EntityId, const FString& FieldName, uint64 Hash,
		TArrayView<const uint8> Value);

	uint64 PendingHashCount() const;

	/** Drains up to Max pending hashes, oldest first, as (tick, hash) pairs. */
	TArray<TPair<uint64, uint64>> TakePendingHashes(uint64 Max);

	/** Returns true and sets OutTick if a new cross-peer divergence was found. */
	bool CheckDivergence(uint64& OutTick);

	/** The combined hash over every tick this session has locally hashed. 0 if unconfigured. */
	uint64 Finish() const;

	bool FinishRecording();

	uint32 TickRateHz() const;
	uint32 PeerCount() const;

private:
	void SetLastErrorFromNative();

	FoldbackHandle* Handle = nullptr;
	FString LastError;
};
