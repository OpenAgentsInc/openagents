// Firm reason: 6 (offscreen Unreal SDK reference rendering; no game scripts or source package saves).
#include "CoreMinimal.h"
#include "FileHelpers.h"
#include "Engine/World.h"
#include "Engine/SkeletalMesh.h"
#include "Animation/AnimSequence.h"
#include "Animation/SkeletalMeshActor.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/SceneCaptureComponent2D.h"
#include "Engine/TextureRenderTarget2D.h"
#include "Engine/DirectionalLight.h"
#include "Components/DirectionalLightComponent.h"
#include "Engine/SkyLight.h"
#include "Components/SkyLightComponent.h"
#include "Engine/TextureCube.h"
#include "GroomAsset.h"
#include "GroomBindingAsset.h"
#include "GroomComponent.h"
#include "Kismet/KismetRenderingLibrary.h"
#include "Misc/Paths.h"
#include "Misc/FileHelper.h"
#include "ShaderCompiler.h"
#include "AssetCompilingManager.h"
#include "RenderingThread.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonSerializer.h"

static void EchoReferencePose(USkeletalMeshComponent* Mesh) {
    TSharedRef<FJsonObject> Output = MakeShared<FJsonObject>();
    TArray<TSharedPtr<FJsonValue>> Bones;
    const auto Transforms = Mesh->GetBoneSpaceTransforms();
    for (int32 Index=0; Index<Transforms.Num(); ++Index) {
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"),Mesh->GetBoneName(Index).ToString());
        const auto T = Transforms[Index].GetTranslation();
        const auto Q = Transforms[Index].GetRotation();
        const auto S = Transforms[Index].GetScale3D();
        TArray<TSharedPtr<FJsonValue>> Values;
        for (double Value : {T.X,T.Y,T.Z,Q.X,Q.Y,Q.Z,Q.W,S.X,S.Y,S.Z}) Values.Add(MakeShared<FJsonValueNumber>(Value));
        Row->SetArrayField(TEXT("local"),Values);
        Bones.Add(MakeShared<FJsonValueObject>(Row));
    }
    Output->SetArrayField(TEXT("bones"),Bones);
    TSharedRef<FJsonObject> Morphs = MakeShared<FJsonObject>();
    for (const auto& Pair : Mesh->GetMorphTargetCurves()) Morphs->SetNumberField(Pair.Key.ToString(),Pair.Value);
    Output->SetObjectField(TEXT("morphs"),Morphs);
    FString Text;
    FJsonSerializer::Serialize(Output,TJsonWriterFactory<>::Create(&Text));
    FFileHelper::SaveStringToFile(Text,*FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source/reference-pose.json")));
}

int32 EchoSourceReference() {
    UWorld* World = UEditorLoadingAndSavingUtils::NewBlankMap(false);
    if (!World) return 10;
    auto* Actor = World->SpawnActor<ASkeletalMeshActor>();
    auto* Mesh = Actor->GetSkeletalMeshComponent();
    Actor->SetActorLocation(FVector(0,0,-1));
    Mesh->SetSkeletalMeshAsset(LoadObject<USkeletalMesh>(nullptr, TEXT("/Game/AncientContent/Characters/Echo/Meshes/Echo")));
    Mesh->SetAnimationMode(EAnimationMode::AnimationSingleNode);
    Mesh->SetAnimation(LoadObject<UAnimSequence>(nullptr, TEXT("/Game/AncientContent/Characters/Echo/Animations/Idle")));
    Mesh->SetPosition(0, false);
    Mesh->TickAnimation(0, false);
    Mesh->RefreshBoneTransforms();
    TSharedRef<FJsonObject> Morphs = MakeShared<FJsonObject>();
    for (const auto& Pair : Mesh->GetMorphTargetCurves()) Morphs->SetNumberField(Pair.Key.ToString(),Pair.Value);
    FString MorphJSON;
    FJsonSerializer::Serialize(Morphs,TJsonWriterFactory<>::Create(&MorphJSON));
    FFileHelper::SaveStringToFile(MorphJSON,*FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source/reference-morph-weights.json")));
    Mesh->SetForcedLOD(1);
    for (const TCHAR* Name : {TEXT("Hair_S_UpdoBuns"), TEXT("Eyebrows_L_Echo")}) {
        auto* Groom = NewObject<UGroomComponent>(Actor);
        Actor->AddInstanceComponent(Groom);
        Groom->SetupAttachment(Mesh);
        auto* Asset = LoadObject<UGroomAsset>(nullptr, *(FString(TEXT("/Game/AncientContent/Characters/Echo/Hair/")) + Name));
        Groom->SetGroomAsset(Asset);
        if (Asset && Asset->GetHairGroupsMaterials().Num()) Groom->SetMaterial(0,Asset->GetHairGroupsMaterials()[0].Material);
        Groom->SetEnableSimulation(false);
        Groom->SetBindingAsset(LoadObject<UGroomBindingAsset>(nullptr, *(FString(TEXT("/Game/AncientContent/Characters/Echo/Hair/")) + Name + TEXT("_Echo_M3D_LOD0_Binding"))));
        Groom->RegisterComponent();
    }
    auto* Light = World->SpawnActor<ADirectionalLight>();
    Light->SetActorRotation(FVector(0.4, -0.45, -0.8).Rotation());
    Light->GetLightComponent()->SetIntensity(3.5f);
    auto* Sky = World->SpawnActor<ASkyLight>();
    Sky->GetLightComponent()->SetMobility(EComponentMobility::Movable);
    Sky->GetLightComponent()->SourceType = SLS_SpecifiedCubemap;
    auto* Cube = LoadObject<UTextureCube>(nullptr,TEXT("/Engine/EngineResources/GrayLightTextureCube"));
    TArray64<uint8> CubeSource;
    if (Cube && Cube->Source.GetMipData(CubeSource,0)) {
        FFileHelper::SaveArrayToFile(CubeSource,*FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source/reference-sky.raw")));
        UE_LOG(LogTemp,Display,TEXT("Echo sky source %d x %d, %d slices, format %d, %lld bytes"),Cube->Source.GetSizeX(),Cube->Source.GetSizeY(),Cube->Source.GetNumSlices(),static_cast<int32>(Cube->Source.GetFormat()),static_cast<long long>(CubeSource.Num()));
    }
    Sky->GetLightComponent()->SetCubemap(Cube);
    Sky->GetLightComponent()->SetIntensity(1);
    USkyLightComponent::UpdateSkyCaptureContents(World);
    auto* CaptureActor = World->SpawnActor<AActor>();
    auto* Capture = NewObject<USceneCaptureComponent2D>(CaptureActor);
    CaptureActor->AddInstanceComponent(Capture);
    CaptureActor->SetRootComponent(Capture);
    Capture->RegisterComponent();
    CaptureActor->SetActorLocation(FVector(0,220,120));
    CaptureActor->SetActorRotation((FVector(0,0,110)-CaptureActor->GetActorLocation()).Rotation());
    Capture->FOVAngle = 60;
    Capture->bCaptureEveryFrame = false;
    Capture->bCaptureOnMovement = false;
    Capture->bAlwaysPersistRenderingState = true;
    auto* Target = NewObject<UTextureRenderTarget2D>();
    Target->RenderTargetFormat = RTF_RGBA8_SRGB;
    Target->InitAutoFormat(1280,800);
    Target->UpdateResourceImmediate(true);
    Capture->TextureTarget = Target;
    FAssetCompilingManager::Get().FinishAllCompilation();
    if (GShaderCompilingManager) GShaderCompilingManager->FinishAllCompilation();
    FlushRenderingCommands();
    // Let real virtual-texture feedback and groom resources settle through SDK frames.
    // No BeginPlay, Blueprint actor or gameplay code is executed.
    for (int Frame=0; Frame<120; ++Frame) {
        ++GFrameCounter;
        World->Tick(LEVELTICK_All, 1.0f/60.0f);
        Mesh->SetPosition(0, false);
        Mesh->TickAnimation(0, false);
        Mesh->RefreshBoneTransforms();
        Capture->CaptureScene();
        FlushRenderingCommands();
    }
    EchoReferencePose(Mesh);
    Capture->CaptureSource = SCS_BaseColor;
    Capture->CaptureScene();
    FlushRenderingCommands();
    UKismetRenderingLibrary::ExportRenderTarget(World, Target, FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source")), TEXT("reference-basecolor.png"));
    Capture->CaptureSource = SCS_FinalColorLDR;
    Capture->PostProcessSettings.bOverride_AutoExposureMethod = true;
    Capture->PostProcessSettings.AutoExposureMethod = AEM_Manual;
    Capture->PostProcessSettings.bOverride_AutoExposureApplyPhysicalCameraExposure = true;
    Capture->PostProcessSettings.AutoExposureApplyPhysicalCameraExposure = false;
    Capture->CaptureScene();
    FlushRenderingCommands();
    UKismetRenderingLibrary::ExportRenderTarget(World, Target, FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source")), TEXT("reference-lit.png"));
    CaptureActor->SetActorLocation(FVector(0,65,165));
    CaptureActor->SetActorRotation((FVector(0,0,160)-CaptureActor->GetActorLocation()).Rotation());
    for (int Frame=0; Frame<120; ++Frame) {
        ++GFrameCounter;
        World->Tick(LEVELTICK_All, 1.0f/60.0f);
        Mesh->SetPosition(0,false);
        Mesh->TickAnimation(0,false);
        Mesh->RefreshBoneTransforms();
        Capture->CaptureScene();
        FlushRenderingCommands();
    }
    UKismetRenderingLibrary::ExportRenderTarget(World, Target, FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source")), TEXT("reference-head.png"));
    return 0;
}
