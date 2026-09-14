// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include "CoreMinimal.h"
#include "FoldbackSession.h"
#include "FoldbackTypes.h"
#include "Subsystems/GameInstanceSubsystem.h"
#include "FoldbackSubsystem.generated.h"

/**
 * `UGameInstanceSubsystem` wrapping `FFoldbackSession` for Blueprint —
 * one subsystem instance per game instance, matching the Rust
 * `Session`/`SessionBuilder` cookbook API but as Unreal object lifetime
 * (`Configure`/`Deinitialize` instead of `Session::builder()`/drop, the
 * same two-step "construct, then configure" shape the Unity and Godot
 * bindings both converged on independently for their own engine-specific
 * reasons — here it's simply that a `GameInstanceSubsystem` is
 * default-constructed by the engine before any game code runs).
 */
UCLASS()
class FOLDBACK_API UFoldbackSubsystem : public UGameInstanceSubsystem
{
	GENERATED_BODY()

public:
	virtual void Deinitialize() override;

	/** Builds the underlying session. Returns false and sets GetLastError() on failure. */
	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool Configure(const FFoldbackConfigBP& Config);

	UFUNCTION(BlueprintPure, Category = "Foldback")
	bool IsConfigured() const { return Session.IsConfigured(); }

	UFUNCTION(BlueprintPure, Category = "Foldback")
	FString GetLastError() const { return Session.GetLastError(); }

	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool HashTick(int64 Tick, const TArray<uint8>& State);

	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool RecordPeerHash(int64 Tick, int32 PeerId, int64 Hash);

	/** Level 2. Recording-only — see FFoldbackSession::HashEntity. */
	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool HashEntity(int64 Tick, int64 EntityId, const TArray<uint8>& State);

	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool RecordPeerEntityHash(int64 Tick, int32 PeerId, int64 EntityId, int64 Hash);

	/** Level 3. Recording-only, same caveat as HashEntity. */
	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool HashField(int64 Tick, int64 EntityId, const FString& FieldName, const TArray<uint8>& Value);

	/**
	 * Reflective Level 3: hashes every `UPROPERTY(meta=(FoldbackHash))`-tagged
	 * field on Object's class, one HashField call per tagged field — see
	 * FFoldbackReflectiveHasher. Editor/Development-Editor builds only
	 * (WITH_METADATA); returns false with GetLastError() unset but a logged
	 * error otherwise (see FoldbackReflection.h for why).
	 */
	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool HashTaggedFields(int64 Tick, int64 EntityId, UObject* Object);

	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool RecordPeerFieldHash(
		int64 Tick, int32 PeerId, int64 EntityId, const FString& FieldName, int64 Hash, const TArray<uint8>& Value);

	UFUNCTION(BlueprintPure, Category = "Foldback")
	int64 PendingHashCount() const;

	UFUNCTION(BlueprintCallable, Category = "Foldback")
	TArray<FFoldbackPendingHashBP> TakePendingHashes(int64 Max);

	/** Returns true and sets OutTick if a new cross-peer divergence was found. */
	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool CheckDivergence(int64& OutTick);

	/** The combined hash over every tick this session has locally hashed. 0 if unconfigured. */
	UFUNCTION(BlueprintPure, Category = "Foldback")
	int64 Finish() const;

	UFUNCTION(BlueprintCallable, Category = "Foldback")
	bool FinishRecording();

	/** Direct C++ access to the underlying session — used by this plugin's own tests. */
	FFoldbackSession& GetSession() { return Session; }

private:
	FFoldbackSession Session;
};
