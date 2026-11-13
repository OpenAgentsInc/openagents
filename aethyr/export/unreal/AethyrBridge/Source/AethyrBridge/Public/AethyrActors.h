// The two actors the importer places from a manifest: a blocking box for each of the rules'
// collision boxes, and a named marker for each placement.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "AethyrActors.generated.h"

class UBoxComponent;

UCLASS()
class AETHYRBRIDGE_API AAethyrCollisionBox : public AActor
{
	GENERATED_BODY()

public:
	AAethyrCollisionBox();

	UPROPERTY(VisibleAnywhere, BlueprintReadOnly, Category = "Aethyr")
	TObjectPtr<UBoxComponent> Box;

	// The height the rules gave the box, in metres. 0: the world draws it another way.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	float RulesHeight = 0.0f;
};

UCLASS()
class AETHYRBRIDGE_API AAethyrMarker : public AActor
{
	GENERATED_BODY()

public:
	AAethyrMarker();

	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	FName Kind;

	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr")
	FName MarkerName;
};
