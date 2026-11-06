// Firm reason: 6 (Unreal's exact subsequence time transform and channel SDKs).
// Read serialized data only. Coax composes the exported camera channels.
#include "LevelSequence.h"
#include "MovieScene.h"
#include "MovieSceneBinding.h"
#include "MovieSceneTrack.h"
#include "Channels/MovieSceneChannelProxy.h"
#include "Channels/MovieSceneDoubleChannel.h"
#include "Sections/TemplateSequenceSection.h"
#include "Tracks/MovieScene3DTransformTrack.h"
#include "Sections/MovieScene3DTransformSection.h"
#include "Evaluation/MovieSceneSequenceTransform.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonSerializer.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "HAL/FileManager.h"
#include "UObject/UnrealType.h"

int32 EchoSourceCamera() {
    auto* Outer = LoadObject<ULevelSequence>(nullptr,
        TEXT("/Game/Slay/Sequences/TF/Shots/TF0290/TF0290_01"));
    if (!Outer || !Outer->GetMovieScene()) return 70;
    UMovieScene* Scene = Outer->GetMovieScene();
    UTemplateSequenceSection* Template = nullptr;
    for (const auto& Binding : Scene->GetBindings())
        for (auto* Track : Binding.GetTracks())
            for (auto* Section : Track->GetAllSections())
                if (auto* Candidate = Cast<UTemplateSequenceSection>(Section)) {
                    if (!Candidate->GetSequence() ||
                        Candidate->GetSequence()->GetName() != TEXT("HandheldShake_NoRollSequence")) continue;
                    if (Template) return 71;
                    Template = Candidate;
                }
    if (!Template) return 72;
    UMovieSceneSequence* Inner = Template->GetSequence();
    UMovieScene* InnerScene = Inner->GetMovieScene();
    if (!InnerScene) return 73;
    UMovieScene3DTransformSection* Transform = nullptr;
    for (const auto& Binding : InnerScene->GetBindings())
        for (auto* Track : Binding.GetTracks())
            if (Cast<UMovieScene3DTransformTrack>(Track))
                for (auto* Section : Track->GetAllSections())
                    if (auto* Candidate = Cast<UMovieScene3DTransformSection>(Section)) {
                        if (Transform) return 74;
                        Transform = Candidate;
                    }
    if (!Transform) return 75;
    const auto Channels = Transform->GetChannelProxy().GetChannels<FMovieSceneDoubleChannel>();
    const auto Metadata = Transform->GetChannelProxy().GetMetaData<FMovieSceneDoubleChannel>();
    if (Channels.Num() != 9 || Metadata.Num() != 9) return 76;
    const auto TrueRange = Transform->GetTrueRange();
    const FFrameTime BaseTime = TrueRange.HasLowerBound() ? TrueRange.GetLowerBoundValue()
        : InnerScene->GetPlaybackRange().GetLowerBoundValue();
    const auto TimeTransform = Template->OuterToInnerTransform();
    TSharedRef<FJsonObject> Out = MakeShared<FJsonObject>();
    Out->SetStringField(TEXT("sequence"), Outer->GetPathName());
    Out->SetStringField(TEXT("template"), Inner->GetPathName());
    Out->SetNumberField(TEXT("source_first"), 1001);
    Out->SetNumberField(TEXT("frames"), 200);
    Out->SetNumberField(TEXT("blend_type"), static_cast<int32>(Transform->GetBlendType().Get()));
    TArray<FFrameTime> Times;
    TArray<TSharedPtr<FJsonValue>> TimeValues;
    for (int32 K = 0; K < 200; ++K) {
        const auto Time = FFrameRate::TransformTime(FFrameTime(1001 + K),
            Scene->GetDisplayRate(), Scene->GetTickResolution());
        Times.Add(TimeTransform.TransformTime(Time));
        TimeValues.Add(MakeShared<FJsonValueNumber>(Times.Last().AsDecimal()));
    }
    Out->SetArrayField(TEXT("inner_ticks"), TimeValues);
    TArray<TSharedPtr<FJsonValue>> Rows;
    for (int32 K = 0; K < Channels.Num(); ++K) {
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Metadata[K].Name.ToString());
        double Base;
        if (!Channels[K]->Evaluate(BaseTime, Base)) return 77;
        Row->SetNumberField(TEXT("base"), Base);
        TArray<TSharedPtr<FJsonValue>> Values;
        for (auto Time : Times) {
            double Value;
            if (!Channels[K]->Evaluate(Time, Value)) return 78;
            Values.Add(MakeShared<FJsonValueNumber>(Value));
        }
        Row->SetArrayField(TEXT("values"), Values);
        Rows.Add(MakeShared<FJsonValueObject>(Row));
    }
    Out->SetArrayField(TEXT("channels"), Rows);
    const auto* Property = FindFProperty<FArrayProperty>(Template->GetClass(), TEXT("PropertyScales"));
    if (!Property) return 79;
    const auto* Scales = Property->ContainerPtrToValuePtr<TArray<FTemplateSectionPropertyScale>>(Template);
    Rows.Reset();
    for (const auto& Scale : *Scales) {
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("binding"), Scale.ObjectBinding.ToString());
        Row->SetStringField(TEXT("property"), Scale.PropertyBinding.PropertyPath.ToString());
        Row->SetStringField(TEXT("type"), StaticEnum<ETemplateSectionPropertyScaleType>()->GetNameStringByValue(
            static_cast<int64>(Scale.PropertyScaleType)));
        TArray<TSharedPtr<FJsonValue>> Values;
        for (int32 K = 0; K < 200; ++K) {
            const auto Time = FFrameRate::TransformTime(FFrameTime(1001 + K),
                Scene->GetDisplayRate(), Scene->GetTickResolution());
            float Value;
            if (!Scale.FloatChannel.Evaluate(Time, Value)) return 80;
            Values.Add(MakeShared<FJsonValueNumber>(Value));
        }
        Row->SetArrayField(TEXT("values"), Values);
        Rows.Add(MakeShared<FJsonValueObject>(Row));
    }
    Out->SetArrayField(TEXT("scales"), Rows);
    FString Text;
    if (!FJsonSerializer::Serialize(Out, TJsonWriterFactory<>::Create(&Text))) return 81;
    const FString Root = FPaths::Combine(FPaths::ProjectSavedDir(), TEXT("slay-source"));
    IFileManager::Get().MakeDirectory(*Root, true);
    if (!FFileHelper::SaveStringToFile(Text, *FPaths::Combine(Root, TEXT("tf0290-camera-template.json")))) return 82;
    UE_LOG(LogTemp, Display, TEXT("SLAY_CAMERA_EXACT frames=200 channels=%d scales=%d"), Channels.Num(), Scales->Num());
    return 0;
}
