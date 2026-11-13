// Firm reason: 6 (serialized animation curve SDK access, unavailable through the C ABI).
#include "CoreMinimal.h"
#include "Animation/AnimSequence.h"
#include "Animation/AnimData/IAnimationDataModel.h"
#include "Animation/AnimCurveTypes.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonSerializer.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"

int32 EchoSourceCurves() {
    const TCHAR* Names[] = {TEXT("Idle"),TEXT("Walk_Fwd"),TEXT("Jog_Fwd"),TEXT("Jump_Idle_Fall"),TEXT("Jump_Idle_Land"),TEXT("Idle_to_Walk_Fwd"),TEXT("Walk_Fwd_to_Idle"),TEXT("Idle_To_Jog_Fwd"),TEXT("Jog_Fwd_to_Idle"),TEXT("VaultOver"),TEXT("Idle_to_Walk_Fwd_Left_90"),TEXT("Idle_to_Walk_Fwd_Right_90"),TEXT("Idle_to_Walk_Fwd_Left_180"),TEXT("Idle_to_Walk_Fwd_Right_180"),TEXT("Idle_to_Jog_Fwd_Left_90"),TEXT("Idle_to_Jog_Fwd_Right_90"),TEXT("Idle_to_Jog_Fwd_Left_180"),TEXT("Idle_to_Jog_Fwd_Right_180"),TEXT("Idle_Long"),TEXT("Sitting_Idle"),TEXT("StandFromSitting"),TEXT("ReachOut_Start"),TEXT("ReachOut_Loop"),TEXT("ReachOut_End")};
    TSharedRef<FJsonObject> Output = MakeShared<FJsonObject>();
    for (const TCHAR* Name : Names) {
        auto* Anim = LoadObject<UAnimSequence>(nullptr,*(FString(TEXT("/Game/AncientContent/Characters/Echo/Animations/"))+Name));
        if (!Anim || !Anim->GetDataModel()) return 20;
        TArray<TSharedPtr<FJsonValue>> Curves;
        for (const auto& Curve : Anim->GetDataModel()->GetFloatCurves()) {
            TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
            Row->SetStringField(TEXT("name"),Curve.GetName().ToString());
            TArray<TSharedPtr<FJsonValue>> Keys;
            for (const auto& Key : Curve.FloatCurve.GetConstRefOfKeys()) {
                TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                Value->SetNumberField(TEXT("time"),Key.Time);
                Value->SetNumberField(TEXT("value"),Key.Value);
                Value->SetNumberField(TEXT("interpolation"),static_cast<int32>(Key.InterpMode));
                Value->SetNumberField(TEXT("arrive_tangent"),Key.ArriveTangent);
                Value->SetNumberField(TEXT("leave_tangent"),Key.LeaveTangent);
                Keys.Add(MakeShared<FJsonValueObject>(Value));
            }
            Row->SetArrayField(TEXT("keys"),Keys);
            Row->SetNumberField(TEXT("at_zero"),Curve.FloatCurve.Eval(0));
            Curves.Add(MakeShared<FJsonValueObject>(Row));
        }
        Output->SetArrayField(Name,Curves);
    }
    FString Text;
    FJsonSerializer::Serialize(Output,TJsonWriterFactory<>::Create(&Text));
    return FFileHelper::SaveStringToFile(Text,*FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source/animation-curves.json"))) ? 0 : 21;
}
