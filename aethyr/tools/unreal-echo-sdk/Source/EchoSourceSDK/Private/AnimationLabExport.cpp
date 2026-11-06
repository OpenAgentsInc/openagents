// Firm reason: 6 (Unreal skeletal mesh, animation and glTF SDK effects).
// A Coax-admitted request supplies asset paths and private output names.
// Read/export only: no sequence playback, game modules, rendering or source saves.
#include "CoreMinimal.h"
#include "Engine/SkeletalMesh.h"
#include "Animation/AnimSequence.h"
#include "Animation/MorphTarget.h"
#include "Exporters/GLTFExporter.h"
#include "Options/GLTFExportOptions.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonSerializer.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "AssetCompilingManager.h"
#include "Misc/EngineVersion.h"

int32 AnimationLabSequence(const FString&,const FString&,const FString&,bool,TArray<TSharedPtr<FJsonValue>>&);

int32 AnimationLabExport(const FString& Params) {
    FString RequestPath,Text;
    if (!FParse::Value(*Params,TEXT("AnimationLab="),RequestPath) ||
        !FFileHelper::LoadFileToString(Text,*RequestPath)) return 60;
    TSharedPtr<FJsonObject> Request;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text),Request) || !Request.IsValid()) return 61;
    FString Root;
    const TArray<TSharedPtr<FJsonValue>> *Meshes,*Animations;
    if (!Request->TryGetStringField(TEXT("output"),Root) ||
        !Request->TryGetArrayField(TEXT("meshes"),Meshes) ||
        !Request->TryGetArrayField(TEXT("animations"),Animations)) return 62;
    FString Profile;Request->TryGetStringField(TEXT("profile"),Profile);
    const bool Legacy=Profile==TEXT("echo-a0");
    auto* Raw=IConsoleManager::Get().FindConsoleVariable(TEXT("a.ForceEvalRawData"));
    if (!Raw) return 69;
    Raw->Set(Legacy?0:1,ECVF_SetByCode);
    IFileManager::Get().MakeDirectory(*Root,true);
    auto* Options=NewObject<UGLTFExportOptions>();
    Options->DefaultLevelOfDetail=0;
    Options->bExportVertexSkinWeights=true;
    Options->bExportAnimationSequences=true;
    Options->bExportMorphTargets=true;
    Options->BakeMaterialInputs=EGLTFMaterialBakeMode::Disabled;
    Options->TextureImageFormat=EGLTFTextureImageFormat::PNG;
    Options->bExportPreviewMesh=false;
    auto Receipt=MakeShared<FJsonObject>();
    Receipt->SetStringField(TEXT("engine_version"),FEngineVersion::Current().ToString());
    Receipt->SetStringField(TEXT("exporter"),TEXT("GLTFExporter, LOD 0, skin weights, morph targets, no material baking, no preview mesh"));
    Receipt->SetStringField(TEXT("sampling"),Legacy?TEXT("compressed-ready"):TEXT("raw-data-model"));
    for (const auto& Value:*Meshes) {
        const auto Row=Value->AsObject();
        if (!Row.IsValid()) return 63;
        const FString Asset=Row->GetStringField(TEXT("asset")),Name=Row->GetStringField(TEXT("name"));
        auto* Mesh=LoadObject<USkeletalMesh>(nullptr,*Asset);
        if (!Mesh || !UGLTFExporter::ExportToGLTF(Mesh,FPaths::Combine(Root,Name+TEXT(".gltf")),Options)) return 64;
        TArray<TSharedPtr<FJsonValue>> Morphs,Bones;
        for (const auto& Morph:Mesh->GetMorphTargets()) {
            auto Item=MakeShared<FJsonObject>();Item->SetStringField(TEXT("name"),Morph->GetName());
            const auto& Lods=Morph->GetMorphLODModels();
            Item->SetNumberField(TEXT("lod0_deltas"),Lods.Num()?Lods[0].Vertices.Num():0);
            Morphs.Add(MakeShared<FJsonValueObject>(Item));
        }
        const auto& Skeleton=Mesh->GetRefSkeleton();
        for (int32 Index=0;Index<Skeleton.GetNum();++Index) {
            auto Item=MakeShared<FJsonObject>();Item->SetStringField(TEXT("name"),Skeleton.GetBoneName(Index).ToString());
            Item->SetNumberField(TEXT("parent"),Skeleton.GetParentIndex(Index));Bones.Add(MakeShared<FJsonValueObject>(Item));
        }
        Receipt->SetArrayField(Asset,Morphs);Receipt->SetArrayField(Asset+TEXT("/bones"),Bones);
    }
    TArray<TSharedPtr<FJsonValue>> Faces,Bodies;
    for (const auto& Value:*Animations) {
        const auto Row=Value->AsObject();if (!Row.IsValid()) return 65;
        const FString Asset=Row->GetStringField(TEXT("asset")),Name=Row->GetStringField(TEXT("name"));
        const bool FaceOnly=Row->GetBoolField(TEXT("face_only"));
        auto* Animation=LoadObject<UAnimSequence>(nullptr,*Asset);if (!Animation) return 66;
        FAssetCompilingManager::Get().FinishAllCompilation();
        if (!UGLTFExporter::ExportToGLTF(Animation,FPaths::Combine(Root,Name+TEXT(".gltf")),Options)) return 67;
        const int32 Status=AnimationLabSequence(Asset,Name,Root,FaceOnly,FaceOnly?Faces:Bodies);if(Status) return Status;
    }
    Receipt->SetArrayField(TEXT("clips"),Faces);Receipt->SetArrayField(TEXT("body_clips"),Bodies);
    FString ReceiptText;FJsonSerializer::Serialize(Receipt,TJsonWriterFactory<>::Create(&ReceiptText));
    return FFileHelper::SaveStringToFile(ReceiptText,*FPaths::Combine(Root,TEXT("receipt.json")))?0:68;
}
