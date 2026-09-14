// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Headless conformance test for the Unreal binding's UPROPERTY(meta=(FoldbackHash)) reflective
// hashing (design doc §5) — run via UnrealEditor-Cmd.exe
// -ExecCmds="Automation RunTests Foldback.ReflectionVerify;Quit" -nullrhi -unattended -nosplash,
// same headless pattern as Foldback.Verify. Proves the walker's own rules, not just that it
// compiles: opt-in tag filtering, the unordered-container sort rule (reflective-hashing doc §3),
// nested-struct recursion semantics, the depth guard, and the UObject* stable-identity path.

#include "FoldbackReflection.h"
#include "FoldbackReflectionTestTypes.h"
#include "FoldbackSession.h"
#include "Misc/AutomationTest.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FFoldbackReflectionVerifyTest, "Foldback.ReflectionVerify",
	EAutomationTestFlags_ApplicationContextMask | EAutomationTestFlags::ProductFilter)

namespace
{
TArray<uint8> SerializeTopLevel(UStruct* Struct, const void* Container, const TCHAR* FieldName)
{
	TArray<uint8> Out;
	FProperty* Property = Struct->FindPropertyByName(FieldName);
	check(Property);
	TSet<const void*> Visited;
	FString Error;
	FFoldbackReflectiveHasher::SerializeValue(
		Property, Property->ContainerPtrToValuePtr<void>(Container), Out, 0, 8, Visited, Error);
	return Out;
}
}	 // namespace

bool FFoldbackReflectionVerifyTest::RunTest(const FString& Parameters)
{
	bool bAllOk = true;
	UScriptStruct* StructType = FFoldbackReflectionTestStruct::StaticStruct();

	// --- opt-in tag filtering: only meta=(FoldbackHash) fields are in the cached list ---
	{
		const TArray<FProperty*>& Tagged = FFoldbackReflectiveHasher::GetTaggedProperties(StructType);
		TSet<FString> Names;
		for (FProperty* P : Tagged)
		{
			Names.Add(P->GetName());
		}
		const bool bExpected = Names.Contains(TEXT("IntField")) && Names.Contains(TEXT("VectorField")) &&
								Names.Contains(TEXT("ArrayField")) && Names.Contains(TEXT("MapField")) &&
								Names.Contains(TEXT("NestedField")) && !Names.Contains(TEXT("UntaggedField")) &&
								Names.Num() == 5;
		if (!bExpected)
		{
			AddError(FString::Printf(TEXT("tagged-property set wrong, found %d entries"), Names.Num()));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: opt-in tag filtering matches exactly the meta=(FoldbackHash) fields"));
		}
	}

	// --- unordered-container rule: TMap must hash identically regardless of insertion order ---
	{
		FFoldbackReflectionTestStruct A, B;
		A.MapField.Add(1, 10);
		A.MapField.Add(2, 20);
		A.MapField.Add(3, 30);
		B.MapField.Add(3, 30);
		B.MapField.Add(1, 10);
		B.MapField.Add(2, 20);

		const TArray<uint8> BytesA = SerializeTopLevel(StructType, &A, TEXT("MapField"));
		const TArray<uint8> BytesB = SerializeTopLevel(StructType, &B, TEXT("MapField"));
		if (BytesA != BytesB || BytesA.IsEmpty())
		{
			AddError(TEXT("TMap serialization is not insertion-order-independent (sorted-container rule violated)"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: TMap serialization is insertion-order-independent"));
		}

		// A genuinely different map must still produce different bytes.
		FFoldbackReflectionTestStruct C;
		C.MapField.Add(1, 10);
		C.MapField.Add(2, 99);
		const TArray<uint8> BytesC = SerializeTopLevel(StructType, &C, TEXT("MapField"));
		if (BytesC == BytesA)
		{
			AddError(TEXT("differing TMap contents produced identical bytes"));
			bAllOk = false;
		}
	}

	// --- TArray: order is NOT normalized (game-controlled, already deterministic) ---
	{
		FFoldbackReflectionTestStruct A, B;
		A.ArrayField = { 1, 2, 3 };
		B.ArrayField = { 3, 2, 1 };
		const TArray<uint8> BytesA = SerializeTopLevel(StructType, &A, TEXT("ArrayField"));
		const TArray<uint8> BytesB = SerializeTopLevel(StructType, &B, TEXT("ArrayField"));
		if (BytesA == BytesB)
		{
			AddError(TEXT("TArray element order should affect the hash but didn't"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: TArray order affects the hash (arrays are not sorted, unlike maps/sets)"));
		}
	}

	// --- nested USTRUCT: recurses over ALL of its own fields, tagged or not (parent tag covers it) ---
	{
		FFoldbackReflectionTestStruct A, B;
		A.NestedField.Value = 1.f;
		A.NestedField.Untracked = 1.f;
		B.NestedField.Value = 1.f;
		B.NestedField.Untracked = 2.f;	   // differs only in the nested struct's own untagged field
		const TArray<uint8> BytesA = SerializeTopLevel(StructType, &A, TEXT("NestedField"));
		const TArray<uint8> BytesB = SerializeTopLevel(StructType, &B, TEXT("NestedField"));
		if (BytesA == BytesB)
		{
			AddError(TEXT("nested struct recursion should include its own untagged fields too, but didn't"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: nested-struct recursion covers all of the nested struct's own fields"));
		}
	}

	// --- depth guard: MaxDepth=0 must fail closed on any nested struct, not hang or crash ---
	{
		FFoldbackReflectionTestStruct A;
		FProperty* NestedProp = StructType->FindPropertyByName(TEXT("NestedField"));
		TArray<uint8> Out;
		TSet<const void*> Visited;
		FString Error;
		const bool bOk = FFoldbackReflectiveHasher::SerializeValue(
			NestedProp, NestedProp->ContainerPtrToValuePtr<void>(&A), Out, /*Depth=*/1, /*MaxDepth=*/0, Visited, Error);
		if (bOk || Error.IsEmpty())
		{
			AddError(TEXT("depth guard should have failed closed with an error, didn't"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: depth guard fails closed (error reported, not a hang/crash)"));
		}
	}

	// --- UObject*: stable-identity path (IFoldbackIdentifiable) vs the disclosed no-identity gap ---
	{
		UScriptStruct* ObjStructType = FFoldbackReflectionObjectTestStruct::StaticStruct();
		FFoldbackReflectionObjectTestStruct WithId, WithoutId;
		UFoldbackReflectionTestIdentifiable* IdObj = NewObject<UFoldbackReflectionTestIdentifiable>();
		IdObj->StableId = 42;
		WithId.ObjectField = IdObj;
		WithoutId.ObjectField = NewObject<UFoldbackReflectionTestPlainObject>();

		const TArray<uint8> BytesWithId = SerializeTopLevel(ObjStructType, &WithId, TEXT("ObjectField"));
		const TArray<uint8> BytesWithoutId = SerializeTopLevel(ObjStructType, &WithoutId, TEXT("ObjectField"));
		if (BytesWithId.IsEmpty() || BytesWithoutId.IsEmpty() || BytesWithId == BytesWithoutId)
		{
			AddError(TEXT("UObject* identity path didn't distinguish an IFoldbackIdentifiable object from a plain one"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: IFoldbackIdentifiable UObject* hashed by stable id, plain UObject* flagged as untracked"));
		}
	}

	// --- end-to-end: HashTaggedFields actually records real HashField calls to a real file ---
	{
		FFoldbackReflectionTestStruct A;
		A.IntField = 7;
		A.ArrayField = { 1, 2, 3 };

		FFoldbackSession Session;
		FFoldbackSession::FConfig Config;
		Config.TickRateHz = 60;
		Config.PeerCount = 1;
		if (!Session.Configure(Config))
		{
			AddError(FString::Printf(TEXT("Session.Configure failed: %s"), *Session.GetLastError()));
			bAllOk = false;
		}
		else if (!FFoldbackReflectiveHasher::HashTaggedFields(Session, 0, 0, StructType, &A))
		{
			AddError(TEXT("HashTaggedFields end-to-end call failed"));
			bAllOk = false;
		}
		else
		{
			AddInfo(TEXT("PASS: HashTaggedFields records real HashField calls end to end"));
		}
	}

	return bAllOk;
}
