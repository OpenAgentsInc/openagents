// The Unreal host's own player camera: three views cycled with one key (F5 or V; the right stick's click or
// the top face button on a gamepad), as Minecraft's F5 cycles them. The world keeps what it owns: where she
// stands, which way she faces and walks, its collision and its run. The host places the view while she is on
// foot and gives the world the view it shows and the ray it aims along (message 622), so the bars over the bots
// and Digger's pointing match the picture. Everywhere else (the title, the edit bay, the drive, the memory) the
// world's own camera is shown, blended to and from.
#pragma once

#include "CoreMinimal.h"
#include "Engine/DataAsset.h"
#include "AethyrCamera.generated.h"

class UCameraShakeBase;

UENUM(BlueprintType)
enum class EAethyrCameraMode : uint8
{
	FirstPerson UMETA(DisplayName = "First person"),
	Behind UMETA(DisplayName = "Third person, behind"),
	Front UMETA(DisplayName = "Third person, front"),
};

// Every number the camera is tuned by, in one place. Make a data asset of this class and set it on the level's
// AethyrGame actor (Camera Tuning) to tune without code; with none set, these defaults are used. Lengths are
// centimetres, angles degrees, speeds per second.
UCLASS(BlueprintType)
class AETHYRBRIDGE_API UAethyrCameraTuning : public UDataAsset
{
	GENERATED_BODY()

public:
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Modes")
	EAethyrCameraMode DefaultMode = EAethyrCameraMode::Behind;
	// Seconds a change of view (or to and from the world's own camera) blends over.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Modes", meta = (ClampMin = "0"))
	float BlendTime = 0.25f;

	// Degrees turned per unit of Unreal's mouse delta (1.47: the browser host's own feel). The player's
	// multiplier is the console variable aethyr.cam.MouseSensitivity.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Look", meta = (ClampMin = "0"))
	float MouseDegreesPerUnit = 1.4667f;
	// Degrees a second at full stick (aethyr.cam.StickSensitivity multiplies them).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Look", meta = (ClampMin = "0"))
	float StickYawSpeed = 200.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Look", meta = (ClampMin = "0"))
	float StickPitchSpeed = 140.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Look", meta = (ClampMin = "0", ClampMax = "0.9"))
	float StickDeadZone = 0.18f;
	// Seconds the look's turn rate is smoothed over (0: raw).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Look", meta = (ClampMin = "0"))
	float LookSmoothing = 0.03f;

	// Horizontal field of view in first person.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person")
	float FirstPersonFov = 90.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person")
	float EyeHeight = 155.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0", ClampMax = "89.9"))
	float FirstPersonPitchLimit = 89.0f;
	// The eye keeps this far from a wall (well past the near clip plane), pulled back toward her middle.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "1"))
	float FirstPersonProbeRadius = 16.0f;
	// The head bob (aethyr.cam.HeadBob 0 turns it off): up and down twice a stride cycle, side to side once.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person")
	bool bHeadBob = true;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0"))
	float BobVertical = 1.6f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0"))
	float BobSide = 0.9f;
	// Metres walked for one full cycle (two steps).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0.1"))
	float BobCycleMetres = 2.4f;
	// Ground speed (cm/s) at which the bob is full; it fades to nothing standing still.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "1"))
	float BobFullSpeed = 560.0f;
	// The landing dip: centimetres per metre fallen, at most LandDipMax, from falls of LandMinFall or more.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0"))
	float LandDipPerMetre = 6.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0"))
	float LandDipMax = 12.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0"))
	float LandMinFall = 40.0f;
	// How fast the dip comes back up (per second).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "First person", meta = (ClampMin = "0.1"))
	float LandRecover = 8.0f;

	// The sprint widening, in every view: from SprintSpeedStart (cm/s; walking is 560) to full at SprintSpeedFull.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Sprint", meta = (ClampMin = "0"))
	float SprintFovKick = 8.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Sprint", meta = (ClampMin = "0"))
	float SprintSpeedStart = 650.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Sprint", meta = (ClampMin = "0"))
	float SprintSpeedFull = 1100.0f;
	// How fast the widening eases in and out (per second).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Sprint", meta = (ClampMin = "0.1"))
	float FovEaseSpeed = 5.0f;

	// Horizontal field of view in third person; 0 uses the world's own (which widens on the hoverboard).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person")
	float ThirdPersonFov = 0.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person", meta = (ClampMin = "0"))
	float ArmLength = 420.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person", meta = (ClampMin = "0"))
	float FrontArmLength = 330.0f;
	// Where the arm starts: this high above her feet, and (behind) this far to her right.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person")
	float PivotHeight = 150.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person")
	float ShoulderOffset = 45.0f;
	// The arm's collision probe: a sphere this wide swept along it. It pulls in at once and lets out at ArmEaseOut.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person", meta = (ClampMin = "1"))
	float ProbeRadius = 18.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person", meta = (ClampMin = "0.1"))
	float ArmEaseOut = 3.0f;
	// Camera lag: how fast the arm's start follows her (light) and its turn follows the look (very light).
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person", meta = (ClampMin = "0.1"))
	float PositionLagSpeed = 14.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person", meta = (ClampMin = "0.1"))
	float RotationLagSpeed = 30.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person")
	float ThirdPersonPitchMin = -70.0f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person")
	float ThirdPersonPitchMax = 55.0f;
	// Her body is hidden while the camera is nearer her than this.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person", meta = (ClampMin = "0"))
	float HideDistance = 70.0f;
	// The camera stays at least this high above her feet.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Third person")
	float MinCameraHeight = 25.0f;

	// The built-in shake on hits and impacts (degrees, decaying over ShakeTime); 0 turns it off.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Shake", meta = (ClampMin = "0"))
	float HitShakeDegrees = 0.6f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Shake", meta = (ClampMin = "0"))
	float ImpactShakeDegrees = 1.2f;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Shake", meta = (ClampMin = "0.01"))
	float ShakeTime = 0.3f;
	// Optional camera shake assets, started on the player camera manager with the same events.
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Shake")
	TSubclassOf<UCameraShakeBase> HitShake;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Shake")
	TSubclassOf<UCameraShakeBase> ImpactShake;
	UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Shake", meta = (ClampMin = "0"))
	float ShakeAssetScale = 0.3f;
};
