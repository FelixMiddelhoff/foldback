// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include "CoreMinimal.h"
#include "FoldbackReflection.h"
#include "FoldbackReflectionTestTypes.generated.h"

USTRUCT()
struct FFoldbackReflectionInner
{
	GENERATED_BODY()

	UPROPERTY(meta = (FoldbackHash))
	float Value = 0.f;

	// Deliberately untagged at this level — but nested structs are recursed into wholesale (design
	// doc §5: "no need to double-tag each of its members"), so this still affects the parent
	// field's hash once NestedField is tagged. The conformance test in
	// FoldbackReflectionVerifyTest.cpp exercises exactly this distinction.
	UPROPERTY()
	float Untracked = 0.f;
};

USTRUCT()
struct FFoldbackReflectionTestStruct
{
	GENERATED_BODY()

	UPROPERTY(meta = (FoldbackHash))
	int32 IntField = 0;

	UPROPERTY()
	int32 UntaggedField = 0;

	UPROPERTY(meta = (FoldbackHash))
	FVector VectorField = FVector::ZeroVector;

	UPROPERTY(meta = (FoldbackHash))
	TArray<int32> ArrayField;

	UPROPERTY(meta = (FoldbackHash))
	TMap<int32, int32> MapField;

	UPROPERTY(meta = (FoldbackHash))
	FFoldbackReflectionInner NestedField;
};

UCLASS()
class UFoldbackReflectionTestPlainObject : public UObject
{
	GENERATED_BODY()
};

UCLASS()
class UFoldbackReflectionTestIdentifiable : public UObject, public IFoldbackIdentifiable
{
	GENERATED_BODY()

public:
	int64 StableId = 0;

	virtual int64 GetFoldbackStableId_Implementation() const override { return StableId; }
};

USTRUCT()
struct FFoldbackReflectionObjectTestStruct
{
	GENERATED_BODY()

	UPROPERTY(meta = (FoldbackHash))
	TObjectPtr<UObject> ObjectField = nullptr;
};
