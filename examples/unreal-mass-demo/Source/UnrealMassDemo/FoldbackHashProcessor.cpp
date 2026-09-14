// SPDX-License-Identifier: MIT OR Apache-2.0
#include "FoldbackHashProcessor.h"

#include "FoldbackDemoFragments.h"
#include "FoldbackSession.h"
#include "MassExecutionContext.h"

UFoldbackHashProcessor::UFoldbackHashProcessor()
{
	ExecutionFlags = static_cast<uint8>(EProcessorExecutionFlags::All);
	EntityQuery.RegisterWithProcessor(*this);
}

void UFoldbackHashProcessor::ConfigureQueries(const TSharedRef<FMassEntityManager>& EntityManager)
{
	EntityQuery.AddRequirement<FFoldbackDemoTransformFragment>(EMassFragmentAccess::ReadOnly);
}

void UFoldbackHashProcessor::Execute(FMassEntityManager& EntityManager, FMassExecutionContext& Context)
{
	if (!Session)
	{
		return;
	}

	EntityQuery.ForEachEntityChunk(Context, [this](FMassExecutionContext& ChunkContext) {
		const TConstArrayView<FFoldbackDemoTransformFragment> Transforms =
			ChunkContext.GetFragmentView<FFoldbackDemoTransformFragment>();

		for (int32 i = 0; i < ChunkContext.GetNumEntities(); ++i)
		{
			const FMassEntityHandle Entity = ChunkContext.GetEntity(i);
			const FFoldbackDemoTransformFragment& Transform = Transforms[i];

			TArray<uint8> State;
			State.Append(reinterpret_cast<const uint8*>(&Transform.PositionX), sizeof(Transform.PositionX));
			State.Append(reinterpret_cast<const uint8*>(&Transform.PositionY), sizeof(Transform.PositionY));

			Session->HashEntity(Tick, static_cast<uint64>(Entity.Index), State);
		}
	});
}
