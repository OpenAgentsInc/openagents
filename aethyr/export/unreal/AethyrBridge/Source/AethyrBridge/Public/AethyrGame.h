// The host of an Aethyr world inside Unreal: what coax/web/host.js is in the browser. One actor
// owns the world's state, steps it each tick through aethyr_host.h, and shows what it writes:
// the live store as a procedural mesh, the camera, the look's numbers in a material parameter
// collection, and the overlay through AAethyrHUD. Keys, the pointer and the mouse go back in.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "GameFramework/GameModeBase.h"
#include "GameFramework/HUD.h"
#include "AethyrGame.generated.h"

class UProceduralMeshComponent;
class UCameraComponent;
class UMaterialInterface;
class UMaterialParameterCollection;
class UFontFace;

UCLASS()
class AETHYRBRIDGE_API AAethyrGame : public AActor
{
	GENERATED_BODY()

public:
	AAethyrGame();
	virtual void BeginPlay() override;
	virtual void EndPlay(const EEndPlayReason::Type Reason) override;
	virtual void Tick(float DeltaSeconds) override;

	UPROPERTY(VisibleAnywhere, Category = "Aethyr")
	TObjectPtr<UProceduralMeshComponent> Live;

	UPROPERTY(VisibleAnywhere, Category = "Aethyr")
	TObjectPtr<UCameraComponent> Camera;

	// The material the live mesh is drawn with (the world's meshes carry their own).
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<UMaterialInterface> LiveMaterial;

	// Each vector parameter named U<n> receives the uniform block's floats n to n + 3 every frame.
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<UMaterialParameterCollection> Look;

	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<UFontFace> TextFace;

	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<UFontFace> TitleFace;

	// Where the uniform block holds the camera and the overlay's panels (the manifest's "uniforms").
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	int32 EyeAt = 20;
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	int32 ForwardAt = 24;
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	int32 UpAt = 32;
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	int32 PanelsAt = 64;
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	int32 PanelCount = 12;

	// The overlay as the world last wrote it, for the HUD.
	const TArray<float>& Overlay() const { return Vertices; }
	int32 OverlayVertices() const { return OverlayCount; }
	int32 OverlayFloats() const { return VertexFloats; }
	const TArray<float>& UniformBlock() const { return Uniforms; }

private:
	void SendKeys(class APlayerController* Player);
	void UpdateLive();
	void UpdateCamera();
	void UpdateLook();

	void* State = nullptr;
	TArray<float> Vertices;
	TArray<float> Uniforms;
	int32 VertexFloats = 0;
	int32 OverlayCount = 0;
	uint32 LiveVersion = 0;
	bool bMouseHeld = false;
	bool bPointerDown = false;
	double LogAt = 0;
	double FrameSeconds = 0;
	double HostSeconds = 0;
	int32 FrameCount = 0;
	int32 ShotAfter = -1;
	FString ShotPath;
	bool bExitAfterShot = false;
};

UCLASS()
class AETHYRBRIDGE_API AAethyrHUD : public AHUD
{
	GENERATED_BODY()

public:
	virtual void DrawHUD() override;

private:
	TSharedPtr<const struct FCompositeFont> TextFont;
	TSharedPtr<const struct FCompositeFont> TitleFont;
};

UCLASS()
class AETHYRBRIDGE_API AAethyrGameMode : public AGameModeBase
{
	GENERATED_BODY()

public:
	AAethyrGameMode();
};
