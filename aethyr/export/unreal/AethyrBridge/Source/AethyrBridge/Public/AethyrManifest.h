// What the importer keeps of an export bundle's manifest.json: which bundle the generated
// content came from, and the game's tuning numbers by name.
#pragma once

#include "CoreMinimal.h"
#include "Engine/DataAsset.h"
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
};
