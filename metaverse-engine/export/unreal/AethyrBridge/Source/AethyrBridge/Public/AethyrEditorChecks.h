// Editor-only calls for the importer (metaverse-engine/export/unreal/import.py) and the level
// extractor (metaverse-engine/import/unreal/export_level.py): Unreal's Python cannot read a mesh's
// colours, a material's compile errors, a convex hull's points or a skeleton's parents, or wait
// for shaders, so these do.
#pragma once

#include "CoreMinimal.h"
#include "Kismet/BlueprintFunctionLibrary.h"
#include "AethyrEditorChecks.generated.h"

class UStaticMesh;
class UMaterialInterface;
class USkeleton;

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

	// The simple collision's convex hulls, in the mesh's space (each hull's own transform applied),
	// in centimetres: Counts[i] points of hull i, one after another in Vertices. False: no body setup.
	UFUNCTION(BlueprintCallable, Category = "Aethyr")
	static bool ReadConvex(UStaticMesh* Mesh, TArray<FVector>& Vertices, TArray<int32>& Counts);

	// The skeleton's reference pose: one name, parent index (-1 for the root) and local rest
	// transform a bone, in the reference skeleton's order.
	UFUNCTION(BlueprintCallable, Category = "Aethyr")
	static bool ReadBones(USkeleton* Skeleton, TArray<FString>& Names, TArray<int32>& Parents, TArray<FTransform>& Rest);
};
