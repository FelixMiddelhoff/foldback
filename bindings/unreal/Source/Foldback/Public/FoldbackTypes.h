// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include "CoreMinimal.h"
#include "FoldbackTypes.generated.h"

/**
 * Blueprint-facing config for `UFoldbackSubsystem::Configure` — mirrors
 * `FFoldbackSession::FConfig`. `int64` hashes/ticks throughout this
 * module's Blueprint surface cross from the underlying `uint64` via exact
 * bit-reinterpretation (Blueprint has no unsigned 64-bit integer type,
 * same reasoning as the Godot binding's `i64` hashes) — a hash may
 * display as negative, which is expected and harmless for the
 * equality-based divergence checks this is used for.
 */
USTRUCT(BlueprintType)
struct FOLDBACK_API FFoldbackConfigBP
{
	GENERATED_BODY()

	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Foldback")
	int32 TickRateHz = 60;

	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Foldback")
	int32 PeerCount = 1;

	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Foldback")
	int32 LocalPeerId = 0;

	/** Ticks of ring-buffer retention. 0 = library default. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Foldback")
	int32 Retention = 0;

	/** Empty = don't record to a file. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Foldback")
	FString RecordToPath;
};

USTRUCT(BlueprintType)
struct FOLDBACK_API FFoldbackPendingHashBP
{
	GENERATED_BODY()

	UPROPERTY(BlueprintReadOnly, Category = "Foldback")
	int64 Tick = 0;

	UPROPERTY(BlueprintReadOnly, Category = "Foldback")
	int64 Hash = 0;
};
