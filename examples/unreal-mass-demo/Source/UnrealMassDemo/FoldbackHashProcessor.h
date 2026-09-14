// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include "CoreMinimal.h"
#include "MassEntityQuery.h"
#include "MassProcessor.h"
#include "FoldbackHashProcessor.generated.h"

class FFoldbackSession;

/**
 * The Mass Entity integration point described in the Unreal binding design
 * doc §6: iterates entities via Mass's own fragment queries (as opposed to
 * examples/unreal-demo's plain `UTickFunction` hook) and hashes each one's
 * relevant fragment(s) as Level 2 data. A `FMassProcessor` is the natural
 * place for this in a Mass-based project — it runs alongside a project's
 * own simulation processors in the same processing phase, iterating the
 * exact same archetype chunks.
 *
 * Kept deliberately simple for this demo: `Execute` is invoked directly by
 * the Automation Test below via `EntityQuery.ForEachEntityChunk`, rather
 * than wiring a full `FMassProcessingPhaseManager` — proving the
 * query/fragment access pattern a real game would use, without needing a
 * running `UWorld`/game instance the way a full phase graph would.
 */
UCLASS()
class UFoldbackHashProcessor : public UMassProcessor
{
	GENERATED_BODY()

public:
	UFoldbackHashProcessor();

	/** Session + tick to hash into for the next Execute() call — set by the test harness per tick. */
	void SetHashTarget(FFoldbackSession* InSession, uint64 InTick) { Session = InSession; Tick = InTick; }

protected:
	virtual void ConfigureQueries(const TSharedRef<FMassEntityManager>& EntityManager) override;
	virtual void Execute(FMassEntityManager& EntityManager, FMassExecutionContext& Context) override;

private:
	FMassEntityQuery EntityQuery;
	FFoldbackSession* Session = nullptr;
	uint64 Tick = 0;
};
