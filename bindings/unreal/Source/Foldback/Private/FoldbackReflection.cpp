// SPDX-License-Identifier: MIT OR Apache-2.0
#include "FoldbackReflection.h"

#include "FoldbackSession.h"
#include "UObject/UnrealType.h"

DEFINE_LOG_CATEGORY_STATIC(LogFoldbackReflection, Log, All);

// IFoldbackIdentifiable::GetFoldbackStableId_Implementation's default body (`return 0`) is
// generated directly into the interface by UHT since no override is given here — implementing
// classes provide their own by overriding GetFoldbackStableId_Implementation in their own .cpp.

namespace
{
FCriticalSection GCacheLock;
TMap<const UStruct*, TArray<FProperty*>> GTaggedPropertyCache;

FCriticalSection GWarnOnceLock;
TSet<FString> GWarnedOnce;

void WarnOnce(const FString& Key, const FString& Message)
{
	FScopeLock Lock(&GWarnOnceLock);
	if (!GWarnedOnce.Contains(Key))
	{
		GWarnedOnce.Add(Key);
		UE_LOG(LogFoldbackReflection, Warning, TEXT("%s"), *Message);
	}
}

void AppendRaw(TArray<uint8>& Out, const void* Data, int32 Num)
{
	Out.Append(static_cast<const uint8*>(Data), Num);
}

void AppendU32(TArray<uint8>& Out, uint32 Value)
{
	AppendRaw(Out, &Value, sizeof(Value));
}

bool IsStruct(const FStructProperty* StructProp, FName StructName)
{
	return StructProp && StructProp->Struct && StructProp->Struct->GetFName() == StructName;
}
}	 // namespace

const TArray<FProperty*>& FFoldbackReflectiveHasher::GetTaggedProperties(const UStruct* Struct)
{
	static const TArray<FProperty*> Empty;
	if (!Struct)
	{
		return Empty;
	}

	FScopeLock Lock(&GCacheLock);
	if (const TArray<FProperty*>* Cached = GTaggedPropertyCache.Find(Struct))
	{
		return *Cached;
	}

	TArray<FProperty*> Tagged;
#if WITH_METADATA
	for (TFieldIterator<FProperty> It(Struct); It; ++It)
	{
		if (It->HasMetaData(TEXT("FoldbackHash")))
		{
			Tagged.Add(*It);
		}
	}
#endif	  // WITH_METADATA
	return GTaggedPropertyCache.Add(Struct, MoveTemp(Tagged));
}

bool FFoldbackReflectiveHasher::SerializeValue(FProperty* Property, const void* ValuePtr, TArray<uint8>& OutBytes,
	int32 Depth, int32 MaxDepth, TSet<const void*>& Visited, FString& OutError)
{
	if (Depth > MaxDepth)
	{
		OutError = FString::Printf(TEXT("FoldbackHash: max recursion depth (%d) exceeded at property '%s' — "
										 "likely a reference cycle without an IFoldbackIdentifiable-based cut"),
			MaxDepth, *Property->GetName());
		return false;
	}

	// --- numeric & bool: raw bytes, fixed-size, no platform-dependent gaps for the types we support ---
	if (const FNumericProperty* NumericProp = CastField<FNumericProperty>(Property))
	{
		AppendRaw(OutBytes, ValuePtr, NumericProp->GetElementSize());
		return true;
	}
	if (const FBoolProperty* BoolProp = CastField<FBoolProperty>(Property))
	{
		const uint8 B = BoolProp->GetPropertyValue(ValuePtr) ? 1 : 0;
		OutBytes.Add(B);
		return true;
	}
	if (const FEnumProperty* EnumProp = CastField<FEnumProperty>(Property))
	{
		// Discriminant only, in the underlying integer property's own fixed width — never the
		// host compiler's in-memory enum layout, per the reflective-hashing doc §3 enum rule.
		return SerializeValue(
			EnumProp->GetUnderlyingProperty(), ValuePtr, OutBytes, Depth, MaxDepth, Visited, OutError);
	}

	// --- FVector/FRotator/FQuat: special-cased per §5 rather than walked generically ---
	if (const FStructProperty* StructProp = CastField<FStructProperty>(Property))
	{
		if (IsStruct(StructProp, NAME_Vector))
		{
			const FVector& V = *reinterpret_cast<const FVector*>(ValuePtr);
			AppendRaw(OutBytes, &V.X, sizeof(V.X));
			AppendRaw(OutBytes, &V.Y, sizeof(V.Y));
			AppendRaw(OutBytes, &V.Z, sizeof(V.Z));
			return true;
		}
		if (IsStruct(StructProp, NAME_Rotator))
		{
			const FRotator& R = *reinterpret_cast<const FRotator*>(ValuePtr);
			AppendRaw(OutBytes, &R.Pitch, sizeof(R.Pitch));
			AppendRaw(OutBytes, &R.Yaw, sizeof(R.Yaw));
			AppendRaw(OutBytes, &R.Roll, sizeof(R.Roll));
			return true;
		}
		if (StructProp->Struct && StructProp->Struct->GetFName() == NAME_Quat)
		{
			const FQuat& Q = *reinterpret_cast<const FQuat*>(ValuePtr);
			AppendRaw(OutBytes, &Q.X, sizeof(Q.X));
			AppendRaw(OutBytes, &Q.Y, sizeof(Q.Y));
			AppendRaw(OutBytes, &Q.Z, sizeof(Q.Z));
			AppendRaw(OutBytes, &Q.W, sizeof(Q.W));
			return true;
		}

		// Generic nested USTRUCT: recurse over ALL its fields (the containing field's own
		// FoldbackHash tag already opted the whole nested struct in — no need to double-tag each
		// of its members), guarded against cycles by pointer identity.
		if (Visited.Contains(ValuePtr))
		{
			OutError = FString::Printf(
				TEXT("FoldbackHash: cycle detected walking nested struct at property '%s'"), *Property->GetName());
			return false;
		}
		Visited.Add(ValuePtr);
		bool bOk = true;
		if (StructProp->Struct)
		{
			for (TFieldIterator<FProperty> It(StructProp->Struct); It && bOk; ++It)
			{
				const void* MemberPtr = It->ContainerPtrToValuePtr<void>(ValuePtr);
				bOk = SerializeValue(*It, MemberPtr, OutBytes, Depth + 1, MaxDepth, Visited, OutError);
			}
		}
		Visited.Remove(ValuePtr);
		return bOk;
	}

	// --- TArray: order is game-controlled and deterministic already, so serialized in place ---
	if (const FArrayProperty* ArrayProp = CastField<FArrayProperty>(Property))
	{
		FScriptArrayHelper Helper(ArrayProp, ValuePtr);
		AppendU32(OutBytes, static_cast<uint32>(Helper.Num()));
		for (int32 i = 0; i < Helper.Num(); ++i)
		{
			if (!SerializeValue(
					ArrayProp->Inner, Helper.GetRawPtr(i), OutBytes, Depth + 1, MaxDepth, Visited, OutError))
			{
				return false;
			}
		}
		return true;
	}

	// --- TMap / TSet: unordered by construction — sort by key bytes before hashing, unconditionally ---
	if (const FMapProperty* MapProp = CastField<FMapProperty>(Property))
	{
		FScriptMapHelper Helper(MapProp, ValuePtr);
		TArray<TPair<TArray<uint8>, TArray<uint8>>> Entries;
		for (FScriptMapHelper::FIterator It(Helper); It; ++It)
		{
			TArray<uint8> KeyBytes, ValBytes;
			if (!SerializeValue(MapProp->KeyProp, Helper.GetKeyPtr(It), KeyBytes, Depth + 1, MaxDepth, Visited,
					OutError) ||
				!SerializeValue(MapProp->ValueProp, Helper.GetValuePtr(It), ValBytes, Depth + 1, MaxDepth, Visited,
					OutError))
			{
				return false;
			}
			Entries.Emplace(MoveTemp(KeyBytes), MoveTemp(ValBytes));
		}
		Entries.Sort([](const auto& A, const auto& B) {
			return FMemory::Memcmp(A.Key.GetData(), B.Key.GetData(), FMath::Min(A.Key.Num(), B.Key.Num())) < 0 ||
				   (A.Key.Num() < B.Key.Num());
		});
		AppendU32(OutBytes, static_cast<uint32>(Entries.Num()));
		for (const auto& Entry : Entries)
		{
			OutBytes.Append(Entry.Key);
			OutBytes.Append(Entry.Value);
		}
		return true;
	}
	if (const FSetProperty* SetProp = CastField<FSetProperty>(Property))
	{
		FScriptSetHelper Helper(SetProp, ValuePtr);
		TArray<TArray<uint8>> Entries;
		for (FScriptSetHelper::FIterator It(Helper); It; ++It)
		{
			TArray<uint8> ElemBytes;
			if (!SerializeValue(SetProp->ElementProp, Helper.GetElementPtr(It), ElemBytes, Depth + 1, MaxDepth,
					Visited, OutError))
			{
				return false;
			}
			Entries.Add(MoveTemp(ElemBytes));
		}
		Entries.Sort([](const TArray<uint8>& A, const TArray<uint8>& B) {
			return FMemory::Memcmp(A.GetData(), B.GetData(), FMath::Min(A.Num(), B.Num())) < 0 || (A.Num() < B.Num());
		});
		AppendU32(OutBytes, static_cast<uint32>(Entries.Num()));
		for (const auto& Elem : Entries)
		{
			OutBytes.Append(Elem);
		}
		return true;
	}

	// --- UObject*: never hash the raw pointer (not deterministic across peers/runs); use a
	// stable ID the game supplies via IFoldbackIdentifiable, else disclose the gap and skip.
	if (const FObjectProperty* ObjProp = CastField<FObjectProperty>(Property))
	{
		UObject* Obj = ObjProp->GetObjectPropertyValue(ValuePtr);
		if (!Obj)
		{
			AppendU32(OutBytes, 0);	   // deterministic "null" marker, distinct from a real 0 id below
			return true;
		}
		if (Obj->Implements<UFoldbackIdentifiable>())
		{
			const int64 StableId = IFoldbackIdentifiable::Execute_GetFoldbackStableId(Obj);
			OutBytes.Add(1);
			AppendRaw(OutBytes, &StableId, sizeof(StableId));
			return true;
		}
		WarnOnce(FString::Printf(TEXT("noid:%s"), *Property->GetName()),
			FString::Printf(TEXT("FoldbackHash: field '%s' references a UObject (class %s) that doesn't implement "
								  "IFoldbackIdentifiable — hashed as untracked (constant), not by identity. "
								  "Determinism bugs involving this reference won't be caught."),
				*Property->GetName(), *Obj->GetClass()->GetName()));
		OutBytes.Add(2);
		return true;
	}

	WarnOnce(FString::Printf(TEXT("unsupported:%s"), *Property->GetCPPType()),
		FString::Printf(TEXT("FoldbackHash: property '%s' has unsupported type '%s' for reflective hashing — "
							  "skipped (not included in the field's hash). Use FFoldbackSession::HashField "
							  "directly for this field if it matters for determinism."),
			*Property->GetName(), *Property->GetCPPType()));
	return true;
}

bool FFoldbackReflectiveHasher::HashTaggedFields(
	FFoldbackSession& Session, uint64 Tick, uint64 EntityId, const UStruct* Struct, const void* ContainerPtr, int32 MaxDepth)
{
#if !WITH_METADATA
	UE_LOG(LogFoldbackReflection, Error,
		TEXT("FoldbackHash: reflective hashing requires WITH_METADATA (Editor/Development-Editor builds only) — "
			 "UPROPERTY meta tags don't exist in this build config. Use FFoldbackSession::HashField explicitly "
			 "for Shipping/cooked-without-editor-data builds."));
	return false;
#else
	if (!Struct || !ContainerPtr)
	{
		return false;
	}
	const TArray<FProperty*>& Tagged = GetTaggedProperties(Struct);
	bool bAllOk = true;
	for (FProperty* Property : Tagged)
	{
		const void* ValuePtr = Property->ContainerPtrToValuePtr<void>(ContainerPtr);
		TArray<uint8> Bytes;
		TSet<const void*> Visited;
		FString Error;
		if (!SerializeValue(Property, ValuePtr, Bytes, 0, MaxDepth, Visited, Error))
		{
			UE_LOG(LogFoldbackReflection, Error, TEXT("%s"), *Error);
			bAllOk = false;
			continue;
		}
		if (!Session.HashField(Tick, EntityId, Property->GetName(), Bytes))
		{
			bAllOk = false;
		}
	}
	return bAllOk;
#endif	  // WITH_METADATA
}

bool FFoldbackReflectiveHasher::HashTaggedFields(
	FFoldbackSession& Session, uint64 Tick, uint64 EntityId, const UObject* Object, int32 MaxDepth)
{
	if (!Object)
	{
		return false;
	}
	return HashTaggedFields(Session, Tick, EntityId, Object->GetClass(), Object, MaxDepth);
}
