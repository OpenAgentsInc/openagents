// The host of a Metaverse Engine world inside Unreal: what coax/web/host.js is in the browser. One actor
// owns the world's state, steps it each tick through aethyr_host.h, and shows what it writes:
// the live store as a procedural mesh, the camera, the look's numbers in a material parameter
// collection, and the overlay through AAethyrHUD. Keys, the pointer and the mouse go back in.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "GameFramework/GameModeBase.h"
#include "GameFramework/HUD.h"
#include "AethyrCamera.h"
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

	// The player camera (AethyrCamera.h): its tunables, with none set the class defaults, and the view shown.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Aethyr|Camera")
	TObjectPtr<UAethyrCameraTuning> CameraTuning;
	UPROPERTY(VisibleInstanceOnly, BlueprintReadOnly, Category = "Aethyr|Camera")
	EAethyrCameraMode CameraMode = EAethyrCameraMode::Behind;

	// The next view: first person, behind, front, and round. Kept for the next level and the next session.
	UFUNCTION(BlueprintCallable, Category = "Aethyr|Camera")
	void CycleCameraMode();
	UFUNCTION(BlueprintCallable, Category = "Aethyr|Camera")
	void SetCameraMode(EAethyrCameraMode Mode);
	// The aim trace of the view shown: from the eye in first person, from her head along the view in third.
	UFUNCTION(BlueprintCallable, Category = "Aethyr|Camera")
	bool AimTrace(FHitResult& Hit, float Distance = 3000.0f) const;
	// A camera shake for an impact (true) or a hit, at the tuning's strength times Scale.
	UFUNCTION(BlueprintCallable, Category = "Aethyr|Camera")
	void PlayCameraShake(bool bImpact, float Scale = 1.0f);
	// A message to the world, as the browser host sends one (the console's aethyr.Message KIND BODY).
	void SendWorldMessage(int32 Kind, const FString& Body);

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

	// The instanced draws (aethyr_instanced): a component for each instance of a mesh, kept by the
	// mesh (its store, first vertex and vertices) and moved to its row each frame. A component's
	// section is written again only when the mesh's store changed since it was written.
	UPROPERTY(Transient)
	TArray<TObjectPtr<UProceduralMeshComponent>> PiecePool;
	TArray<uint32> PieceBuilt;              // the store version each component's section was written from
	TMap<uint64, TArray<int32>> PieceByMesh;
	int32 PiecesShown = 0;

	// -AethyrWorld=URL: the page this host stands for; the world's requests go to its site
	FString WorldOrigin;
	int32 RequestsOut = 0;                  // the ids 0 to 7 with an answer still to come, a bit each
	int32 RequestsSent = 0;
	int32 RequestsAnswered = 0;

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
	void UpdatePieces();
	void UpdateRequests();
	void Section(UProceduralMeshComponent* Mesh, const uint32* Words, int32 Count) const;
	void UpdateCamera(float DeltaSeconds);
	void UpdateLook();

	// ---- the player camera (AethyrCamera.cpp)
	const UAethyrCameraTuning& Tuning() const;
	void LookInput(class APlayerController* Player, float DeltaSeconds);
	void ReadCameraSettings();
	void SendView(bool bOwned);
	bool bCameraReady = false;          // the view below has been set once
	bool bCameraOwned = false;          // the host places the view (she is on foot)
	bool bBodyHidden = false;
	bool bHavePlayer = false;
	bool bAirborne = false;
	bool bFalling = false;
	bool bViewSent = false;
	EAethyrCameraMode BlendedMode = EAethyrCameraMode::Behind;
	float LookPitch = 0.0f;             // degrees, up positive: the host's own (first person reaches ±89)
	float LookYawRate = 0.0f, LookPitchRate = 0.0f;   // smoothed, degrees a second
	FVector PlayerAt = FVector::ZeroVector;
	float PlayerSpeed = 0.0f, PlayerClimb = 0.0f;      // cm/s, smoothed
	float AirPeak = 0.0f, LandDip = 0.0f, LandAge = 10.0f;
	float BobPhase = 0.0f, BobWeight = 0.0f, SprintAlpha = 0.0f;
	FVector PivotNow = FVector::ZeroVector;
	FQuat TurnNow = FQuat::Identity;
	float ArmNow = 0.0f;
	FVector ViewAt = FVector::ZeroVector;            // the view as last shown
	FQuat ViewTurn = FQuat::Identity;
	float ViewFov = 90.0f;
	FVector BlendFromAt = FVector::ZeroVector;
	FQuat BlendFromTurn = FQuat::Identity;
	float BlendFromFov = 90.0f, BlendAge = 10.0f;
	float ShakeDegrees = 0.0f, ShakeAge = 10.0f;
	FVector AimFrom = FVector::ZeroVector, AimDir = FVector::ForwardVector;
	int32 SentView[14] = {};
	int32 SeenModeVar = -1;
	float MouseScale = 1.0f, StickScale = 1.0f;
	bool bInvertY = false, bBob = true;

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
