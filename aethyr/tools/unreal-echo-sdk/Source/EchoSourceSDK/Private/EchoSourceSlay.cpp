// Firm reason: 6 (Unreal's serialized animation/morph SDK, without a C ABI).
// Read-only SDK export. No sequence player, Blueprint events, source saves or rendering.
#include "CoreMinimal.h"
#include "Animation/AnimSequence.h"
#include "Animation/AnimData/IAnimationDataModel.h"
#include "Animation/AnimCurveTypes.h"
#include "Animation/MorphTarget.h"
#include "Engine/SkeletalMesh.h"
#include "Animation/Skeleton.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonSerializer.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "HAL/FileManager.h"

static TSharedPtr<FJsonValue> SlayNumber(double X) { return MakeShared<FJsonValueNumber>(X); }
static bool SlaySave(const FString& Name, const TSharedRef<FJsonObject>& Object) {
    FString Text;
    FJsonSerializer::Serialize(Object,TJsonWriterFactory<>::Create(&Text));
    const FString Root=FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("slay-source"));
    IFileManager::Get().MakeDirectory(*Root,true);
    return FFileHelper::SaveStringToFile(Text,*FPaths::Combine(Root,Name+TEXT(".json")));
}
static int32 SlayAnimation(const FString& Name, bool FaceOnly, TArray<TSharedPtr<FJsonValue>>& Receipt) {
    FString Shot=Name.Left(6);
    auto* Anim=LoadObject<UAnimSequence>(nullptr,*(TEXT("/Game/Slay/Sequences/TF/Shots/")+Shot+TEXT("/Animations/")+Name));
    if (!Anim || !Anim->GetDataModel()) { UE_LOG(LogTemp,Error,TEXT("Missing Slay animation %s"),*Name); return 30; }
    const auto* Model=Anim->GetDataModel();
    const FFrameRate Rate=Model->GetFrameRate();
    const int32 Frames=Model->GetNumberOfKeys();
    TSharedRef<FJsonObject> Out=MakeShared<FJsonObject>();
    Out->SetStringField(TEXT("asset"),Anim->GetPathName());
    Out->SetNumberField(TEXT("fps_numerator"),Rate.Numerator);
    Out->SetNumberField(TEXT("fps_denominator"),Rate.Denominator);
    Out->SetNumberField(TEXT("frames"),Frames);
    Out->SetNumberField(TEXT("duration"),Model->GetPlayLength());
    TArray<TSharedPtr<FJsonValue>> Curves;
    for (const auto& Curve:Model->GetFloatCurves()) {
        TSharedRef<FJsonObject> Row=MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"),Curve.GetName().ToString());
        TArray<TSharedPtr<FJsonValue>> Values,Keys;
        for (int32 Frame=0;Frame<Frames;++Frame) Values.Add(SlayNumber(Curve.FloatCurve.Eval(Rate.AsSeconds(FFrameTime(Frame)))));
        for (const auto& Key:Curve.FloatCurve.GetConstRefOfKeys()) {
            TSharedRef<FJsonObject> K=MakeShared<FJsonObject>();
            K->SetNumberField(TEXT("time"),Key.Time);K->SetNumberField(TEXT("value"),Key.Value);
            K->SetNumberField(TEXT("interpolation"),static_cast<int32>(Key.InterpMode));
            K->SetNumberField(TEXT("arrive_tangent"),Key.ArriveTangent);K->SetNumberField(TEXT("leave_tangent"),Key.LeaveTangent);
            Keys.Add(MakeShared<FJsonValueObject>(K));
        }
        Row->SetArrayField(TEXT("values"),Values);Row->SetArrayField(TEXT("keys"),Keys);
        Curves.Add(MakeShared<FJsonValueObject>(Row));
    }
    Out->SetArrayField(TEXT("curves"),Curves);
    TArray<FName> Names;Model->GetBoneTrackNames(Names);
    TArray<TSharedPtr<FJsonValue>> Bones;
    for (FName Bone:Names) {
        if (FaceOnly && !Bone.ToString().StartsWith(TEXT("FACIAL_"))) continue;
        TArray<FTransform> Transforms;Model->GetBoneTrackTransforms(Bone,Transforms);
        TSharedRef<FJsonObject> Row=MakeShared<FJsonObject>();Row->SetStringField(TEXT("name"),Bone.ToString());
        const auto& Ref=Anim->GetSkeleton()->GetReferenceSkeleton();
        const int32 RefIndex=Ref.FindBoneIndex(Bone), Parent=RefIndex>=0?Ref.GetParentIndex(RefIndex):INDEX_NONE;
        Row->SetStringField(TEXT("parent"),Parent>=0?Ref.GetBoneName(Parent).ToString():TEXT(""));
        TArray<TSharedPtr<FJsonValue>> Values;
        for (const auto& Transform:Transforms) {
            const auto T=Transform.GetTranslation();const auto Q=Transform.GetRotation();const auto S=Transform.GetScale3D();
            for (double X:{T.X,T.Y,T.Z,Q.X,Q.Y,Q.Z,Q.W,S.X,S.Y,S.Z}) Values.Add(SlayNumber(X));
        }
        Row->SetArrayField(TEXT("values"),Values);Bones.Add(MakeShared<FJsonValueObject>(Row));
    }
    Out->SetArrayField(TEXT("bones"),Bones);
    if (FaceOnly && !SlaySave(Name,Out)) return 31;
    // Preserve original keys in the receipt export; give Coax a separate bounded
    // source-rate stream without duplicating each key's editor metadata.
    for (auto& Curve:Curves) Curve->AsObject()->RemoveField(TEXT("keys"));
    if (!SlaySave(Name+TEXT(".samples"),Out)) return 31;
    TSharedRef<FJsonObject> Entry=MakeShared<FJsonObject>();
    Entry->SetStringField(TEXT("name"),Name);Entry->SetNumberField(TEXT("frames"),Frames);
    Entry->SetNumberField(TEXT("fps"),Rate.AsDecimal());Entry->SetNumberField(TEXT("curves"),Curves.Num());Entry->SetNumberField(TEXT("bones"),Bones.Num());
    Receipt.Add(MakeShared<FJsonValueObject>(Entry));
    UE_LOG(LogTemp,Display,TEXT("SLAY %s frames=%d curves=%d bones=%d"),*Name,Frames,Curves.Num(),Bones.Num());
    return 0;
}
int32 EchoSourceSlay(const FString& Params) {
    FString Only;
    if (FParse::Value(*Params,TEXT("Only="),Only)) {
        TArray<TSharedPtr<FJsonValue>> Rows;
        return SlayAnimation(Only,false,Rows);
    }
    TArray<TSharedPtr<FJsonValue>> Clips;
    const TCHAR* Shots[]={TEXT("0030"),TEXT("0050"),TEXT("0060"),TEXT("0090"),TEXT("0100"),TEXT("0120"),TEXT("0130"),TEXT("0140"),TEXT("0140_02"),TEXT("0145"),TEXT("0160"),TEXT("0210"),TEXT("0220"),TEXT("0230"),TEXT("0250"),TEXT("0270"),TEXT("0290"),TEXT("0300"),TEXT("0310"),TEXT("0330")};
    for (const TCHAR* Shot:Shots) {const int32 R=SlayAnimation(FString(TEXT("TF"))+Shot+TEXT("_Echo_Face"),true,Clips);if(R)return R;}
    TSharedRef<FJsonObject> Receipt=MakeShared<FJsonObject>();Receipt->SetArrayField(TEXT("clips"),Clips);
    TArray<TSharedPtr<FJsonValue>> Bodies;
    for (const TCHAR* Shot:{TEXT("0050"),TEXT("0100"),TEXT("0270"),TEXT("0300"),TEXT("0310"),TEXT("0330")}) {
        const int32 R=SlayAnimation(FString(TEXT("TF"))+Shot+TEXT("_Echo"),false,Bodies);if(R)return R;
    }
    Receipt->SetArrayField(TEXT("body_clips"),Bodies);
    if (const int32 R=SlayAnimation(TEXT("TF0050_Orb_Anim"),false,Bodies)) return R;
    for (const TCHAR* MeshPath:{TEXT("/Game/AncientContent/Characters/Echo/Meshes/Echo"),TEXT("/Game/Slay/Assets/Chr/Echo/Meshes/SK_Echo")}) {
        auto* Mesh=LoadObject<USkeletalMesh>(nullptr,MeshPath);if(!Mesh)return 32;
        TArray<TSharedPtr<FJsonValue>> Morphs;
        for(const auto& Morph:Mesh->GetMorphTargets()) {
            TSharedRef<FJsonObject> Row=MakeShared<FJsonObject>();Row->SetStringField(TEXT("name"),Morph->GetName());
            const auto& Lods=Morph->GetMorphLODModels();Row->SetNumberField(TEXT("lod0_deltas"),Lods.Num()?Lods[0].Vertices.Num():0);
            Morphs.Add(MakeShared<FJsonValueObject>(Row));
        }
        Receipt->SetArrayField(MeshPath,Morphs);
        TArray<TSharedPtr<FJsonValue>> BoneNames;
        const auto& Ref=Mesh->GetRefSkeleton();
        for(int32 Index=0;Index<Ref.GetNum();++Index) {
            TSharedRef<FJsonObject> Row=MakeShared<FJsonObject>();Row->SetStringField(TEXT("name"),Ref.GetBoneName(Index).ToString());
            const int32 Parent=Ref.GetParentIndex(Index);Row->SetNumberField(TEXT("parent"),Parent);
            BoneNames.Add(MakeShared<FJsonValueObject>(Row));
        }
        Receipt->SetArrayField(FString(MeshPath)+TEXT("/bones"),BoneNames);
    }
    return SlaySave(TEXT("receipt"),Receipt)?0:33;
}
