#include "AethyrGame.h"
#include "AethyrManifest.h"

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
#include "HttpModule.h"
#include "Interfaces/IHttpRequest.h"
#include "Interfaces/IHttpResponse.h"
#include "InputCoreTypes.h"
#include "Kismet/GameplayStatics.h"
#include "Kismet/KismetMaterialLibrary.h"
#include "Sound/SoundBase.h"
#include "Engine/StaticMesh.h"
#include "Engine/SkeletalMesh.h"
#include "Components/SkeletalMeshComponent.h"
#include "Animation/AnimSequenceBase.h"
#include "Components/LocalLightComponent.h"
#include "Components/LightComponent.h"
#include "Components/SceneCaptureComponent2D.h"
#include "Engine/Level.h"
#include "Engine/LevelStreamingDynamic.h"
#include "Engine/TextureRenderTarget2D.h"
#include "RenderingThread.h"
#include "RHICommandList.h"
#include "TextureResource.h"
#include "Engine/DataTable.h"
#include "Kismet/KismetSystemLibrary.h"
#include "Materials/MaterialParameterCollection.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "ProceduralMeshComponent.h"
#include "Rendering/SlateRenderer.h"
#include "UnrealClient.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"

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

	// Metaverse Engine metres, y up, right-handed -> Unreal centimetres, z up (measured: GitHub #13399)
	FVector ToUnreal(float X, float Y, float Z) { return FVector(X * Units, Z * Units, Y * Units); }
	FVector DirToUnreal(float X, float Y, float Z) { return FVector(X, Z, Y); }

	float SrgbToLinear(float C) { return C <= 0.04045f ? C / 12.92f : FMath::Pow((C + 0.055f) / 1.055f, 2.4f); }

	// The graybox rule (metaverse-engine/render/graybox.coax): how much of a colour's hue is kept. Orange to
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
			{EKeys::Left, -4}, {EKeys::Right, -5}, {EKeys::Up, -6}, {EKeys::Down, -7}, {EKeys::Tab, -15},
			// the slice's key map (metaverse/splice-bots/README.md, Controls), coded as the browser host codes them (coax/web/host.js KEYS)
			{EKeys::Q, 'q'}, {EKeys::I, 'i'}, {EKeys::M, 'm'}, {EKeys::Escape, -1}, {EKeys::BackSpace, -3},
			{EKeys::LeftShift, -13}, {EKeys::RightShift, -13}, {EKeys::LeftControl, -14}, {EKeys::RightControl, -14}};
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
	// the angle the host sets is the horizontal one, whatever the window's shape
	Camera->bOverrideAspectRatioAxisConstraint = true;
	Camera->AspectRatioAxisConstraint = AspectRatio_MaintainXFOV;
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
	// -AethyrWorld=URL: the page's address (the browser host's kind 600), so the world follows its saved
	// edit layer from that site as the page does: e.g. http://127.0.0.1:8800/worlds/splice-bots
	FString WorldUrl;
	if (FParse::Value(Args, TEXT("AethyrWorld="), WorldUrl, false) && WorldUrl.Contains(TEXT("://")))
	{
		const int32 Host = WorldUrl.Find(TEXT("://")) + 3;
		int32 Path = WorldUrl.Find(TEXT("/"), ESearchCase::CaseSensitive, ESearchDir::FromStart, Host);
		WorldOrigin = Path < 0 ? WorldUrl : WorldUrl.Left(Path);
		const FTCHARToUTF8 Body(*WorldUrl);
		aethyr_message(State, 600, Body.Get(), Body.Length());
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_WORLD follows %s"), *WorldUrl);
	}
	int32 Stage = -1;
	if (FParse::Value(Args, TEXT("AethyrStage="), Stage) && Stage >= 0)
	{
		// one frame first: a stage stands in for a moment of a run that has started
		aethyr_frame(State, Vertices.GetData(), Uniforms.GetData(), 1280, 800, 1, 0.02f, 0);
		const FString Text = FString::FromInt(Stage);
		const FTCHARToUTF8 Body(*Text);
		aethyr_message(State, 640, Body.Get(), Body.Length());
	}
	// -AethyrPlay: the exported run, played on this world so its things, sounds and screen are the game's own
	if (FParse::Param(Args, TEXT("AethyrPlay")))
	{
#if AETHYR_HAS_SCRIPT
		FString(UTF8_TO_TCHAR(AETHYR_RUN_SCRIPT)).ParseIntoArrayLines(PlayLines, false);
		bPlaying = true;
		FParse::Value(Args, TEXT("AethyrPlaySpeed="), PlaySpeed);
		PlaySpeed = FMath::Clamp(PlaySpeed, 1, 64);
		bExitAfterPlay = FParse::Param(Args, TEXT("AethyrExit"));
#endif
	}
	// -AethyrPlayFile=FILE: a script read from a file, played on screen as -AethyrPlay plays the bundle's
	// (keys, messages and frames; a look at one moment or one movement, with no rebuild)
	FString PlayFile;
	if (FParse::Value(Args, TEXT("AethyrPlayFile="), PlayFile) && FFileHelper::LoadFileToStringArray(PlayLines, *PlayFile))
	{
		bPlaying = true; PlaySpeed = 1;
		bExitAfterPlay = FParse::Param(Args, TEXT("AethyrExit"));
	}
	// -AethyrShots=DIR -AethyrShotEvery=N: a numbered picture every N frames (a film's frames)
	if (FParse::Value(Args, TEXT("AethyrShots="), ShotsDir)) { ShotEvery = 2; FParse::Value(Args, TEXT("AethyrShotEvery="), ShotEvery); ShotEvery = FMath::Max(1, ShotEvery); }
	SetUpThings();
	SetUpLook();
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
		// A look level (the row look.level) owns the tone curve and bloom: its volume sets them.
		Flags.SetTonemapper(LookLevel != nullptr);
		Flags.SetEyeAdaptation(false);
		Flags.SetBloom(LookLevel != nullptr);
		Flags.SetMotionBlur(false);
		Flags.SetAmbientOcclusion(false);
		Flags.SetVignette(false);
		Flags.SetGrain(false);
		Flags.SetLensFlares(false);
		Flags.SetAntiAliasing(!FParse::Param(FCommandLine::Get(), TEXT("AethyrNoAA")));
	}
	// a still (-AethyrShot) ignores input too: an offscreen run still reads the desktop's mouse, which turned the view
	FString StillPath;
	bInput = !FParse::Param(FCommandLine::Get(), TEXT("AethyrNoInput")) && !FParse::Value(FCommandLine::Get(), TEXT("AethyrShot="), StillPath);
	ReadCameraSettings();
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
	// the look (mouse and right stick, AethyrCamera.cpp) and the camera's own key: F5 or V, the right stick's click or the top face button
	if (bHold) { LookInput(Player, GetWorld()->GetDeltaSeconds()); }
	if (Player->WasInputKeyJustPressed(EKeys::F5) || Player->WasInputKeyJustPressed(EKeys::V) ||
		Player->WasInputKeyJustPressed(EKeys::Gamepad_RightThumbstick) || Player->WasInputKeyJustPressed(EKeys::Gamepad_FaceButton_Top))
	{
		CycleCameraMode();
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

// A run of cel-mesh vertices (six words each) as a procedural mesh's one section.
void AAethyrGame::Section(UProceduralMeshComponent* Mesh, const uint32* W, int32 Count) const
{
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
	Mesh->CreateMeshSection_LinearColor(0, Positions, Triangles, Normals, Uv0, Uv1, TArray<FVector2D>(), TArray<FVector2D>(), Colours, TArray<FProcMeshTangent>(), false, false);
	if (LiveMaterial) { Mesh->SetMaterial(0, LiveMaterial); }
}

void AAethyrGame::UpdateLive()
{
#if AETHYR_HAS_RULES
	const uint32 Version = aethyr_store_version(State, 1);
	if (Version == LiveVersion) { return; }
	LiveVersion = Version;
	Section(Live, aethyr_store(State, 1), (int32)(aethyr_store_bytes(State, 1) / 24));
#endif
}

// The instanced draws: each mesh written once in its own space, drawn at each of its rows (turned
// about the world's up, then moved). The browser draws them as GPU instances (a page's scene_vs);
// here each instance is a component holding the mesh, moved to its row.
void AAethyrGame::UpdatePieces()
{
#if AETHYR_HAS_RULES
	uint32 Draws[96 * 6];
	const int32 Count = aethyr_instanced(State, Draws, 96 * 6);
	TMap<uint64, int32> Used;
	PiecesShown = 0;
	for (int32 D = 0; D < Count; D++)
	{
		const uint32* R = Draws + D * 6;
		const int32 MeshStore = (int32)R[0], RowStore = (int32)R[3];
		const uint32 First = R[1], Verts = R[2], Row0 = R[4], Rows = R[5];
		if (Verts < 3 || First >= (1u << 20) || Verts >= (1u << 20)) { continue; }
		if ((uint64)(First + Verts) * 24 > aethyr_store_bytes(State, MeshStore) || (uint64)(Row0 + Rows) * 32 > aethyr_store_bytes(State, RowStore)) { continue; }
		const uint32* Mesh = aethyr_store(State, MeshStore);
		const uint32* RowWords = aethyr_store(State, RowStore);
		const uint32 Version = aethyr_store_version(State, MeshStore);
		const uint64 Key = ((uint64)MeshStore << 40) | ((uint64)First << 20) | Verts;
		TArray<int32>& Pool = PieceByMesh.FindOrAdd(Key);
		int32& Next = Used.FindOrAdd(Key);
		for (uint32 Row = Row0; Row < Row0 + Rows; Row++)
		{
			if (Next >= Pool.Num())
			{
				UProceduralMeshComponent* Made = NewObject<UProceduralMeshComponent>(this);
				Made->SetupAttachment(RootComponent);
				Made->SetCollisionEnabled(ECollisionEnabled::NoCollision);
				Made->bUseAsyncCooking = false;
				Made->SetMobility(EComponentMobility::Movable);
				Made->RegisterComponent();
				Pool.Add(PiecePool.Add(Made));
				PieceBuilt.Add(0);
			}
			const int32 At = Pool[Next++];
			UProceduralMeshComponent* C = PiecePool[At];
			if (PieceBuilt[At] != Version || C->GetNumSections() == 0) { Section(C, Mesh + First * 6, (int32)Verts); PieceBuilt[At] = Version; }
			float F[8]; FMemory::Memcpy(F, RowWords + Row * 8, 32);
			// the row's +z goes to (sin, cos): a turn of -atan2(sin, cos) about Unreal's up (as a thing's yaw)
			C->SetWorldLocationAndRotation(ToUnreal(F[0], F[1], F[2]), FRotator(0.0f, -FMath::RadiansToDegrees(FMath::Atan2(F[5], F[4])), 0.0f));
			C->SetVisibility(true);
			PiecesShown++;
		}
	}
	for (TPair<uint64, TArray<int32>>& Pair : PieceByMesh)
	{
		const int32* Shown = Used.Find(Pair.Key);
		for (int32 I = Shown ? *Shown : 0; I < Pair.Value.Num(); I++) { PiecePool[Pair.Value[I]]->SetVisibility(false); }
	}
#endif
}

// The world's one request at a time, sent to the site the page stands for (-AethyrWorld), its
// answer handed back as the browser host hands it: message 512 + id, "STATUS\n" then the body.
void AAethyrGame::UpdateRequests()
{
#if AETHYR_HAS_RULES
	const char* Url = nullptr; const char* Body = nullptr; size_t UrlLength = 0, BodyLength = 0;
	const int Flags = aethyr_request(State, &Url, &UrlLength, &Body, &BodyLength);
	if (Flags == 0) { return; }
	int32 Id = -1;
	for (int32 K = 0; K < 8; K++) { if (!(RequestsOut & (1 << K))) { Id = K; break; } }
	if (WorldOrigin.IsEmpty() || Id < 0) { aethyr_request_sent(State, -1); return; }
	const auto Path = StringCast<TCHAR>(reinterpret_cast<const UTF8CHAR*>(Url), (int32)UrlLength);
	TSharedRef<IHttpRequest, ESPMode::ThreadSafe> Request = FHttpModule::Get().CreateRequest();
	Request->SetURL(WorldOrigin + FString(Path.Length(), Path.Get()));
	Request->SetVerb((Flags & 1) ? TEXT("POST") : TEXT("GET"));
	if (Flags & 1) { Request->SetContent(TArray<uint8>(reinterpret_cast<const uint8*>(Body), (int32)BodyLength)); Request->SetHeader(TEXT("Content-Type"), TEXT("application/json")); }
	const bool bWanted = (Flags & 4) != 0;
	TWeakObjectPtr<AAethyrGame> Self(this);
	Request->OnProcessRequestComplete().BindLambda([Self, Id, bWanted](FHttpRequestPtr, FHttpResponsePtr Response, bool bOk)
	{
		AAethyrGame* Game = Self.Get();
		if (!Game || !Game->State) { return; }
		Game->RequestsOut &= ~(1 << Id);
		if (!bWanted) { return; }
		const int32 Status = (bOk && Response.IsValid()) ? Response->GetResponseCode() : 0;
		TArray<uint8> Text;
		const FTCHARToUTF8 Head(*FString::Printf(TEXT("%d\n"), Status));
		Text.Append(reinterpret_cast<const uint8*>(Head.Get()), Head.Length());
		if (Status != 0) { Text.Append(Response->GetContent()); }
		aethyr_message(Game->State, 512 + Id, reinterpret_cast<const char*>(Text.GetData()), (size_t)Text.Num());
		Game->RequestsAnswered++;
	});
	if (!Request->ProcessRequest()) { aethyr_request_sent(State, -1); return; }
	if (bWanted) { RequestsOut |= 1 << Id; }
	aethyr_request_sent(State, Id);
	RequestsSent++;
#endif
}

void AAethyrGame::UpdateLook()
{
	// the generated collection, and the project's own (look.params): the same U<n> names in both
	for (UMaterialParameterCollection* Collection : {Look.Get(), LookParams.Get()})
	{
		if (!Collection) { continue; }
		for (const FCollectionVectorParameter& P : Collection->VectorParameters)
		{
			const FString Name = P.ParameterName.ToString();
			if (!Name.StartsWith(TEXT("U")) || Name.Len() < 2 || !FChar::IsDigit(Name[1])) { continue; }
			const int32 At = FCString::Atoi(*Name + 1);
			if (At >= 0 && At + 4 <= Uniforms.Num())
			{
				UKismetMaterialLibrary::SetVectorParameterValue(this, Collection, P.ParameterName, FLinearColor(Uniforms[At], Uniforms[At + 1], Uniforms[At + 2], Uniforms[At + 3]));
			}
		}
	}
	UpdateLookWorld();
}

// ---- the look a project owns. Three rows of the overrides table, each optional:
//   look.level   a level, loaded beside the generated one: its volumes, fog and lights are the project's.
//                With it the tone curve and bloom stay on, for its post-process volume to set.
//                In it, an actor tagged AethyrSun is turned to the key light and takes its colour; one
//                tagged AethyrLight0 to AethyrLight7 is kept at that light of the game's, with its
//                colour and reach, its own brightness times the light's strength.
//   look.params  a material parameter collection. Each vector named U<n> receives the uniform block's
//                floats n to n + 3, as the generated collection's do, and these when it has them:
//                AeLook (1, the clock 0 to 1, seconds, 0), AeSun (toward the key light, Unreal's axes; w 1 for the moon),
//                AeShadowU, AeShadowV, AeShadowD (a place's u, v and depth in cm in the shadow picture:
//                dot(xyz, place) + w) and AeShadowInfo (1 when the picture is drawn, a texel, its width in cm).
//                AeShadowSet, when it has it, is read once: the shadow picture's width and depth in metres.
//   look.shadow  a render target (one float channel): the scene's depth seen from the key light, a
//                square ShadowWidth wide ahead of the camera, drawn each frame.
void AAethyrGame::SetUpLook()
{
	if (!Overrides || FParse::Param(FCommandLine::Get(), TEXT("AethyrNoLook"))) { return; }
	LookParams = Cast<UMaterialParameterCollection>(OverrideFor(TEXT("look.params")));
	if (LookParams)
	{
		for (const FCollectionVectorParameter& P : LookParams->VectorParameters)
		{
			LookNames.Add(P.ParameterName);
			// AeShadowSet: the shadow picture's width and depth in metres, set in the collection
			if (P.ParameterName == FName(TEXT("AeShadowSet")) && P.DefaultValue.R > 0.0f && P.DefaultValue.G > 0.0f) { ShadowWidth = P.DefaultValue.R * Units; ShadowReach = P.DefaultValue.G * Units; }
		}
	}
	if (const FAethyrOverrideRow* Row = Overrides->FindRow<FAethyrOverrideRow>(FName(TEXT("look.level")), TEXT("Aethyr"), false))
	{
		bool bOk = false;
		LookLevel = ULevelStreamingDynamic::LoadLevelInstanceBySoftObjectPtr(this, TSoftObjectPtr<UWorld>(Row->Asset.ToSoftObjectPath()), FVector::ZeroVector, FRotator::ZeroRotator, bOk);
		if (LookLevel) { LookLevel->bShouldBlockOnLoad = true; }
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_LOOK level %s %s"), *Row->Asset.ToString(), LookLevel ? TEXT("loading") : TEXT("FAILED"));
	}
	ShadowTarget = Cast<UTextureRenderTarget2D>(OverrideFor(TEXT("look.shadow")));
	if (ShadowTarget)
	{
		ShadowDepth = NewObject<UTextureRenderTarget2D>(this);
		ShadowDepth->RenderTargetFormat = ShadowTarget->RenderTargetFormat;
		ShadowDepth->ClearColor = FLinearColor(1.0e9f, 1.0e9f, 1.0e9f, 1.0f);
		ShadowDepth->bAutoGenerateMips = false;
		ShadowDepth->InitAutoFormat(ShadowTarget->SizeX, ShadowTarget->SizeY);
		ShadowDepth->UpdateResourceImmediate(true);
		ShadowCapture = NewObject<USceneCaptureComponent2D>(this);
		ShadowCapture->SetupAttachment(RootComponent);
		ShadowCapture->SetUsingAbsoluteLocation(true);
		ShadowCapture->SetUsingAbsoluteRotation(true);
		ShadowCapture->ProjectionType = ECameraProjectionMode::Orthographic;
		ShadowCapture->OrthoWidth = ShadowWidth;
		ShadowCapture->CaptureSource = ESceneCaptureSource::SCS_SceneDepth;
		ShadowCapture->TextureTarget = ShadowDepth;
		ShadowCapture->bCaptureEveryFrame = false;
		ShadowCapture->bCaptureOnMovement = false;
		ShadowCapture->bAlwaysPersistRenderingState = true;
		ShadowCapture->HiddenComponents.Add(Sky);
		FEngineShowFlags& F = ShadowCapture->ShowFlags;
		F.SetLighting(false); F.SetPostProcessing(false); F.SetFog(false); F.SetAtmosphere(false); F.SetSkyLighting(false);
		F.SetDynamicShadows(false); F.SetAntiAliasing(false); F.SetBloom(false); F.SetAmbientOcclusion(false); F.SetTranslucency(false);
		F.SetMotionBlur(false); F.SetEyeAdaptation(false); F.SetVolumetricFog(false); F.SetCloud(false); F.SetDecals(false);
		ShadowCapture->RegisterComponent();
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_LOOK shadow %s %dx%d, %.0f m wide"), *ShadowTarget->GetName(), ShadowTarget->SizeX, ShadowTarget->SizeY, ShadowWidth / Units);
	}
	UE_LOG(LogAethyr, Display, TEXT("AETHYR_LOOK params %s"), LookParams ? *LookParams->GetName() : TEXT("none"));
}

void AAethyrGame::LookVector(FName Name, const FLinearColor& Value)
{
	if (LookParams && LookNames.Contains(Name)) { UKismetMaterialLibrary::SetVectorParameterValue(this, LookParams, Name, Value); }
}

void AAethyrGame::UpdateLookWorld()
{
	if (!LookParams && !LookLevel && !ShadowCapture) { return; }
	if (StyleAt < 0 || StyleAt + 128 > Uniforms.Num()) { return; }
	const float* S = Uniforms.GetData() + StyleAt;
	const FVector Sun = DirToUnreal(S[4], S[5], S[6]).GetSafeNormal(UE_SMALL_NUMBER, FVector::UpVector);
	static const FName NLook(TEXT("AeLook")), NSun(TEXT("AeSun")), NU(TEXT("AeShadowU")), NV(TEXT("AeShadowV")), ND(TEXT("AeShadowD")), NInfo(TEXT("AeShadowInfo"));
	LookVector(NLook, FLinearColor(1.0f, S[127], GetWorld()->GetTimeSeconds(), 0.0f));
	LookVector(NSun, FLinearColor(Sun.X, Sun.Y, Sun.Z, S[7]));

	// the look level's actors, found once it is shown
	if (LookLevel && !bLookFound && LookLevel->GetLoadedLevel() && LookLevel->IsLevelVisible())
	{
		bLookFound = true;
		LookLights.SetNum(LightCount); LookLightBase.SetNumZeroed(LightCount);
		static const FName SunTag(TEXT("AethyrSun"));
		for (AActor* A : LookLevel->GetLoadedLevel()->Actors)
		{
			if (!A) { continue; }
			if (A->ActorHasTag(SunTag)) { LookSuns.Add(A); }
			for (int32 K = 0; K < LightCount; K++)
			{
				if (!A->ActorHasTag(FName(*FString::Printf(TEXT("AethyrLight%d"), K)))) { continue; }
				LookLights[K] = A;
				if (const ULightComponent* L = A->FindComponentByClass<ULightComponent>()) { LookLightBase[K] = L->Intensity; }
			}
		}
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_LOOK level shown: %d sun, %d lights"), LookSuns.Num(), LookLights.FilterByPredicate([](const TObjectPtr<AActor>& A) { return A != nullptr; }).Num());
	}
	for (AActor* A : LookSuns)
	{
		if (!A) { continue; }
		A->SetActorRotation((-Sun).Rotation());
		if (ULightComponent* L = A->FindComponentByClass<ULightComponent>()) { L->SetLightColor(FLinearColor(S[0], S[1], S[2])); }
	}
	for (int32 K = 0; K < LookLights.Num(); K++)
	{
		AActor* A = LookLights[K];
		const int32 At = LightsAt + K * 8;
		if (!A || At + 8 > Uniforms.Num()) { continue; }
		const float* L = Uniforms.GetData() + At;
		const bool bOn = L[7] > 0.0f && L[3] > 0.0f;
		A->SetActorHiddenInGame(!bOn);
		ULightComponent* Light = A->FindComponentByClass<ULightComponent>();
		if (Light) { Light->SetVisibility(bOn); }
		if (!bOn) { continue; }
		A->SetActorLocation(ToUnreal(L[0], L[1], L[2]));
		if (Light)
		{
			Light->SetLightColor(FLinearColor(L[4], L[5], L[6]));
			Light->SetIntensity(LookLightBase[K] * L[7]);
			if (ULocalLightComponent* Local = Cast<ULocalLightComponent>(Light)) { Local->SetAttenuationRadius(L[3] * Units); }
		}
	}

	// the key light's depth picture: a square of ground ahead of the camera, its texels still on the ground as the camera moves
	if (ShadowCapture && ShadowTarget && ShadowDepth)
	{
		const FQuat Turn = FRotationMatrix::MakeFromX(-Sun).ToQuat();
		const FVector Fwd = -Sun, Right = Turn.GetRightVector(), Up = Turn.GetUpVector();
		FVector Ahead = Camera->GetForwardVector(); Ahead.Z = 0.0;
		const FVector Focus = Camera->GetComponentLocation() + Ahead.GetSafeNormal() * ShadowWidth * 0.3f;
		const double Texel = ShadowWidth / FMath::Max(ShadowTarget->SizeX, 1);
		const FVector At = Right * FMath::GridSnap(FVector::DotProduct(Focus, Right), Texel) + Up * FMath::GridSnap(FVector::DotProduct(Focus, Up), Texel)
			+ Fwd * (FMath::GridSnap(FVector::DotProduct(Focus, Fwd), 100.0) - ShadowReach * 0.5);
		ShadowCapture->SetWorldLocationAndRotation(At, Turn);
		ShadowCapture->OrthoWidth = ShadowWidth;
		ShadowCapture->CaptureScene();
		// materials read a copy: the picture they read is never the one being drawn
		FTextureRenderTargetResource* From = ShadowDepth->GameThread_GetRenderTargetResource();
		FTextureRenderTargetResource* To = ShadowTarget->GameThread_GetRenderTargetResource();
		if (From && To)
		{
			ENQUEUE_RENDER_COMMAND(AethyrShadowCopy)([From, To](FRHICommandListImmediate& RHICmdList)
			{
				FRHITexture* A = From->GetRenderTargetTexture();
				FRHITexture* B = To->GetRenderTargetTexture();
				if (!A || !B || A->GetDesc().Extent != B->GetDesc().Extent || A->GetFormat() != B->GetFormat()) { return; }
				RHICmdList.Transition({FRHITransitionInfo(A, ERHIAccess::Unknown, ERHIAccess::CopySrc), FRHITransitionInfo(B, ERHIAccess::Unknown, ERHIAccess::CopyDest)});
				RHICmdList.CopyTexture(A, B, FRHICopyTextureInfo());
				RHICmdList.Transition({FRHITransitionInfo(A, ERHIAccess::CopySrc, ERHIAccess::SRVMask), FRHITransitionInfo(B, ERHIAccess::CopyDest, ERHIAccess::SRVMask)});
			});
		}
		const float W = ShadowWidth;
		LookVector(NU, FLinearColor(Right.X / W, Right.Y / W, Right.Z / W, 0.5f - FVector::DotProduct(At, Right) / W));
		LookVector(NV, FLinearColor(-Up.X / W, -Up.Y / W, -Up.Z / W, 0.5f + FVector::DotProduct(At, Up) / W));
		LookVector(ND, FLinearColor(Fwd.X, Fwd.Y, Fwd.Z, -FVector::DotProduct(At, Fwd)));
		LookVector(NInfo, FLinearColor(1.0f, 1.0f / FMath::Max(ShadowTarget->SizeX, 1), W, 0.0f));
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
	if (bPlaying)
	{
		// the script's own frame length, several frames a tick when asked: the run is the same each time
		for (int32 K = 0; K < PlaySpeed && bPlaying; K++)
		{
			if (!StepPlay()) { break; }
			OverlayCount = (int32)aethyr_frame(State, Vertices.GetData(), Uniforms.GetData(), Size.X, Size.Y, 1, PlayDt, 0);
			PlayEvents();
		}
	}
	else
	{
		OverlayCount = (int32)aethyr_frame(State, Vertices.GetData(), Uniforms.GetData(), Size.X, Size.Y, 1, FMath::Min(DeltaSeconds, 0.05f), 0);
		PlayEvents();
	}
	UpdateRequests();
	UpdateThings();
	UpdateLive();
	UpdatePieces();
	UpdateCamera(DeltaSeconds);
	UpdateLook();
	HostSeconds += FPlatformTime::Seconds() - Before;
	FrameSeconds += DeltaSeconds;
	FrameCount++;
	if (FrameSeconds >= 1.0)
	{
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_STATE mode %d zone %d phase %d hearts %d gray %d live %d overlay %d pieces %d requests %d/%d frame_ms %.2f host_ms %.2f"),
			aethyr_fact(State, 0), aethyr_fact(State, 1), aethyr_fact(State, 2), aethyr_fact(State, 3), aethyr_fact(State, 10),
			(int32)(aethyr_store_bytes(State, 1) / 24), OverlayCount, PiecesShown, RequestsAnswered, RequestsSent, FrameSeconds / FrameCount * 1000.0, HostSeconds / FrameCount * 1000.0);
		FrameSeconds = 0; HostSeconds = 0; FrameCount = 0;
	}
	if (ShotEvery > 0 && (++ShotFrame % ShotEvery) == 0)
	{
		FScreenshotRequest::RequestScreenshot(FPaths::Combine(ShotsDir, FString::Printf(TEXT("f%05d.png"), ShotFrame / ShotEvery)), true, false);
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
	static const uint32 Extra[16] = {183, 8211, 8212, 8216, 8217, 8220, 8221, 8226, 8230, 8594, 8383, 8722, 215, 8804, 8805, 8800};   // metaverse-engine/render/sans-profile.coax
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

// ---- things a host draws itself, and sounds

UObject* AAethyrGame::OverrideFor(const FString& Row) const
{
	if (!Overrides) { return nullptr; }
	const FAethyrOverrideRow* Found = Overrides->FindRow<FAethyrOverrideRow>(FName(*Row), TEXT("Aethyr"), false);
	return Found ? Found->Asset.LoadSynchronous() : nullptr;
}

void AAethyrGame::SetUpThings()
{
#if AETHYR_HAS_RULES
	if (!Manifest || !State) { return; }
	// -AethyrNativeThings: every kind with an exported model is drawn with it, as a check of the path
	const bool bAll = FParse::Param(FCommandLine::Get(), TEXT("AethyrNativeThings"));
	FParse::Value(FCommandLine::Get(), TEXT("AethyrThingBeside="), ThingBeside);
	// the clips: every "anim." row that is an animation, by its row's name
	if (Overrides)
	{
		for (const FName& Row : Overrides->GetRowNames())
		{
			const FString Key = Row.ToString();
			if (!Key.StartsWith(TEXT("anim."))) { continue; }
			if (UAnimSequenceBase* Clip = Cast<UAnimSequenceBase>(OverrideFor(Key))) { Clips.Add(Key, Clip); }
		}
	}
	for (int32 Kind = 0; Kind < Manifest->ThingKinds.Num(); Kind++)
	{
		const FString Name = Manifest->ThingKinds[Kind].ToString();
		UObject* Asset = OverrideFor(TEXT("thing.") + Name);
		UStaticMesh* Whole = Cast<UStaticMesh>(Asset);
		bool bNative = false;
		if (USkeletalMesh* Skin = Cast<USkeletalMesh>(Asset))
		{
			// a skeletal mesh stands for every variant of its kind: the variant picks its clip
			NativeSkins.Add(Kind, Skin);
			NativeKinds.Add(Kind);
			if (ThingBeside == 0.0f) { aethyr_thing_hide(State, Kind, 1); }
			int32 Mine = 0;
			for (const TPair<FString, TObjectPtr<UAnimSequenceBase>>& Clip : Clips) { if (Clip.Key.StartsWith(TEXT("anim.") + Name)) { Mine++; } }
			UE_LOG(LogAethyr, Display, TEXT("AETHYR_THINGS kind %s is drawn by Unreal with the skeletal mesh %s and %d clips"), *Name, *Skin->GetName(), Mine);
			continue;
		}
		for (int32 Variant = 0; Variant < 16; Variant++)
		{
			const FString Key = FString::Printf(TEXT("%s_v%d"), *Name, Variant);
			UStaticMesh* Mesh = Cast<UStaticMesh>(OverrideFor(TEXT("thing.") + Key));
			if (!Mesh) { Mesh = Whole; }
			if (!Mesh && bAll) { if (const TObjectPtr<UStaticMesh>* Baked = Manifest->ThingMeshes.Find(FName(*Key))) { Mesh = *Baked; } }
			if (Mesh) { NativeMeshes.Add(Kind * 16 + Variant, Mesh); bNative = true; }
		}
		if (bNative)
		{
			NativeKinds.Add(Kind);
			if (ThingBeside == 0.0f) { aethyr_thing_hide(State, Kind, 1); }
			UE_LOG(LogAethyr, Display, TEXT("AETHYR_THINGS kind %s is drawn by Unreal%s"), *Name, Whole ? *FString::Printf(TEXT(" with %s"), *Whole->GetName()) : TEXT(""));
		}
	}
#endif
}

// The clip for a thing of this kind in this variant at this speed (metres a second). Rows, most
// particular first: "anim.KIND_vN.run", ".walk", ".idle", or "anim.KIND_vN" alone (one clip for the
// variant: a jump, a stance); a variant with no row of its own takes the kind's, "anim.KIND.run",
// ".walk", ".idle". Faster than RunAt is run, faster than WalkAt is walk, else idle; a missing one
// takes the next slower. bGait: the clip is a walk or a run, whose place in its cycle is the thing's phase.
UAnimSequenceBase* AAethyrGame::ClipFor(const FString& Kind, int32 Variant, float Speed, bool& bGait) const
{
	const float WalkAt = 0.3f, RunAt = 3.0f;
	const FString Own = FString::Printf(TEXT("anim.%s_v%d"), *Kind, Variant);
	bool bOwn = false;
	for (const TPair<FString, TObjectPtr<UAnimSequenceBase>>& Clip : Clips) { if (Clip.Key == Own || Clip.Key.StartsWith(Own + TEXT("."))) { bOwn = true; break; } }
	const FString Base = bOwn ? Own : TEXT("anim.") + Kind;
	const TCHAR* Tiers[3] = { TEXT(".run"), TEXT(".walk"), TEXT(".idle") };
	for (int32 Tier = Speed >= RunAt ? 0 : Speed >= WalkAt ? 1 : 2; Tier < 3; Tier++)
	{
		if (const TObjectPtr<UAnimSequenceBase>* Found = Clips.Find(Base + Tiers[Tier])) { bGait = Tier < 2; return *Found; }
	}
	bGait = false;
	const TObjectPtr<UAnimSequenceBase>* Alone = Clips.Find(Base);
	return Alone ? Alone->Get() : nullptr;
}

void AAethyrGame::UpdateThings()
{
#if AETHYR_HAS_RULES
	if (NativeKinds.Num() == 0) { return; }
	float Things[48 * 8];
	const int32 Count = aethyr_things(State, Things, 48 * 8);
	int32 Used = 0, Skinned = 0;
	const float Dt = GetWorld() ? GetWorld()->GetDeltaSeconds() : 0.0f;
	for (int32 I = 0; I < Count; I++)
	{
		const float* T = Things + I * 8;
		const int32 Kind = (int32)T[0], Variant = FMath::Clamp((int32)T[1], 0, 15);
		if (!NativeKinds.Contains(Kind)) { continue; }
		// to the side of the world's own drawing of it, across the way it faces (-AethyrThingBeside)
		const FVector Beside = ThingBeside == 0.0f ? FVector::ZeroVector : FVector(FMath::Cos(T[5]), -FMath::Sin(T[5]), 0.0f) * ThingBeside;
		if (const TObjectPtr<USkeletalMesh>* Skin = NativeSkins.Find(Kind))
		{
			if (Skinned >= SkinPool.Num())
			{
				USkeletalMeshComponent* Made = NewObject<USkeletalMeshComponent>(this);
				Made->SetupAttachment(RootComponent);
				Made->SetCollisionEnabled(ECollisionEnabled::NoCollision);
				Made->SetMobility(EComponentMobility::Movable);
				Made->SetAnimationMode(EAnimationMode::AnimationSingleNode);
				Made->VisibilityBasedAnimTickOption = EVisibilityBasedAnimTickOption::AlwaysTickPoseAndRefreshBones;
				Made->RegisterComponent();
				SkinPool.Add(Made); SkinNow.AddDefaulted();
			}
			USkeletalMeshComponent* C = SkinPool[Skinned]; FSkinNow& Now = SkinNow[Skinned]; Skinned++;
			if (C->GetSkeletalMeshAsset() != *Skin)
			{
				C->SetSkeletalMeshAsset(*Skin); Now = FSkinNow();
				// a mesh that carries fills (vertex colours) wears the look's own cel material, as the
				// world's things do: its colour a vertex, its material in the second UV set's u
				if (LiveMaterial && (*Skin)->GetHasVertexColors()) { for (int32 Slot = 0; Slot < C->GetNumMaterials(); Slot++) { C->SetMaterial(Slot, LiveMaterial); } }
			}
			const FVector At = ToUnreal(T[2], T[3], T[4]);
			// its speed over the ground, from where it was (a step of more than 3 m is the run placing it)
			const float Moved = Now.bSeen ? FVector::Dist2D(At, Now.Last) / Units : 0.0f;
			const float Speed = (Dt > 0.0001f && Moved < 3.0f) ? Moved / Dt : 0.0f;
			Now.Speed += (Speed - Now.Speed) * (1.0f - FMath::Exp(-12.0f * Dt));
			const bool bPhased = Now.bSeen && T[7] != Now.Phase;
			Now.Last = At; Now.Phase = T[7]; Now.bSeen = true;
			bool bGait = false;
			UAnimSequenceBase* Clip = ClipFor(Manifest->ThingKinds[Kind].ToString(), Variant, Now.Speed, bGait);
			if (Clip != Now.Clip) { Now.Clip = Clip; if (Clip) { C->PlayAnimation(Clip, true); } }
			if (Clip && bGait && bPhased)
			{
				// a gait keeps step with the rules: a whole turn of the phase is one cycle of the clip
				const float Turn = T[7] / (2.0f * PI);
				C->SetPlayRate(0.0f);
				C->SetPosition((Turn - FMath::FloorToFloat(Turn)) * Clip->GetPlayLength(), false);
			}
			else if (Clip) { C->SetPlayRate(1.0f); if (!C->IsPlaying()) { C->Play(true); } }
			C->SetWorldLocationAndRotation(At + Beside, FRotator(0.0f, -FMath::RadiansToDegrees(T[5]), 0.0f));
			C->SetVisibility(true);
			C->bCastHiddenShadow = true;
			C->SetHiddenInGame(Kind == 0 && bBodyHidden);
			continue;
		}
		TObjectPtr<UStaticMesh>* Mesh = NativeMeshes.Find(Kind * 16 + Variant);
		if (!Mesh) { Mesh = NativeMeshes.Find(Kind * 16); }
		if (!Mesh) { continue; }
		if (Used >= ThingPool.Num())
		{
			UStaticMeshComponent* Made = NewObject<UStaticMeshComponent>(this);
			Made->SetupAttachment(RootComponent);
			Made->SetCollisionEnabled(ECollisionEnabled::NoCollision);
			Made->SetMobility(EComponentMobility::Movable);
			Made->RegisterComponent();
			ThingPool.Add(Made);
		}
		UStaticMeshComponent* C = ThingPool[Used++];
		if (C->GetStaticMesh() != *Mesh) { C->SetStaticMesh(*Mesh); }
		// a thing faces +z in its own space; turned by its yaw about the world's up
		C->SetWorldLocationAndRotation(ToUnreal(T[2], T[3], T[4]) + Beside, FRotator(0.0f, -FMath::RadiansToDegrees(T[5]), 0.0f));
		C->SetVisibility(true);
		// her own model, when Unreal draws it: hidden from the first-person camera, its shadow still cast
		C->bCastHiddenShadow = true;
		C->SetHiddenInGame(Kind == 0 && bBodyHidden);
	}
	for (int32 I = Used; I < ThingPool.Num(); I++) { ThingPool[I]->SetVisibility(false); }
	for (int32 I = Skinned; I < SkinPool.Num(); I++) { SkinPool[I]->SetVisibility(false); SkinNow[I].bSeen = false; }
#endif
}

void AAethyrGame::PlayEvents()
{
#if AETHYR_HAS_RULES
	float Events[16];
	const int32 Count = aethyr_events(State, Events, 16);
	for (int32 I = 0; I < Count; I++)
	{
		const int32 Id = (int32)Events[I];
		const FName* Name = Manifest ? Manifest->Events.Find(Id) : nullptr;
		if (!Name) { continue; }
		UE_LOG(LogAethyr, Display, TEXT("AETHYR_EVENT %s"), *Name->ToString());
		// the camera's shake: a hit on her, or an impact (a wall broken, the boss down, a blow)
		static const FName Hurt(TEXT("hurt")), Smack(TEXT("smack")), Broke(TEXT("wall-broke")), Down(TEXT("boss-down"));
		if (*Name == Hurt) { PlayCameraShake(false); }
		else if (*Name == Smack || *Name == Broke || *Name == Down) { PlayCameraShake(true); }
		if (USoundBase* Sound = Cast<USoundBase>(OverrideFor(TEXT("event.") + Name->ToString())))
		{
			UGameplayStatics::PlaySound2D(this, Sound);
			UE_LOG(LogAethyr, Display, TEXT("AETHYR_SOUND %s %s"), *Name->ToString(), *Sound->GetName());
		}
	}
#endif
}

// One step of the exported script on this actor's world. True when a frame should run now.
bool AAethyrGame::StepPlay()
{
#if AETHYR_HAS_RULES
	while (bPlaying)
	{
		if (PlayFrames > 0) { PlayFrames--; return true; }
		if (PlayLine >= PlayLines.Num())
		{
			bPlaying = false;
			UE_LOG(LogAethyr, Display, TEXT("AETHYR_PLAY %s"), bPlayFailed ? TEXT("FAILED") : TEXT("done"));
			if (bExitAfterPlay) { UKismetSystemLibrary::QuitGame(this, nullptr, EQuitPreference::Quit, false); }
			return false;
		}
		const FString Line = PlayLines[PlayLine++].TrimStartAndEnd();
		TArray<FString> W;
		Line.ParseIntoArrayWS(W);
		if (W.Num() == 0 || W[0].StartsWith(TEXT("#")) || W[0] == TEXT("size")) { continue; }
		if (W[0] == TEXT("key") && W.Num() >= 2)
		{
			const bool bNumber = W[1].Len() > 1 && (W[1][0] == TEXT('-') || FChar::IsDigit(W[1][0]));
			aethyr_key(State, bNumber ? FCString::Atoi(*W[1]) : (int32)W[1][0], !(W.Num() > 2 && W[2] == TEXT("up")));
		}
		else if (W[0] == TEXT("click") && W.Num() == 3) { aethyr_pointer(State, FCString::Atof(*W[1]), FCString::Atof(*W[2]), 0); aethyr_pointer(State, FCString::Atof(*W[1]), FCString::Atof(*W[2]), 2); }
		else if (W[0] == TEXT("msg") && W.Num() >= 2)
		{
			FString Body;
			for (int32 K = 2; K < W.Num(); K++) { Body += (K > 2 ? TEXT(" ") : TEXT("")) + W[K]; }
			const FTCHARToUTF8 Utf8(*Body);
			aethyr_message(State, FCString::Atoi(*W[1]), Utf8.Get(), Utf8.Length());
		}
		else if (W[0] == TEXT("frames") && W.Num() == 3) { PlayFrames = FCString::Atoi(*W[1]); PlayDt = FCString::Atof(*W[2]) / 1000.0f; }
		else if (W[0] == TEXT("expect") && W.Num() == 3)
		{
			const int32 Got = aethyr_fact(State, FCString::Atoi(*W[1]));
			if (Got != FCString::Atoi(*W[2])) { bPlayFailed = true; UE_LOG(LogAethyr, Display, TEXT("AETHYR_PLAY FAIL line %d: fact %s is %d, expected %s"), PlayLine, *W[1], Got, *W[2]); PlayLine = PlayLines.Num(); }
		}
		else if (W[0] == TEXT("beat") && W.Num() >= 2)
		{
			UE_LOG(LogAethyr, Display, TEXT("AETHYR_PLAY beat %s mode %d zone %d phase %d hearts %d"), *W[1], aethyr_fact(State, 0), aethyr_fact(State, 1), aethyr_fact(State, 2), aethyr_fact(State, 3));
		}
	}
#endif
	return false;
}
