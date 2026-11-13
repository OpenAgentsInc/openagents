#include "AethyrGame.h"

#include "Camera/CameraComponent.h"
#include "CanvasItem.h"
#include "Components/SceneComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/Canvas.h"
#include "Engine/Engine.h"
#include "Engine/Font.h"
#include "Engine/FontFace.h"
#include "Engine/GameViewportClient.h"
#include "Engine/World.h"
#include "ShowFlags.h"
#include "EngineUtils.h"
#include "Fonts/CompositeFont.h"
#include "Fonts/FontMeasure.h"
#include "Fonts/SlateFontInfo.h"
#include "Framework/Application/SlateApplication.h"
#include "GameFramework/PlayerController.h"
#include "GlobalRenderResources.h"
#include "InputCoreTypes.h"
#include "Kismet/KismetMaterialLibrary.h"
#include "Kismet/KismetSystemLibrary.h"
#include "Materials/MaterialParameterCollection.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "ProceduralMeshComponent.h"
#include "Rendering/SlateRenderer.h"
#include "UnrealClient.h"

#if AETHYR_HAS_RULES
extern "C" {
#include "aethyr_host.h"
int aethyr_script_run(const char* Text, size_t Length, void (*Line)(const char*));
}
#if __has_include("aethyr_run_script.h")
#include "aethyr_run_script.h"
#define AETHYR_HAS_SCRIPT 1
#else
#define AETHYR_HAS_SCRIPT 0
#endif
#endif

DEFINE_LOG_CATEGORY_STATIC(LogAethyr, Log, All);

namespace
{
	const float Units = 100.0f;   // Unreal centimetres a metre

	// Aethyr metres, y up, right-handed -> Unreal centimetres, z up (measured: GitHub #13399)
	FVector ToUnreal(float X, float Y, float Z) { return FVector(X * Units, Z * Units, Y * Units); }
	FVector DirToUnreal(float X, float Y, float Z) { return FVector(X, Z, Y); }

	float SrgbToLinear(float C) { return C <= 0.04045f ? C / 12.92f : FMath::Pow((C + 0.055f) / 1.055f, 2.4f); }

	// The graybox rule (aethyr/render/graybox.coax): how much of a colour's hue is kept. Orange to
	// yellow from `Warm` saturation up, cyan from `Cyan` up, everything else none.
	float GrayKeep(float R, float G, float B, float Warm, float Cyan)
	{
		const float Mx = FMath::Max3(R, G, B), Mn = FMath::Min3(R, G, B), Span = Mx - Mn;
		if (Span < 0.0001f) { return 0.0f; }
		const float Sat = Span / Mx;
		float Lo = 1.0f;
		if (B <= G && R >= G * 0.85f && (G - B) > Span * 0.15f) { Lo = Warm; }
		const float Gb = FMath::Min(G, B);
		if (R <= Gb && (Gb - R) > Span * 0.6f) { Lo = Cyan; }
		const float T = FMath::Clamp((Sat - Lo) / 0.1f, 0.0f, 1.0f);
		return T * T * (3.0f - 2.0f * T);
	}
	void Gray(float& R, float& G, float& B, float Warm, float Cyan)
	{
		const float K = GrayKeep(R, G, B, Warm, Cyan), L = 0.2126f * R + 0.7152f * G + 0.0722f * B;
		R = FMath::Lerp(L, R, K); G = FMath::Lerp(L, G, K); B = FMath::Lerp(L, B, K);
	}

	void ScriptLine(const char* Text) { UE_LOG(LogAethyr, Display, TEXT("%hs"), Text); }

	struct FKeyCode { FKey Key; int32 Code; };
	const TArray<FKeyCode>& KeyCodes()
	{
		static const TArray<FKeyCode> Codes = {
			{EKeys::W, 'w'}, {EKeys::A, 'a'}, {EKeys::S, 's'}, {EKeys::D, 'd'}, {EKeys::E, 'e'}, {EKeys::F, 'f'}, {EKeys::G, 'g'},
			{EKeys::C, 'c'}, {EKeys::X, 'x'}, {EKeys::R, 'r'}, {EKeys::One, '1'}, {EKeys::Two, '2'}, {EKeys::Three, '3'},
			{EKeys::Four, '4'}, {EKeys::Five, '5'}, {EKeys::Six, '6'}, {EKeys::SpaceBar, ' '}, {EKeys::Enter, -2},
			{EKeys::Left, -4}, {EKeys::Right, -5}, {EKeys::Up, -6}, {EKeys::Down, -7}, {EKeys::Tab, -15}};
		return Codes;
	}
}

AAethyrGame::AAethyrGame()
{
	PrimaryActorTick.bCanEverTick = true;
	RootComponent = CreateDefaultSubobject<USceneComponent>(TEXT("Root"));
	Live = CreateDefaultSubobject<UProceduralMeshComponent>(TEXT("Live"));
	Live->SetupAttachment(RootComponent);
	Live->SetCollisionEnabled(ECollisionEnabled::NoCollision);
	Live->bUseAsyncCooking = false;
	Camera = CreateDefaultSubobject<UCameraComponent>(TEXT("Camera"));
	Camera->SetupAttachment(RootComponent);
	Camera->bConstrainAspectRatio = false;
	Sky = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("Sky"));
	Sky->SetupAttachment(RootComponent);
	Sky->SetCollisionEnabled(ECollisionEnabled::NoCollision);
	Sky->SetCastShadow(false);
	Sky->SetUsingAbsoluteScale(true);
	Sky->SetWorldScale3D(FVector(3000.0));   // the engine's sphere is 50 units in radius: 1.5 km
}

void AAethyrGame::BeginPlay()
{
	Super::BeginPlay();
#if AETHYR_HAS_RULES
	const TCHAR* Args = FCommandLine::Get();
	// The scripted run: the same C that makes the bundle's expected lines, run here.
	if (FParse::Param(Args, TEXT("AethyrScript")))
	{
#if AETHYR_HAS_SCRIPT
		const int Result = aethyr_script_run(AETHYR_RUN_SCRIPT, sizeof(AETHYR_RUN_SCRIPT) - 1, &ScriptLine);
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_SCRIPT result %d"), Result);
#else
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_SCRIPT FAILED: no script was exported"));
#endif
		if (FParse::Param(Args, TEXT("AethyrExit")))
		{
			UKismetSystemLibrary::QuitGame(this, nullptr, EQuitPreference::Quit, false);
			return;
		}
	}
	State = FMemory::MallocZeroed(aethyr_state_size(), 16);
	aethyr_state_init(State);
	VertexFloats = (int32)aethyr_vertex_floats();
	Vertices.SetNumZeroed((int32)aethyr_max_vertices() * VertexFloats);
	Uniforms.SetNumZeroed((int32)aethyr_uniform_floats());
	if (FParse::Param(Args, TEXT("AethyrGray"))) { const char* Gray = "?gray"; aethyr_message(State, 600, Gray, 5); }
	int32 Stage = -1;
	if (FParse::Value(Args, TEXT("AethyrStage="), Stage) && Stage >= 0)
	{
		// one frame first: a stage stands in for a moment of a run that has started
		aethyr_frame(State, Vertices.GetData(), Uniforms.GetData(), 1280, 800, 1, 0.02f, 0);
		const FString Text = FString::FromInt(Stage);
		const FTCHARToUTF8 Body(*Text);
		aethyr_message(State, 610, Body.Get(), Body.Length());
	}
	if (FParse::Value(Args, TEXT("AethyrShot="), ShotPath)) { ShotAfter = 20; FParse::Value(Args, TEXT("AethyrShotAfter="), ShotAfter); bExitAfterShot = true; }
#endif
	if (APlayerController* Player = GetWorld()->GetFirstPlayerController())
	{
		Player->SetViewTarget(this);
		Player->bShowMouseCursor = true;
	}
	// The world's materials write the picture's final colours themselves (the cel look's tone is
	// part of the look), so Unreal's own tone curve, exposure and lens effects are off.
	if (GEngine && GEngine->GameViewport)
	{
		FEngineShowFlags& Flags = GEngine->GameViewport->EngineShowFlags;
		Flags.SetTonemapper(false);
		Flags.SetEyeAdaptation(false);
		Flags.SetBloom(false);
		Flags.SetMotionBlur(false);
		Flags.SetAmbientOcclusion(false);
		Flags.SetVignette(false);
		Flags.SetGrain(false);
		Flags.SetLensFlares(false);
		Flags.SetAntiAliasing(!FParse::Param(FCommandLine::Get(), TEXT("AethyrNoAA")));
	}
	bInput = !FParse::Param(FCommandLine::Get(), TEXT("AethyrNoInput"));
	if (LiveMaterial) { Live->SetMaterial(0, LiveMaterial); }
}

void AAethyrGame::EndPlay(const EEndPlayReason::Type Reason)
{
	if (State) { FMemory::Free(State); State = nullptr; }
	Super::EndPlay(Reason);
}

void AAethyrGame::SendKeys(APlayerController* Player)
{
#if AETHYR_HAS_RULES
	for (const FKeyCode& K : KeyCodes())
	{
		if (Player->WasInputKeyJustPressed(K.Key)) { aethyr_key(State, K.Code, 1); }
		if (Player->WasInputKeyJustReleased(K.Key)) { aethyr_key(State, K.Code, 0); }
	}
	// the world asks for the mouse while you walk and gives it back in its panels
	const bool bHold = aethyr_mouselook(State) == 2;
	if (bHold != bMouseHeld)
	{
		bMouseHeld = bHold;
		Player->bShowMouseCursor = !bHold;
		if (bHold) { Player->SetInputMode(FInputModeGameOnly()); } else { Player->SetInputMode(FInputModeGameAndUI().SetHideCursorDuringCapture(false)); }
		aethyr_pointer_lock(State, bHold ? 1 : 0);
	}
	if (bHold)
	{
		float Dx = 0, Dy = 0;
		Player->GetInputMouseDelta(Dx, Dy);
		if (Dx != 0 || Dy != 0) { aethyr_look_delta(State, Dx * 8.0f, -Dy * 8.0f); }   // Unreal's delta is up-positive and in its own small units
	}
	float X = 0, Y = 0;
	if (Player->GetMousePosition(X, Y))
	{
		const bool bDown = Player->IsInputKeyDown(EKeys::LeftMouseButton);
		if (bDown && !bPointerDown) { aethyr_pointer(State, X, Y, 0); }
		else if (!bDown && bPointerDown) { aethyr_pointer(State, X, Y, 2); }
		else { aethyr_pointer(State, X, Y, 1); }
		bPointerDown = bDown;
	}
#endif
}

void AAethyrGame::UpdateLive()
{
#if AETHYR_HAS_RULES
	const uint32 Version = aethyr_store_version(State, 1);
	if (Version == LiveVersion) { return; }
	LiveVersion = Version;
	const int32 Count = (int32)(aethyr_store_bytes(State, 1) / 24);
	const uint32* W = aethyr_store(State, 1);
	TArray<FVector> Positions; TArray<int32> Triangles; TArray<FVector> Normals; TArray<FVector2D> Uv0; TArray<FVector2D> Uv1; TArray<FLinearColor> Colours;
	Positions.SetNumUninitialized(Count); Triangles.SetNumUninitialized(Count); Normals.SetNumUninitialized(Count);
	Uv0.SetNumUninitialized(Count); Uv1.SetNumUninitialized(Count); Colours.SetNumUninitialized(Count);
	for (int32 I = 0; I < Count; I++)
	{
		const uint32* V = W + I * 6;
		float P[3]; FMemory::Memcpy(P, V, 12);
		float Param; FMemory::Memcpy(&Param, V + 5, 4);
		Positions[I] = ToUnreal(P[0], P[1], P[2]);
		const uint32 N = V[3], C = V[4];
		Normals[I] = DirToUnreal((N & 255) / 255.0f * 2 - 1, ((N >> 8) & 255) / 255.0f * 2 - 1, ((N >> 16) & 255) / 255.0f * 2 - 1).GetSafeNormal();
		Colours[I] = FLinearColor(SrgbToLinear((C & 255) / 255.0f), SrgbToLinear(((C >> 8) & 255) / 255.0f), SrgbToLinear(((C >> 16) & 255) / 255.0f), 1.0f);
		Uv0[I] = FVector2D(Param, (float)(N >> 24));
		Uv1[I] = FVector2D((float)(C >> 24), 0.0f);
	}
	// the other hand: a triangle's last two corners swap
	for (int32 T = 0; T + 2 < Count; T += 3) { Triangles[T] = T; Triangles[T + 1] = T + 2; Triangles[T + 2] = T + 1; }
	Live->CreateMeshSection_LinearColor(0, Positions, Triangles, Normals, Uv0, Uv1, TArray<FVector2D>(), TArray<FVector2D>(), Colours, TArray<FProcMeshTangent>(), false, false);
	if (LiveMaterial) { Live->SetMaterial(0, LiveMaterial); }
#endif
}

void AAethyrGame::UpdateCamera()
{
	if (Uniforms.Num() < FMath::Max3(EyeAt, ForwardAt, UpAt) + 4) { return; }
	const float* U = Uniforms.GetData();
	const FVector Eye = ToUnreal(U[EyeAt], U[EyeAt + 1], U[EyeAt + 2]);
	const FVector Forward = DirToUnreal(U[ForwardAt], U[ForwardAt + 1], U[ForwardAt + 2]);
	const FVector Up = DirToUnreal(U[UpAt], U[UpAt + 1], U[UpAt + 2]);
	if (Forward.IsNearlyZero()) { return; }
	Camera->SetWorldLocationAndRotation(Eye, FRotationMatrix::MakeFromXZ(Forward, Up).Rotator());
	Sky->SetWorldLocation(Eye);
	// the world's angle is vertical; Unreal's is horizontal
	const float Tan = U[EyeAt + 3], Aspect = U[ForwardAt + 3];
	if (Tan > 0 && Aspect > 0) { Camera->SetFieldOfView(FMath::RadiansToDegrees(2.0f * FMath::Atan(Tan * Aspect))); }
}

void AAethyrGame::UpdateLook()
{
	if (!Look) { return; }
	for (const FCollectionVectorParameter& P : Look->VectorParameters)
	{
		const FString Name = P.ParameterName.ToString();
		if (!Name.StartsWith(TEXT("U"))) { continue; }
		const int32 At = FCString::Atoi(*Name + 1);
		if (At >= 0 && At + 4 <= Uniforms.Num())
		{
			UKismetMaterialLibrary::SetVectorParameterValue(this, Look, P.ParameterName, FLinearColor(Uniforms[At], Uniforms[At + 1], Uniforms[At + 2], Uniforms[At + 3]));
		}
	}
}

void AAethyrGame::Tick(float DeltaSeconds)
{
	Super::Tick(DeltaSeconds);
#if AETHYR_HAS_RULES
	if (!State) { return; }
	APlayerController* Player = GetWorld()->GetFirstPlayerController();
	if (Player && bInput) { SendKeys(Player); }
	FVector2D Size(1280, 800);
	if (GEngine && GEngine->GameViewport) { GEngine->GameViewport->GetViewportSize(Size); }
	if (Size.X < 1 || Size.Y < 1) { Size = FVector2D(1280, 800); }
	const double Before = FPlatformTime::Seconds();
	OverlayCount = (int32)aethyr_frame(State, Vertices.GetData(), Uniforms.GetData(), Size.X, Size.Y, 1, FMath::Min(DeltaSeconds, 0.05f), 0);
	UpdateLive();
	UpdateCamera();
	UpdateLook();
	HostSeconds += FPlatformTime::Seconds() - Before;
	FrameSeconds += DeltaSeconds;
	FrameCount++;
	if (FrameSeconds >= 1.0)
	{
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_STATE mode %d zone %d phase %d hearts %d gray %d live %d overlay %d frame_ms %.2f host_ms %.2f"),
			aethyr_fact(State, 0), aethyr_fact(State, 1), aethyr_fact(State, 2), aethyr_fact(State, 3), aethyr_fact(State, 10),
			(int32)(aethyr_store_bytes(State, 1) / 24), OverlayCount, FrameSeconds / FrameCount * 1000.0, HostSeconds / FrameCount * 1000.0);
		FrameSeconds = 0; HostSeconds = 0; FrameCount = 0;
	}
	if (ShotAfter >= 0 && --ShotAfter < 0)
	{
		if (ShotPath.IsEmpty()) { if (bExitAfterShot) { UKismetSystemLibrary::QuitGame(this, nullptr, EQuitPreference::Quit, false); } }
		else { FScreenshotRequest::RequestScreenshot(ShotPath, true, false); ShotPath.Empty(); ShotAfter = 10; }
	}
#endif
}

// ---- the overlay

void AAethyrHUD::DrawHUD()
{
	Super::DrawHUD();
	AAethyrGame* Game = nullptr;
	for (TActorIterator<AAethyrGame> It(GetWorld()); It; ++It) { Game = *It; break; }
	if (!Game || !Canvas) { return; }
	const TArray<float>& U = Game->UniformBlock();
	const bool bGray = Game->FxAt + 2 < U.Num() && U[Game->FxAt + 2] < -0.5f;
	// panels: flat rectangles under everything (x0, y0, x1, y1, then colour and alpha)
	for (int32 P = 0; P < Game->PanelCount; P++)
	{
		const int32 At = Game->PanelsAt + P * 8;
		if (At + 8 > U.Num() || U[At + 7] <= 0) { continue; }
		float Pr = U[At + 4], Pg = U[At + 5], Pb = U[At + 6];
		if (bGray) { Gray(Pr, Pg, Pb, 0.62f, 0.38f); }
		FCanvasTileItem Tile(FVector2D(U[At], U[At + 1]), FVector2D(U[At + 2] - U[At], U[At + 3] - U[At + 1]), FLinearColor(SrgbToLinear(Pr), SrgbToLinear(Pg), SrgbToLinear(Pb), U[At + 7]));
		Tile.BlendMode = SE_BLEND_Translucent;
		Canvas->DrawItem(Tile);
	}
	const int32 F = Game->OverlayFloats();
	const float* V = Game->Overlay().GetData();
	const int32 Quads = F >= 12 ? Game->OverlayVertices() / 6 : 0;
	auto FaceFont = [this](const UFontFace* Face)
	{
		// a runtime font holding the one face: what a Font asset made from it would be
		UFont* Font = NewObject<UFont>(this);
		Font->FontCacheType = EFontCacheType::Runtime;
		FTypefaceEntry Entry(TEXT("Regular"));
		Entry.Font = FFontData(Face);
		Font->GetMutableInternalCompositeFont().DefaultTypeface.Fonts.Add(Entry);
		return Font;
	};
	if (!TextFont && Game->TextFace) { TextFont = FaceFont(Game->TextFace); }
	if (!TitleFont && Game->TitleFace) { TitleFont = FaceFont(Game->TitleFace); }
	TArray<FCanvasUVTri> Tris;
	auto Flush = [&]()
	{
		if (Tris.Num() == 0) { return; }
		FCanvasTriangleItem Item(Tris, GWhiteTexture);
		Item.BlendMode = SE_BLEND_Translucent;
		Canvas->DrawItem(Item);
		Tris.Reset();
	};
	auto Tri = [&](const FVector2D& A, const FVector2D& B, const FVector2D& C, const FLinearColor& Colour)
	{
		FCanvasUVTri T;
		T.V0_Pos = A; T.V1_Pos = B; T.V2_Pos = C;
		T.V0_UV = T.V1_UV = T.V2_UV = FVector2D(0, 0);
		T.V0_Color = T.V1_Color = T.V2_Color = Colour;
		Tris.Add(T);
	};
	static const uint32 Extra[16] = {183, 8211, 8212, 8216, 8217, 8220, 8221, 8226, 8230, 8594, 8383, 8722, 215, 8804, 8805, 8800};   // aethyr/render/sans-profile.coax
	for (int32 Q = 0; Q < Quads; Q++)
	{
		const float* A = V + Q * 6 * F;
		const float Mode = A[11];
		float Tr = A[8], Tg = A[9], Tb = A[10];
		if (bGray) { Gray(Tr, Tg, Tb, 0.62f, 0.38f); }
		const FLinearColor Colour(SrgbToLinear(Tr), SrgbToLinear(Tg), SrgbToLinear(Tb), FMath::Min(A[6], 1.0f));
		if (Mode > 3.5f && Mode < 5.5f)
		{
			// a glyph: its cell in the atlas says which character; draw that character in the face
			Flush();
			const bool bTitle = Mode > 4.5f;
			const UFont* Face = bTitle ? TitleFont : TextFont;
			if (!Face || !FSlateApplication::IsInitialized()) { continue; }
			const int32 Cell = FMath::RoundToInt(A[2] * 16.0f) + FMath::RoundToInt(A[3] * 7.0f) * 16;
			const uint32 Code = Cell < 95 ? 32 + Cell : (Cell - 95 < 16 ? Extra[Cell - 95] : '?');
			const float S = A[4] / 64.0f;
			const float Em = A[4] * (bTitle ? 0.7f : 1.0f);
			const FSlateFontInfo Font(Face, Em * 0.75f);
			const TSharedRef<FSlateFontMeasure> Measure = FSlateApplication::Get().GetRenderer()->GetFontMeasureService();
			const float Top = (A[1] + 72.0f * S) - (Measure->GetMaxCharacterHeight(Font) + Measure->GetBaseline(Font));
			FCanvasTextItem Text(FVector2D(A[0] + 16.0f * S, Top), FText::FromString(FString::Chr((TCHAR)Code)), Font, Colour);
			Text.BlendMode = SE_BLEND_Translucent;
			Canvas->DrawItem(Text);
			continue;
		}
		// a stroke: a segment from 0 to its length along its own x, drawn `core` thick with round
		// ends. The corners carry (along, across), which gives the segment's place and direction.
		const float* B = A + F; const float* C = A + 2 * F;
		const FVector2D P0(A[0], A[1]), P1(B[0], B[1]), P2(C[0], C[1]);
		const FVector2D L0(A[2], A[3]), L1(B[2], B[3]), L2(C[2], C[3]);
		const float Det = (L1.X - L0.X) * (L2.Y - L0.Y) - (L2.X - L0.X) * (L1.Y - L0.Y);
		if (FMath::Abs(Det) < 1e-6f) { continue; }
		const FVector2D AxisX = ((P1 - P0) * (L2.Y - L0.Y) - (P2 - P0) * (L1.Y - L0.Y)) / Det;
		const FVector2D AxisY = ((P2 - P0) * (L1.X - L0.X) - (P1 - P0) * (L2.X - L0.X)) / Det;
		const FVector2D Origin = P0 - AxisX * L0.X - AxisY * L0.Y;
		const float Length = A[4], Core = A[5];
		const FVector2D Start = Origin, End = Origin + AxisX * Length;
		const FVector2D Side = AxisY * Core, Along = AxisX * Core;
		Tri(Start - Side, End - Side, End + Side, Colour);
		Tri(Start - Side, End + Side, Start + Side, Colour);
		const int32 Steps = Core > 3.0f ? 8 : 4;
		for (int32 K = 0; K < Steps; K++)
		{
			const float T0 = PI * K / Steps - HALF_PI, T1 = PI * (K + 1) / Steps - HALF_PI;
			Tri(End, End + Along * FMath::Cos(T0) + Side * FMath::Sin(T0), End + Along * FMath::Cos(T1) + Side * FMath::Sin(T1), Colour);
			Tri(Start, Start - Along * FMath::Cos(T0) + Side * FMath::Sin(T0), Start - Along * FMath::Cos(T1) + Side * FMath::Sin(T1), Colour);
		}
	}
	Flush();
}

AAethyrGameMode::AAethyrGameMode()
{
	DefaultPawnClass = nullptr;
	HUDClass = AAethyrHUD::StaticClass();
}
