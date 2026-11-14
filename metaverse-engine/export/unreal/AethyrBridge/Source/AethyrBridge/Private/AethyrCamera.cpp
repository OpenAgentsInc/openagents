// The Unreal host's player camera (AethyrCamera.h). Each tick it reads the world's own camera from the uniform
// block and her place and facing from the things list (kind 0), and, while she is on foot, places the view
// itself: first person at her eye, a spring arm behind her shoulder, or one in front looking back. Nothing here
// allocates per frame: the things list and the message to the world are stack buffers, and traces use one
// sphere and no ignore list (the host's own meshes do not collide).
#include "AethyrGame.h"

#include "Camera/CameraComponent.h"
#include "Camera/CameraShakeBase.h"
#include "Camera/PlayerCameraManager.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "EngineUtils.h"
#include "GameFramework/PlayerController.h"
#include "HAL/IConsoleManager.h"
#include "InputCoreTypes.h"
#include "Misc/ConfigCacheIni.h"

#if AETHYR_HAS_RULES
extern "C" {
#include "aethyr_host.h"
}
#endif

DEFINE_LOG_CATEGORY_STATIC(LogAethyrCamera, Log, All);

namespace
{
	// the player's settings, also settable from the console (and so from a web harness's exec steps)
	TAutoConsoleVariable<int32> CVarMode(TEXT("aethyr.cam.Mode"), -1, TEXT("The camera's view: 0 first person, 1 behind, 2 front (-1: as it is)."));
	TAutoConsoleVariable<float> CVarMouse(TEXT("aethyr.cam.MouseSensitivity"), 1.0f, TEXT("Mouse look speed, times the tuning's."));
	TAutoConsoleVariable<float> CVarStick(TEXT("aethyr.cam.StickSensitivity"), 1.0f, TEXT("Stick look speed, times the tuning's."));
	TAutoConsoleVariable<int32> CVarInvert(TEXT("aethyr.cam.InvertY"), 0, TEXT("1: pushing the mouse or stick up looks down."));
	TAutoConsoleVariable<int32> CVarBob(TEXT("aethyr.cam.HeadBob"), 1, TEXT("0: no head bob in first person."));

	const TCHAR* CamSection = TEXT("AethyrCamera");   // GameUserSettings.ini

	const float GameLookDegrees = 0.18335f;   // one unit of aethyr_look_delta: the world's T_LOOK, 0.0032 rad

	FVector ToUnreal(float X, float Y, float Z) { return FVector(X * 100.0f, Z * 100.0f, Y * 100.0f); }
	FVector DirToUnreal(float X, float Y, float Z) { return FVector(X, Z, Y); }
	// Unreal centimetres, z up -> the world's millimetres, y up
	void ToGameMm(const FVector& V, int32* Out) { Out[0] = FMath::RoundToInt(V.X * 10.0f); Out[1] = FMath::RoundToInt(V.Z * 10.0f); Out[2] = FMath::RoundToInt(V.Y * 10.0f); }

	float Ease(float Rate, float Dt) { return 1.0f - FMath::Exp(-Rate * Dt); }
	float Smooth01(float T) { T = FMath::Clamp(T, 0.0f, 1.0f); return T * T * (3.0f - 2.0f * T); }
	const char* ModeName(EAethyrCameraMode M) { return M == EAethyrCameraMode::FirstPerson ? "first" : M == EAethyrCameraMode::Front ? "front" : "behind"; }

	// aethyr.Message KIND BODY: a message to the world, as the browser host sends one (a test's teleport: 611 X Z)
	FAutoConsoleCommandWithWorldAndArgs MessageCommand(TEXT("aethyr.Message"), TEXT("aethyr.Message KIND BODY: a message to the world."),
		FConsoleCommandWithWorldAndArgsDelegate::CreateLambda([](const TArray<FString>& Args, UWorld* World)
		{
			if (!World || Args.Num() < 1) { return; }
			FString Body;
			for (int32 I = 1; I < Args.Num(); I++) { if (I > 1) { Body += TEXT(" "); } Body += Args[I]; }
			for (TActorIterator<AAethyrGame> It(World); It; ++It) { It->SendWorldMessage(FCString::Atoi(*Args[0]), Body); }
		}));
}

const UAethyrCameraTuning& AAethyrGame::Tuning() const
{
	return CameraTuning ? *CameraTuning : *GetDefault<UAethyrCameraTuning>();
}

void AAethyrGame::SendWorldMessage(int32 Kind, const FString& Body)
{
#if AETHYR_HAS_RULES
	if (!State) { return; }
	const FTCHARToUTF8 Text(*Body);
	aethyr_message(State, Kind, Text.Get(), Text.Length());
#endif
}

// The view and the player's settings as they were left (GameUserSettings.ini), or the tuning's defaults.
void AAethyrGame::ReadCameraSettings()
{
	int32 Mode = (int32)Tuning().DefaultMode;
	float Mouse = 1.0f, Stick = 1.0f; bool bInv = false, bHeadBob = Tuning().bHeadBob;
	if (GConfig)
	{
		GConfig->GetInt(CamSection, TEXT("Mode"), Mode, GGameUserSettingsIni);
		GConfig->GetFloat(CamSection, TEXT("MouseSensitivity"), Mouse, GGameUserSettingsIni);
		GConfig->GetFloat(CamSection, TEXT("StickSensitivity"), Stick, GGameUserSettingsIni);
		GConfig->GetBool(CamSection, TEXT("InvertY"), bInv, GGameUserSettingsIni);
		GConfig->GetBool(CamSection, TEXT("HeadBob"), bHeadBob, GGameUserSettingsIni);
	}
	CameraMode = BlendedMode = (EAethyrCameraMode)FMath::Clamp(Mode, 0, 2);
	CVarMouse->Set(Mouse, ECVF_SetByGameSetting); CVarStick->Set(Stick, ECVF_SetByGameSetting);
	CVarInvert->Set(bInv ? 1 : 0, ECVF_SetByGameSetting); CVarBob->Set(bHeadBob ? 1 : 0, ECVF_SetByGameSetting);
	MouseScale = Mouse; StickScale = Stick; bInvertY = bInv; bBob = bHeadBob;
	SeenModeVar = CVarMode->GetInt();
}

void AAethyrGame::SetCameraMode(EAethyrCameraMode Mode)
{
	if (Mode == CameraMode) { return; }
	CameraMode = Mode;
	if (GConfig) { GConfig->SetInt(CamSection, TEXT("Mode"), (int32)Mode, GGameUserSettingsIni); GConfig->Flush(false, GGameUserSettingsIni); }
	UE_LOG(LogAethyrCamera, Display, TEXT("AETHYR_CAMERA view %hs"), ModeName(Mode));
}

void AAethyrGame::CycleCameraMode()
{
	SetCameraMode(CameraMode == EAethyrCameraMode::FirstPerson ? EAethyrCameraMode::Behind
		: CameraMode == EAethyrCameraMode::Behind ? EAethyrCameraMode::Front : EAethyrCameraMode::FirstPerson);
}

void AAethyrGame::PlayCameraShake(bool bImpact, float Scale)
{
	const UAethyrCameraTuning& T = Tuning();
	const float Degrees = (bImpact ? T.ImpactShakeDegrees : T.HitShakeDegrees) * Scale;
	if (Degrees > ShakeDegrees * FMath::Max(0.0f, 1.0f - ShakeAge / T.ShakeTime)) { ShakeDegrees = Degrees; ShakeAge = 0.0f; }
	const TSubclassOf<UCameraShakeBase> Asset = bImpact ? T.ImpactShake : T.HitShake;
	if (Asset)
	{
		if (APlayerController* Player = GetWorld()->GetFirstPlayerController())
		{
			if (Player->PlayerCameraManager) { Player->PlayerCameraManager->StartCameraShake(Asset, T.ShakeAssetScale * Scale); }
		}
	}
}

bool AAethyrGame::AimTrace(FHitResult& Hit, float Distance) const
{
	FCollisionQueryParams Params(SCENE_QUERY_STAT(AethyrAim), false);
	return GetWorld()->LineTraceSingleByChannel(Hit, AimFrom, AimFrom + AimDir * Distance, ECC_Visibility, Params);
}

// The mouse and the right stick: smoothed, scaled, Y inverted when asked. The turn goes to the world (which way
// she walks is its), the pitch is the host's own while it places the view (and goes to the world too, which keeps
// it for its own camera within its own limits).
void AAethyrGame::LookInput(APlayerController* Player, float Dt)
{
#if AETHYR_HAS_RULES
	const UAethyrCameraTuning& T = Tuning();
	float Dx = 0, Dy = 0;
	Player->GetInputMouseDelta(Dx, Dy);   // Unreal's delta is up-positive
	float Yaw = Dx * T.MouseDegreesPerUnit * MouseScale, Pitch = Dy * T.MouseDegreesPerUnit * MouseScale;
	auto Stick = [&T](float V) { const float A = FMath::Abs(V); return A < T.StickDeadZone ? 0.0f : FMath::Sign(V) * (A - T.StickDeadZone) / (1.0f - T.StickDeadZone); };
	const float Sx = Stick(Player->GetInputAnalogKeyState(EKeys::Gamepad_RightX)), Sy = Stick(Player->GetInputAnalogKeyState(EKeys::Gamepad_RightY));
	Yaw += Sx * T.StickYawSpeed * StickScale * Dt; Pitch += Sy * T.StickPitchSpeed * StickScale * Dt;
	if (bInvertY) { Pitch = -Pitch; }
	if (T.LookSmoothing > 0 && Dt > 0)
	{
		const float K = 1.0f - FMath::Exp(-Dt / T.LookSmoothing);
		LookYawRate += (Yaw / Dt - LookYawRate) * K; LookPitchRate += (Pitch / Dt - LookPitchRate) * K;
		Yaw = LookYawRate * Dt; Pitch = LookPitchRate * Dt;
	}
	if (FMath::Abs(Yaw) < 1e-4f && FMath::Abs(Pitch) < 1e-4f) { return; }
	aethyr_look_delta(State, Yaw / GameLookDegrees, -Pitch / GameLookDegrees);
	// Hold R: the pointer picks on the bot wheel, not the view
	if (bCameraOwned && !Player->IsInputKeyDown(EKeys::R)) { LookPitch += Pitch; }
#endif
}

void AAethyrGame::UpdateCamera(float Dt)
{
#if AETHYR_HAS_RULES
	if (Uniforms.Num() < FMath::Max3(EyeAt, ForwardAt, UpAt) + 4 || !State) { return; }
	const UAethyrCameraTuning& T = Tuning();
	Dt = FMath::Clamp(Dt, 0.0f, 0.1f);
	float* U = Uniforms.GetData();
	// the world's own camera
	const FVector GameEye = ToUnreal(U[EyeAt], U[EyeAt + 1], U[EyeAt + 2]);
	const FVector GameFwd = DirToUnreal(U[ForwardAt], U[ForwardAt + 1], U[ForwardAt + 2]);
	const FVector GameUp = DirToUnreal(U[UpAt], U[UpAt + 1], U[UpAt + 2]);
	if (GameFwd.IsNearlyZero()) { return; }
	const float Tan = U[EyeAt + 3], Aspect = U[ForwardAt + 3];
	const float GameFov = (Tan > 0 && Aspect > 0) ? FMath::RadiansToDegrees(2.0f * FMath::Atan(Tan * Aspect)) : 90.0f;

	// the console's settings (cheap reads; a change is kept)
	const int32 ModeVar = CVarMode->GetInt();
	if (ModeVar != SeenModeVar) { SeenModeVar = ModeVar; if (ModeVar >= 0 && ModeVar <= 2) { SetCameraMode((EAethyrCameraMode)ModeVar); } }
	const float Mouse = CVarMouse->GetFloat(), Stick = CVarStick->GetFloat(); const bool bInv = CVarInvert->GetInt() != 0, bHeadBob = CVarBob->GetInt() != 0;
	if (Mouse != MouseScale || Stick != StickScale || bInv != bInvertY || bHeadBob != bBob)
	{
		MouseScale = Mouse; StickScale = Stick; bInvertY = bInv; bBob = bHeadBob;
		if (GConfig)
		{
			GConfig->SetFloat(CamSection, TEXT("MouseSensitivity"), Mouse, GGameUserSettingsIni); GConfig->SetFloat(CamSection, TEXT("StickSensitivity"), Stick, GGameUserSettingsIni);
			GConfig->SetBool(CamSection, TEXT("InvertY"), bInv, GGameUserSettingsIni); GConfig->SetBool(CamSection, TEXT("HeadBob"), bHeadBob, GGameUserSettingsIni);
			GConfig->Flush(false, GGameUserSettingsIni);
		}
	}

	// her place and facing: thing kind 0 (missing for a frame while she blinks after a hit)
	float Things[48 * 8];
	const int32 Count = aethyr_things(State, Things, 48 * 8);
	bool bFound = false; FVector Feet = PlayerAt;
	for (int32 I = 0; I < Count; I++) { if ((int32)Things[I * 8] == 0) { Feet = ToUnreal(Things[I * 8 + 2], Things[I * 8 + 3], Things[I * 8 + 4]); bFound = true; break; } }
	bool bSnap = !bHavePlayer;
	if (bFound)
	{
		const FVector Step = Feet - PlayerAt;
		if (!bHavePlayer || Step.Size() > 400.0f || Dt <= 0) { bSnap = true; PlayerSpeed = 0; PlayerClimb = 0; bAirborne = bFalling = false; }   // a teleport or a door
		else
		{
			PlayerSpeed += (Step.Size2D() / Dt - PlayerSpeed) * Ease(10.0f, Dt);
			PlayerClimb += (Step.Z / Dt - PlayerClimb) * Ease(20.0f, Dt);
		}
		PlayerAt = Feet; bHavePlayer = true;
	}
	// airborne and landing: a fall of LandMinFall or more dips the eye when the climb stops
	if (PlayerClimb > 60.0f || PlayerClimb < -60.0f)
	{
		if (!bAirborne) { bAirborne = true; AirPeak = PlayerAt.Z; }
		AirPeak = FMath::Max(AirPeak, PlayerAt.Z);
		if (PlayerClimb < -60.0f) { bFalling = true; }
	}
	else if (bAirborne && FMath::Abs(PlayerClimb) < 25.0f && bFalling)
	{
		const float Fall = AirPeak - PlayerAt.Z;
		if (Fall >= T.LandMinFall) { LandDip = FMath::Min(Fall * 0.01f * T.LandDipPerMetre, T.LandDipMax); LandAge = 0; }
		bAirborne = bFalling = false;
	}
	LandAge += Dt;

	// the host places the view while she is on foot (the world's mode 1); everywhere else the world's own camera
	const bool bOwned = bHavePlayer && aethyr_fact(State, 0) == 1;
	const float ViewYaw = FMath::RadiansToDegrees(FMath::Atan2(GameFwd.Y, GameFwd.X));
	if (!bOwned) { LookPitch = FMath::RadiansToDegrees(FMath::Asin(FMath::Clamp(GameFwd.Z, -1.0, 1.0))); }
	const bool bFirst = CameraMode == EAethyrCameraMode::FirstPerson, bFront = CameraMode == EAethyrCameraMode::Front;
	LookPitch = bFirst ? FMath::Clamp(LookPitch, -T.FirstPersonPitchLimit, T.FirstPersonPitchLimit) : FMath::Clamp(LookPitch, T.ThirdPersonPitchMin, T.ThirdPersonPitchMax);

	// speed: the sprint widening and the bob's weight
	const float SprintGoal = T.SprintSpeedFull > T.SprintSpeedStart ? FMath::Clamp((PlayerSpeed - T.SprintSpeedStart) / (T.SprintSpeedFull - T.SprintSpeedStart), 0.0f, 1.0f) : 0.0f;
	SprintAlpha += (SprintGoal - SprintAlpha) * Ease(T.FovEaseSpeed, Dt);
	const float BobGoal = (bBob && !bAirborne) ? FMath::Clamp(PlayerSpeed / T.BobFullSpeed, 0.0f, 1.0f) : 0.0f;
	BobWeight += (BobGoal - BobWeight) * Ease(8.0f, Dt);
	BobPhase = FMath::Fmod(BobPhase + PlayerSpeed * 0.01f * Dt / T.BobCycleMetres * 2.0f * PI, 2.0f * PI);

	FVector At = GameEye; FQuat Turn = FRotationMatrix::MakeFromXZ(GameFwd, GameUp).ToQuat(); float Fov = GameFov;
	const FVector Up = FVector::UpVector;
	const FVector Head = PlayerAt + Up * T.EyeHeight;
	FCollisionQueryParams Params(SCENE_QUERY_STAT(AethyrCamera), false);
	UWorld* World = GetWorld();
	if (bOwned)
	{
		const FRotator LookTurn(LookPitch, ViewYaw, 0.0f);
		const FVector YawRight = FRotator(0.0f, ViewYaw, 0.0f).Quaternion().GetRightVector();
		if (bFirst)
		{
			Turn = LookTurn.Quaternion();
			const float Dip = LandAge < 0.06f ? LandDip * LandAge / 0.06f : LandDip * FMath::Exp(-(LandAge - 0.06f) * T.LandRecover);
			At = Head + Up * (FMath::Sin(BobPhase * 2.0f) * T.BobVertical * BobWeight - Dip) + YawRight * (FMath::Sin(BobPhase) * T.BobSide * BobWeight);
			// never into a wall: the eye is swept out from her middle
			const FVector From = PlayerAt + Up * (T.EyeHeight * 0.6f);
			FHitResult Hit;
			if (World && World->SweepSingleByChannel(Hit, From, At, FQuat::Identity, ECC_Camera, FCollisionShape::MakeSphere(T.FirstPersonProbeRadius), Params) && !Hit.bStartPenetrating) { At = Hit.Location; }
			Fov = T.FirstPersonFov;
			PivotNow = PlayerAt + Up * T.PivotHeight; TurnNow = Turn; ArmNow = 0.0f;
		}
		else
		{
			const FVector Pivot = PlayerAt + Up * T.PivotHeight;
			PivotNow = bSnap ? Pivot : FMath::Lerp(PivotNow, Pivot, Ease(T.PositionLagSpeed, Dt));
			const FQuat Goal = (bFront ? FRotator(-LookPitch, ViewYaw + 180.0f, 0.0f) : LookTurn).Quaternion();
			TurnNow = (bSnap || BlendedMode != CameraMode) ? Goal : FQuat::Slerp(TurnNow, Goal, Ease(T.RotationLagSpeed, Dt)).GetNormalized();
			const FVector Base = PivotNow + (bFront ? FVector::ZeroVector : YawRight * T.ShoulderOffset);
			float Arm = bFront ? T.FrontArmLength : T.ArmLength;
			// where the world itself pulls its camera in (the rover's cabin, inside a building), so does the arm
			const float WorldArm = FVector::Dist(GameEye, PlayerAt + Up * 170.0f);
			if (!bFront && WorldArm < Arm * 0.75f) { Arm = FMath::Max(WorldArm, 60.0f); }
			const FVector Back = -TurnNow.GetForwardVector();
			FHitResult Hit;
			float Reach = Arm;
			if (World && World->SweepSingleByChannel(Hit, Base, Base + Back * Arm, FQuat::Identity, ECC_Camera, FCollisionShape::MakeSphere(T.ProbeRadius), Params)) { Reach = Hit.Distance; }
			// a probe that starts inside a collider (her shoulder in a cabin wall, a room's blocking volume around
			// her) tells nothing: try from her middle, and failing that keep the world's own arm, which its rooms limit
			static bool bWasInside = false;
			if (Hit.bStartPenetrating != bWasInside)
			{
				bWasInside = Hit.bStartPenetrating;
				if (bWasInside) { UE_LOG(LogAethyrCamera, Display, TEXT("AETHYR_CAMERA probe starts inside %s"), Hit.GetActor() ? *Hit.GetActor()->GetName() : TEXT("?")); }
			}
			if (Hit.bStartPenetrating)
			{
				const FVector Middle = PivotNow;
				FHitResult Again;
				if (World && World->SweepSingleByChannel(Again, Middle, Middle + Back * Arm, FQuat::Identity, ECC_Camera, FCollisionShape::MakeSphere(T.ProbeRadius), Params))
				{
					Reach = Again.bStartPenetrating ? FMath::Min(Arm, WorldArm) : Again.Distance;
				}
				else { Reach = Arm; }
			}
			ArmNow = (bSnap || Reach < ArmNow || ArmNow <= 0.0f) ? Reach : ArmNow + (Reach - ArmNow) * Ease(T.ArmEaseOut, Dt);   // in at once, out slowly
			At = (Hit.bStartPenetrating ? PivotNow : Base) + Back * ArmNow;
			At.Z = FMath::Max(At.Z, PlayerAt.Z + T.MinCameraHeight);
			Turn = TurnNow;
			Fov = T.ThirdPersonFov > 0 ? T.ThirdPersonFov : GameFov;
		}
		Fov += T.SprintFovKick * SprintAlpha;
	}
	// the shake: small, decaying turns
	ShakeAge += Dt;
	if (ShakeAge < T.ShakeTime && ShakeDegrees > 0)
	{
		const float K = FMath::Square(1.0f - ShakeAge / T.ShakeTime) * ShakeDegrees, S = ShakeAge;
		Turn = Turn * FRotator(FMath::Sin(S * 47.0f) * K, FMath::Sin(S * 61.0f + 1.0f) * K, FMath::Sin(S * 37.0f + 2.0f) * K * 0.5f).Quaternion();
	}

	// a change of view, or to and from the world's camera, blends from what was last shown
	if (!bCameraReady) { ViewAt = At; ViewTurn = Turn; ViewFov = Fov; BlendAge = 10.0f; bCameraReady = true; }
	else if (bOwned != bCameraOwned || (bOwned && BlendedMode != CameraMode)) { BlendFromAt = ViewAt; BlendFromTurn = ViewTurn; BlendFromFov = ViewFov; BlendAge = 0.0f; }
	bCameraOwned = bOwned; BlendedMode = CameraMode;
	float Blend = 1.0f;
	if (BlendAge < T.BlendTime && T.BlendTime > 0)
	{
		Blend = Smooth01(BlendAge / T.BlendTime);
		At = FMath::Lerp(BlendFromAt, At, Blend); Turn = FQuat::Slerp(BlendFromTurn, Turn, Blend).GetNormalized(); Fov = FMath::Lerp(BlendFromFov, Fov, Blend);
	}
	BlendAge += Dt;
	ViewAt = At; ViewTurn = Turn; ViewFov = Fov;

	// her body: hidden in first person (from halfway through the blend in) and whenever the camera is in close
	const bool bHide = bOwned && ((bFirst && Blend >= 0.5f) || FVector::Dist(At, PlayerAt + Up * (T.EyeHeight * 0.7f)) < T.HideDistance);
	if (bHide != bBodyHidden) { bBodyHidden = bHide; if (!NativeKinds.Contains(0)) { aethyr_thing_hide(State, 0, bHide ? 1 : 0); } }   // a native model hides in UpdateThings

	Camera->SetWorldLocationAndRotation(At, Turn);
	Camera->SetFieldOfView(Fov);
	Sky->SetWorldLocation(At);

	// the aim: from the eye in first person; in third from her head, along the view (behind) or her facing view (front)
	const FVector Fwd = Turn.GetForwardVector();
	AimFrom = (bOwned && !bFirst) ? Head : At;
	AimDir = (bOwned && bFront) ? FRotator(LookPitch, ViewYaw, 0.0f).Vector() : Fwd;

	if (bOwned)
	{
		// the look's material shades for the view shown (the world's coordinates in the uniform block)
		const FVector CamUp = Turn.GetUpVector();
		U[EyeAt] = At.X * 0.01f; U[EyeAt + 1] = At.Z * 0.01f; U[EyeAt + 2] = At.Y * 0.01f;
		U[ForwardAt] = Fwd.X; U[ForwardAt + 1] = Fwd.Z; U[ForwardAt + 2] = Fwd.Y;
		U[UpAt] = CamUp.X; U[UpAt + 1] = CamUp.Z; U[UpAt + 2] = CamUp.Y;
		if (Aspect > 0) { U[EyeAt + 3] = FMath::Tan(FMath::DegreesToRadians(Fov) * 0.5f) / Aspect; }
	}
	SendView(bOwned);

	static double CamLogAt = 0;
	const double Now = FPlatformTime::Seconds();
	if (Now - CamLogAt >= 1.0)
	{
		CamLogAt = Now;
		UE_LOG(LogAethyrCamera, Display, TEXT("AETHYR_CAMERA view %hs owned %d fov %.1f arm %.0f pitch %.1f speed %.0f hidden %d"),
			ModeName(CameraMode), bOwned ? 1 : 0, Fov, ArmNow, LookPitch, PlayerSpeed, bBodyHidden ? 1 : 0);
	}
#endif
}

// The world is told the view shown and the aim (message 622), so the bars over the bots sit on them and Digger is
// pointed where the crosshair is; "0" when the world's own camera is back. Not while a script plays the run (its
// clicks were written for the world's own camera) or without input. Sent only when it changes.
void AAethyrGame::SendView(bool bOwned)
{
#if AETHYR_HAS_RULES
	const bool bSend = bOwned && bInput && !bPlaying;
	if (!bSend)
	{
		if (bViewSent) { aethyr_message(State, 622, "0", 1); bViewSent = false; }
		return;
	}
	int32 V[14];
	ToGameMm(ViewAt, V); ToGameMm(ViewAt + ViewTurn.GetForwardVector() * 1000.0f, V + 3);
	const float Aspect = Uniforms[ForwardAt + 3] > 0 ? Uniforms[ForwardAt + 3] : 1.6f;
	V[6] = FMath::RoundToInt(2.0f * FMath::Atan(FMath::Tan(FMath::DegreesToRadians(ViewFov) * 0.5f) / Aspect) * 1000.0f);
	ToGameMm(AimFrom, V + 7); ToGameMm(AimFrom + AimDir * 1000.0f, V + 10);
	V[13] = 1;
	if (bViewSent && FMemory::Memcmp(V, SentView, sizeof(V)) == 0) { return; }
	FMemory::Memcpy(SentView, V, sizeof(V));
	char Body[192];
	const int32 N = FCStringAnsi::Snprintf(Body, sizeof(Body), "1 %d %d %d %d %d %d %d %d %d %d %d %d %d", V[0], V[1], V[2], V[3], V[4], V[5], V[6], V[7], V[8], V[9], V[10], V[11], V[12]);
	if (N > 0) { aethyr_message(State, 622, Body, (size_t)FMath::Min(N, (int32)sizeof(Body) - 1)); bViewSent = true; }
#endif
}
