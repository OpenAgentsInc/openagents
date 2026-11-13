// The host of a Metaverse Engine world inside Unreal: what coax/web/host.js is in the browser. One actor
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

	// The sky: a great sphere seen from inside, kept round the eye. The importer gives it its
	// mesh and material.
	UPROPERTY(VisibleAnywhere, BlueprintReadOnly, Category = "Aethyr")
	TObjectPtr<class UStaticMeshComponent> Sky;

	// The material the live mesh is drawn with (the world's meshes carry their own).
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<UMaterialInterface> LiveMaterial;

	// Each vector parameter named U<n> receives the uniform block's floats n to n + 3 every frame.
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<UMaterialParameterCollection> Look;

	// The bundle's manifest: the kinds of thing, their meshes, the events' names.
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<class UAethyrManifest> Manifest;

	// What people replaced: a model for a kind of thing, a sound for an event.
	UPROPERTY(EditAnywhere, Category = "Aethyr")
	TObjectPtr<class UDataTable> Overrides;

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
	int32 FxAt = 52;
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
	void SetUpThings();
	void UpdateThings();
	void PlayEvents();
	bool StepPlay();
	UObject* OverrideFor(const FString& Row) const;

	// one model for each (kind, variant) a host draws; empty for kinds the world draws
	TMap<int32, TObjectPtr<class UStaticMesh>> NativeMeshes;
	TSet<int32> NativeKinds;

	UPROPERTY(Transient)
	TArray<TObjectPtr<class UStaticMeshComponent>> ThingPool;

	// -AethyrPlay: the exported script, stepped on this actor's own world
	TArray<FString> PlayLines;
	int32 PlayLine = 0;
	int32 PlayFrames = 0;
	float PlayDt = 0.02f;
	int32 PlaySpeed = 1;
	bool bPlaying = false;
	bool bPlayFailed = false;
	bool bExitAfterPlay = false;

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
	bool bInput = true;
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
	UPROPERTY(Transient)
	TObjectPtr<class UFont> TextFont;

	UPROPERTY(Transient)
	TObjectPtr<class UFont> TitleFont;
};

UCLASS()
class AETHYRBRIDGE_API AAethyrGameMode : public AGameModeBase
{
	GENERATED_BODY()

public:
	AAethyrGameMode();
};
