// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include "CoreMinimal.h"
#include "UObject/Interface.h"
#include "FoldbackReflection.generated.h"

class FFoldbackSession;

/**
 * Reflective (`UPROPERTY(meta=(FoldbackHash))`) field hashing — the
 * Unreal-specific per-field opt-in path from the design doc §5, letting
 * already-`UPROPERTY`-annotated gameplay structs get Level 3 hashing
 * without a hand-written `HashField` call per field per tick.
 *
 * IMPORTANT, found by checking the real macro rather than assuming: Unreal
 * `meta=(...)` tags are `UPROPERTY` **metadata**, and `FField::HasMetaData`
 * is compiled out whenever `WITH_METADATA` (== `WITH_EDITORONLY_DATA`) is 0
 * — i.e. in any Shipping/cooked-without-editor-data build
 * (Engine/Source/Runtime/Core/Public/Misc/CoreMiscDefines.h). So this walker
 * only ever sees the `FoldbackHash` tags in Editor/Development-Editor
 * builds — exactly the builds this project's own headless Automation-Test
 * verification pattern already runs in (`UnrealEditor-Cmd.exe`), but *not*
 * a packaged Shipping build. Outside `WITH_METADATA`, every entry point
 * below fails loudly (returns false, sets a clear error) instead of
 * silently hashing nothing — a shipping game that needs field-level
 * bisection should call `FFoldbackSession::HashField` explicitly instead;
 * this stays additive tooling on top of that API, never a replacement
 * (per foldback-reflective-hashing.md §1).
 */
class FOLDBACK_API FFoldbackReflectiveHasher
{
public:
	/** Default recursion limit for nested USTRUCTs, per the reflective-hashing doc §3 cycle-guard rule. */
	static constexpr int32 DefaultMaxDepth = 8;

	/**
	 * Walks every `UPROPERTY(meta=(FoldbackHash))`-tagged top-level field on
	 * Struct (cached per-`UStruct` after the first call) and records each as
	 * one `Session.HashField(Tick, EntityId, <field name>, <serialized
	 * bytes>)` call — reusing the existing Level 3 pipeline rather than
	 * inventing a second one. Nested USTRUCTs, TArray/TMap/TSet, and the
	 * common math types are serialized recursively into that one field's
	 * byte blob (see .cpp); they do not become separate HashField calls.
	 *
	 * Returns false (with Session.GetLastError() set) on: WITH_METADATA
	 * disabled, an unconfigured Session, a cycle, or exceeding MaxDepth.
	 * Unsupported property types are skipped with a logged warning (once
	 * per type+field) rather than failing the whole walk — consistent with
	 * "opt-in, and a clear error beats a silent gap" but "one field this
	 * binding doesn't yet know how to serialize" isn't fatal to the rest.
	 */
	static bool HashTaggedFields(FFoldbackSession& Session, uint64 Tick, uint64 EntityId, const UStruct* Struct,
		const void* ContainerPtr, int32 MaxDepth = DefaultMaxDepth);

	/** Convenience overload for hashing a UObject's own tagged fields by its UClass. */
	static bool HashTaggedFields(
		FFoldbackSession& Session, uint64 Tick, uint64 EntityId, const UObject* Object, int32 MaxDepth = DefaultMaxDepth);

	/** Returns the cached list of FoldbackHash-tagged top-level FProperty*s for Struct (builds it on first call). */
	static const TArray<FProperty*>& GetTaggedProperties(const UStruct* Struct);

	/**
	 * Serializes one property's value into canonical bytes (numeric raw
	 * bytes; FVector/FRotator/FQuat component-wise; TArray/TMap/TSet
	 * recursively, maps/sets sorted by key bytes per the shared
	 * unordered-container rule; nested UStructs recursively; UObject*
	 * hashed via IFoldbackIdentifiable if implemented, else a logged
	 * gap). Exposed publicly so the conformance test can exercise it
	 * directly without a full Session round-trip.
	 */
	static bool SerializeValue(FProperty* Property, const void* ValuePtr, TArray<uint8>& OutBytes, int32 Depth,
		int32 MaxDepth, TSet<const void*>& Visited, FString& OutError);
};

/**
 * Optional interface a `UObject` can implement so `UObject*` fields
 * reachable from a `FoldbackHash`-tagged field hash by a stable identity
 * instead of a raw pointer (never deterministic across peers/runs) — per
 * the design doc §5's "hash by a stable identity/index the game supplies"
 * requirement.
 */
UINTERFACE(MinimalAPI, BlueprintType)
class UFoldbackIdentifiable : public UInterface
{
	GENERATED_BODY()
};

class FOLDBACK_API IFoldbackIdentifiable
{
	GENERATED_BODY()

public:
	UFUNCTION(BlueprintNativeEvent, Category = "Foldback")
	int64 GetFoldbackStableId() const;
};
