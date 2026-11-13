// What the importer keeps of an export bundle's manifest.json: which bundle the generated
// content came from, and the game's tuning numbers by name.
#pragma once

#include "CoreMinimal.h"
#include "Engine/DataAsset.h"
#include "Engine/DataTable.h"
#include "AethyrManifest.generated.h"

UCLASS(BlueprintType)
class AETHYRBRIDGE_API UAethyrManifest : public UDataAsset
{
	GENERATED_BODY()

public:
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	FString Game;

	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	FString WorldSha256;

	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	int32 Meshes = 0;

	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	int32 Triangles = 0;

	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	TMap<FName, float> Tuning;

	// The kinds of moving thing, by the number the world lists them with (aethyr_things).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	TArray<FName> ThingKinds;

	// Each kind and variant as the world draws it at rest: "raider_v0" and so on.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	TMap<FName, TObjectPtr<class UStaticMesh>> ThingMeshes;

	// What aethyr_events reports, by number.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	TMap<int32, FName> Events;
};

// A row of the overrides table, DT_AethyrOverrides: the row's name says what is replaced
// ("thing.raider", "thing.raider_v1", "world.<mesh name>", "material.cel", "event.pickup") and
// Asset is what replaces it (a static mesh, a material, a sound). People own the table; the
// importer reads it and never writes it.
USTRUCT(BlueprintType)
struct AETHYRBRIDGE_API FAethyrOverrideRow : public FTableRowBase
{
	GENERATED_BODY()

	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	TSoftObjectPtr<UObject> Asset;
};
