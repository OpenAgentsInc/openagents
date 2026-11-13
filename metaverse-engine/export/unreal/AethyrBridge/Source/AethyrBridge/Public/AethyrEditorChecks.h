// Editor-only calls for the importer (metaverse-engine/export/unreal/import.py): Unreal's Python cannot
// read a mesh's colours, a material's compile errors, or wait for shaders, so these do.
#pragma once

#include "CoreMinimal.h"
#include "Kismet/BlueprintFunctionLibrary.h"
#include "AethyrEditorChecks.generated.h"

class UStaticMesh;
class UMaterialInterface;

UCLASS()
class AETHYRBRIDGE_API UAethyrEditorChecks : public UBlueprintFunctionLibrary
{
	GENERATED_BODY()

public:
	// LOD 0 as rendered: one position and colour a vertex, three indices a triangle.
	UFUNCTION(BlueprintCallable, Category = "Aethyr")
	static bool ReadMesh(UStaticMesh* Mesh, TArray<FVector>& Positions, TArray<FColor>& Colors, TArray<int32>& Indices);

	// Waits for every shader and asset still compiling.
	UFUNCTION(BlueprintCallable, Category = "Aethyr")
	static void FinishCompiling();

	// The material's compile errors for the running shader platform. Empty: it compiled.
	UFUNCTION(BlueprintCallable, Category = "Aethyr")
	static TArray<FString> MaterialErrors(UMaterialInterface* Material);
};
