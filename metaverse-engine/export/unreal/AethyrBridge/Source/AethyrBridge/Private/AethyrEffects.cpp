// Effects: what the rules decided, shown with Unreal's own particles and sound (the README's "Effects";
// docs/unreal/2026-11-14-what-belongs-in-unreal.md, section 5; GitHub #13599). The rules say what happened
// and where (aethyr_event_records) and what each thing is doing (aethyr_thing_states); rows of the
// overrides table say which Niagara system shows it. Nothing here goes back to the rules.
#include "AethyrGame.h"
#include "AethyrManifest.h"

#include "Camera/CameraComponent.h"
#include "Components/SceneComponent.h"
#include "Curves/CurveLinearColor.h"
#include "Engine/DataTable.h"
#include "Engine/World.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "NiagaraComponent.h"
#include "NiagaraFunctionLibrary.h"
#include "NiagaraSystem.h"
#include "Sound/SoundAttenuation.h"
#include "Sound/SoundBase.h"

#if AETHYR_HAS_RULES
extern "C" {
#include "aethyr_host.h"
}
#endif

DEFINE_LOG_CATEGORY_STATIC(LogAethyr, Log, All);

namespace
{
	const float Units = 100.0f;   // Unreal centimetres a metre
	// Metaverse Engine metres, y up, right-handed -> Unreal centimetres, z up (as AethyrGame.cpp)
	FVector ToUnreal(const FVector& M) { return FVector(M.X * Units, M.Z * Units, M.Y * Units); }
	FVector DirToUnreal(const FVector& M) { return FVector(M.X, M.Z, M.Y); }
	const int32 EventFloats = 9, StateFloats = 16;

	// The flags' bits when the manifest asset was imported before it listed them (splice-game.coax TF_*).
	int32 FlagBit(const UAethyrManifest* Manifest, const FString& Name)
	{
		if (Manifest) { if (const int32* Bit = Manifest->StateFlags.Find(FName(*Name))) { return *Bit; } }
		static const TCHAR* Names[8] = { TEXT("sprinting"), TEXT("board"), TEXT("drilling"), TEXT("stunned"), TEXT("charging"), TEXT("dead"), TEXT("driving"), TEXT("open") };
		if (!Manifest || Manifest->StateFlags.Num() == 0) { for (int32 K = 0; K < 8; K++) { if (Name == Names[K]) { return 1 << K; } } }
		return 0;
	}
}

void AAethyrGame::SetUpEffects()
{
	const TCHAR* Args = FCommandLine::Get();
	FParse::Value(Args, TEXT("AethyrReplay="), ReplayFile, false);
	FParse::Value(Args, TEXT("AethyrRecord="), RecordPath, false);
	FParse::Value(Args, TEXT("AethyrFxScale="), EffectScale);
	if (!ReplayFile.IsEmpty())
	{
		bReplaying = FFileHelper::LoadFileToStringArray(ReplayLines, *ReplayFile) && ReplayLines.Num() > 1;
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_REPLAY %s: %d lines%s"), *ReplayFile, ReplayLines.Num(), bReplaying ? TEXT("") : TEXT(" (not played)"));
	}
	if (!RecordPath.IsEmpty()) { RecordLines.Add(TEXT("aethyr-fx-record 1")); }
	FxRows.Reset(); EventFx.Reset(); FxKeep.Reset(); FxFill = nullptr; FxShade = nullptr;
	if (!Overrides || !Manifest) { return; }
	for (const FName& Row : Overrides->GetRowNames())
	{
		const FString Key = Row.ToString();
		if (Key == TEXT("look.fx.fill") || Key == TEXT("look.fx.shade"))
		{
			UCurveLinearColor* Curve = Cast<UCurveLinearColor>(OverrideFor(Key));
			if (Curve) { (Key.EndsWith(TEXT("fill")) ? FxFill : FxShade) = Curve; FxKeep.Add(Curve); }
			else { UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX row %s is not a colour curve"), *Key); }
			continue;
		}
		const bool bEvent = Key.StartsWith(TEXT("fx.")), bTrail = Key.StartsWith(TEXT("trail.")), bState = Key.StartsWith(TEXT("state."));
		if (!bEvent && !bTrail && !bState) { continue; }
		UNiagaraSystem* System = Cast<UNiagaraSystem>(OverrideFor(Key));
		if (!System) { UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX row %s is not a Niagara system"), *Key); continue; }
		if (bEvent)
		{
			const FName Name(*Key.Mid(3));
			bool bKnown = false;
			for (const TPair<int32, FName>& Event : Manifest->Events) { if (Event.Value == Name) { bKnown = true; break; } }
			if (!bKnown) { UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX row %s names no event of the manifest"), *Key); continue; }
			EventFx.Add(Name, System); FxKeep.Add(System);
			continue;
		}
		FString Kind = Key.Mid(6), Flag;   // past "trail." or "state."
		if (bState && !Kind.Split(TEXT("."), &Kind, &Flag)) { UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX row %s should be state.KIND.FLAG"), *Key); continue; }
		FFxRow Made;
		Made.Kind = Manifest->ThingKinds.IndexOfByKey(FName(*Kind));
		Made.Flag = bState ? FlagBit(Manifest, Flag) : 0;
		Made.Name = Key; Made.System = System;
		if (Made.Kind < 0 || (bState && Made.Flag == 0)) { UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX row %s names no kind%s of the manifest"), *Key, bState ? TEXT(" or flag") : TEXT("")); continue; }
		FxRows.Add(Made); FxKeep.Add(System);
	}
	UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX %d event rows, %d trail and state rows, colours %s"), EventFx.Num(), FxRows.Num(), (FxFill || FxShade) ? TEXT("from the look's rows") : TEXT("each system's own"));
}

// What every system is told, when it declares the parameter: Strength (1 is ordinary), Speed (metres a
// second), Level (0 to 1, how far through its action the thing is), Direction (a unit vector, Unreal's
// axes), and Fill and Shade, the two flat colours for this time of day.
void AAethyrGame::SetFxParameters(UNiagaraComponent* Fx, float Strength, float Speed, float Level, const FVector& Direction)
{
	if (!Fx) { return; }
	static const FName NStrength(TEXT("Strength")), NSpeed(TEXT("Speed")), NLevel(TEXT("Level")), NDirection(TEXT("Direction")), NFill(TEXT("Fill")), NShade(TEXT("Shade"));
	Fx->SetVariableFloat(NStrength, Strength);
	Fx->SetVariableFloat(NSpeed, Speed);
	Fx->SetVariableFloat(NLevel, Level);
	Fx->SetVariableVec3(NDirection, Direction);
#if AETHYR_HAS_RULES
	const float Phase = State ? (float)aethyr_fact(State, 2) : 1.0f;   // 0 dawn, 1 day, 2 dusk, 3 night
#else
	const float Phase = 1.0f;
#endif
	if (FxFill) { Fx->SetVariableLinearColor(NFill, FxFill->GetLinearColorValue(Phase)); }
	if (FxShade) { Fx->SetVariableLinearColor(NShade, FxShade->GetLinearColorValue(Phase)); }
}

// One event, as the rules' record of it (metres, y up): its line in the log, the camera's shake, its
// sound at its place (an "event." row), and its effect there, turned to its direction (an "fx." row).
void AAethyrGame::FeedEvent(const float* R)
{
	const FName* Name = Manifest ? Manifest->Events.Find((int32)R[0]) : nullptr;
	if (!Name) { return; }
	if (!RecordPath.IsEmpty()) { EventLog.Append(R, EventFloats); }
	const FVector At = ToUnreal(FVector(R[1], R[2], R[3])), Dir = DirToUnreal(FVector(R[4], R[5], R[6]));
	UE_LOG(LogAethyr, Display, TEXT("AETHYR_EVENT %s"), *Name->ToString());
	// the camera's shake: a hit on her, or an impact (a wall broken, the boss down, a blow)
	static const FName Hurt(TEXT("hurt")), Smack(TEXT("smack")), Broke(TEXT("wall-broke")), Down(TEXT("boss-down"));
	if (*Name == Hurt) { PlayCameraShake(false); }
	else if (*Name == Smack || *Name == Broke || *Name == Down) { PlayCameraShake(true); }
	if (USoundBase* Sound = Cast<USoundBase>(OverrideFor(TEXT("event.") + Name->ToString())))
	{
		if (!EventFalloff)
		{
			// heard whole within 15 m of the listener, gone by 105 m, and from its side
			EventFalloff = NewObject<USoundAttenuation>(this);
			EventFalloff->Attenuation.bAttenuate = true;
			EventFalloff->Attenuation.bSpatialize = true;
			EventFalloff->Attenuation.AttenuationShape = EAttenuationShape::Sphere;
			EventFalloff->Attenuation.AttenuationShapeExtents = FVector(1500.0f);
			EventFalloff->Attenuation.FalloffDistance = 9000.0f;
		}
		UGameplayStatics::PlaySoundAtLocation(this, Sound, At, FRotator::ZeroRotator, 1.0f, 1.0f, 0.0f, EventFalloff);
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_SOUND %s %s"), *Name->ToString(), *Sound->GetName());
	}
	if (UNiagaraSystem* const* System = EventFx.Find(*Name))
	{
		const FRotator Turn = Dir.IsNearlyZero() ? FRotator::ZeroRotator : Dir.Rotation();
		UNiagaraComponent* Fx = UNiagaraFunctionLibrary::SpawnSystemAtLocation(this, *System, At, Turn, FVector(EffectScale), true, false, ENCPoolMethod::None, false);
		if (Fx)
		{
			SetFxParameters(Fx, R[7], 0.0f, 0.0f, Dir);
			Fx->Activate(true);
		}
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX %s plays %s at %.0f %.0f %.0f strength %.2f"), *Name->ToString(), *(*System)->GetName(), At.X, At.Y, At.Z, R[7]);
	}
}

// This frame's things and what each is doing: the rules', or the recording's.
void AAethyrGame::ReadStates()
{
	if (bReplaying) { return; }   // StepReplay wrote them
	States.Reset();
#if AETHYR_HAS_RULES
	if (!State) { return; }
	float Raw[56 * StateFloats];
	const int32 Count = aethyr_thing_states(State, Raw, 56 * StateFloats);
	for (int32 I = 0; I < Count; I++)
	{
		const float* T = Raw + I * StateFloats;
		FThingState S;
		S.Id = (int32)T[0]; S.Kind = (int32)T[1]; S.Variant = (int32)T[2]; S.Flags = (int32)T[3];
		S.At = FVector(T[4], T[5], T[6]); S.Yaw = T[7]; S.Velocity = FVector(T[8], T[9], T[10]); S.Level = T[11]; S.ActAt = FVector(T[12], T[13], T[14]);
		States.Add(S);
	}
#endif
}

// The systems kept on things: one for each thing a "trail." row's kind lists, one for each thing with a
// "state." row's flag set. Each follows its thing (a state's stands where the thing's action is), faces
// the way the thing moves or, at rest, the way it faces, and is told the thing's speed and level every
// frame. When the thing goes or the flag clears the system is told to stop, and ends when its last
// particle does.
void AAethyrGame::UpdateEffects(float DeltaSeconds)
{
	ReadStates();
	TSet<int64> Seen;
	for (const FThingState& S : States)
	{
		for (int32 R = 0; R < FxRows.Num(); R++)
		{
			const FFxRow& Row = FxRows[R];
			if (Row.Kind != S.Kind || (Row.Flag != 0 && (S.Flags & Row.Flag) == 0)) { continue; }
			const int64 Key = ((int64)S.Id << 16) | R;
			Seen.Add(Key);
			const float Speed = S.Velocity.Size();
			const FVector Face(FMath::Sin(S.Yaw), FMath::Cos(S.Yaw), 0.0f);
			const FVector Way = DirToUnreal(S.Velocity).GetSafeNormal2D();
			const FVector Dir = (Speed > 0.5f && !Way.IsNearlyZero()) ? Way : Face;
			const FVector Where = ToUnreal(Row.Flag != 0 ? S.ActAt : S.At);
			TObjectPtr<UNiagaraComponent>& Fx = FxLive.FindOrAdd(Key);
			if (!Fx)
			{
				Fx = UNiagaraFunctionLibrary::SpawnSystemAttached(Row.System, RootComponent, NAME_None, Where, Dir.Rotation(), FVector(EffectScale), EAttachLocation::KeepWorldPosition, false, ENCPoolMethod::None, false, false);
				if (Fx) { SetFxParameters(Fx, 1.0f, Speed, S.Level, Dir); Fx->Activate(true); }
				UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX %s starts %s on thing %d"), *Row.Name, *Row.System->GetName(), S.Id);
			}
			else
			{
				Fx->SetWorldLocationAndRotation(Where, Dir.Rotation());
				SetFxParameters(Fx, 1.0f, Speed, S.Level, Dir);
			}
		}
	}
	for (auto It = FxLive.CreateIterator(); It; ++It)
	{
		if (Seen.Contains(It.Key())) { continue; }
		if (UNiagaraComponent* Fx = It.Value())
		{
			Fx->SetAutoDestroy(true);
			Fx->Deactivate();
			const int32 R = (int32)(It.Key() & 0xffff);
			UE_LOG(LogAethyr, Display, TEXT("AETHYR_FX %s stops on thing %d"), FxRows.IsValidIndex(R) ? *FxRows[R].Name : TEXT("?"), (int32)(It.Key() >> 16));
		}
		It.RemoveCurrent();
	}
	if (bReplaying && bReplayView && Camera)
	{
		Camera->SetWorldLocationAndRotation(ReplayViewAt, ReplayViewTurn);
		Camera->SetFieldOfView(ReplayViewFov);
	}
	RecordFrame(DeltaSeconds);
}

// ---- a run's events and things in a file (-AethyrRecord=FILE), and that file played back (-AethyrReplay=FILE,
// or the actor's Replay File). Text, a line each: "f DT" starts a frame, "v" is the view shown (Unreal's place,
// turn and angle), "e" an event's nine floats, "t" a thing's sixteen. Events and things are the rules' own
// numbers, in metres.

void AAethyrGame::RecordFrame(float DeltaSeconds)
{
	if (RecordPath.IsEmpty() || bReplaying) { return; }
	RecordLines.Add(FString::Printf(TEXT("f %.5f"), DeltaSeconds));
	RecordLines.Add(FString::Printf(TEXT("v %.2f %.2f %.2f %.6f %.6f %.6f %.6f %.3f"), ViewAt.X, ViewAt.Y, ViewAt.Z, ViewTurn.X, ViewTurn.Y, ViewTurn.Z, ViewTurn.W, ViewFov));
	for (int32 I = 0; I + EventFloats <= EventLog.Num(); I += EventFloats)
	{
		const float* R = EventLog.GetData() + I;
		RecordLines.Add(FString::Printf(TEXT("e %d %.3f %.3f %.3f %.4f %.4f %.4f %.3f %d"), (int32)R[0], R[1], R[2], R[3], R[4], R[5], R[6], R[7], (int32)R[8]));
	}
	EventLog.Reset();
	for (const FThingState& S : States)
	{
		RecordLines.Add(FString::Printf(TEXT("t %d %d %d %d %.3f %.3f %.3f %.4f %.3f %.3f %.3f %.3f %.3f %.3f %.3f 0"), S.Id, S.Kind, S.Variant, S.Flags,
			S.At.X, S.At.Y, S.At.Z, S.Yaw, S.Velocity.X, S.Velocity.Y, S.Velocity.Z, S.Level, S.ActAt.X, S.ActAt.Y, S.ActAt.Z));
	}
	if ((RecordLines.Num() & 1023) < 2 + States.Num()) { SaveRecording(); }
}

void AAethyrGame::SaveRecording()
{
	if (!RecordPath.IsEmpty() && RecordLines.Num() > 1) { FFileHelper::SaveStringArrayToFile(RecordLines, *RecordPath); }
}

// The recording's next frame: its events are fed as the rules' would be, its things become this frame's.
// At its end it starts again. False when there is nothing to play.
bool AAethyrGame::StepReplay()
{
	if (!bReplaying) { return false; }
	States.Reset();
	bool bStarted = false;
	for (int32 Guard = 0; Guard < ReplayLines.Num() + 2; Guard++)
	{
		if (ReplayLine >= ReplayLines.Num())
		{
			if (bStarted) { break; }
			ReplayLine = 0;
			continue;
		}
		const FString& Line = ReplayLines[ReplayLine];
		if (Line.StartsWith(TEXT("f ")))
		{
			if (bStarted) { break; }
			bStarted = true; ReplayLine++;
			continue;
		}
		ReplayLine++;
		if (!bStarted) { continue; }
		TArray<FString> W;
		Line.ParseIntoArrayWS(W);
		if (W.Num() == 9 && W[0] == TEXT("v"))
		{
			ReplayViewAt = FVector(FCString::Atod(*W[1]), FCString::Atod(*W[2]), FCString::Atod(*W[3]));
			ReplayViewTurn = FQuat(FCString::Atod(*W[4]), FCString::Atod(*W[5]), FCString::Atod(*W[6]), FCString::Atod(*W[7]));
			ReplayViewFov = FCString::Atof(*W[8]);
			bReplayView = !FParse::Param(FCommandLine::Get(), TEXT("AethyrReplayFreeView"));
		}
		else if (W.Num() == 1 + EventFloats && W[0] == TEXT("e"))
		{
			float R[EventFloats];
			for (int32 K = 0; K < EventFloats; K++) { R[K] = FCString::Atof(*W[1 + K]); }
			FeedEvent(R);
		}
		else if (W.Num() == 1 + StateFloats && W[0] == TEXT("t"))
		{
			FThingState S;
			S.Id = FCString::Atoi(*W[1]); S.Kind = FCString::Atoi(*W[2]); S.Variant = FCString::Atoi(*W[3]); S.Flags = FCString::Atoi(*W[4]);
			S.At = FVector(FCString::Atof(*W[5]), FCString::Atof(*W[6]), FCString::Atof(*W[7])); S.Yaw = FCString::Atof(*W[8]);
			S.Velocity = FVector(FCString::Atof(*W[9]), FCString::Atof(*W[10]), FCString::Atof(*W[11])); S.Level = FCString::Atof(*W[12]);
			S.ActAt = FVector(FCString::Atof(*W[13]), FCString::Atof(*W[14]), FCString::Atof(*W[15]));
			States.Add(S);
		}
	}
	return bStarted;
}
