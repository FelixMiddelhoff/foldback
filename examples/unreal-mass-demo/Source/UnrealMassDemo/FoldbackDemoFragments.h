// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include "CoreMinimal.h"
#include "MassEntityTypes.h"
#include "FoldbackDemoFragments.generated.h"

/** Toy per-entity simulation state — the "natural Level-2 hook" the Unreal binding design doc §6 describes. */
USTRUCT()
struct FFoldbackDemoTransformFragment : public FMassFragment
{
	GENERATED_BODY()

	UPROPERTY()
	float PositionX = 0.f;

	UPROPERTY()
	float PositionY = 0.f;
};
