// Firm reason: 6 (Unreal's native GroomAsset SDK is not exposed to Python or C).
// Original arrays only: no decimation, conversion, simulation, graph execution or source save.
#include "EchoSourceExportCommandlet.h"
#include "GroomAsset.h"
#include "HairStrandsDatas.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/BufferArchive.h"
#include "Modules/ModuleManager.h"
#include "HAL/FileManager.h"
IMPLEMENT_MODULE(FDefaultModuleImpl, EchoSourceSDK)
int32 EchoSourceReference();
int32 EchoSourceCurves();
int32 EchoSourceSlay(const FString& Params);
int32 EchoSourceCamera();
int32 AnimationLabExport(const FString& Params);

int32 UEchoSourceExportCommandlet::Main(const FString& Params) {
    if (Params.Contains(TEXT("AnimationLab="))) return AnimationLabExport(Params);
    if (Params.Contains(TEXT("CameraTemplate"))) return EchoSourceCamera();
    if (FParse::Param(*Params,TEXT("Slay"))) return EchoSourceSlay(Params);
    if (Params.Contains(TEXT("Curves"))) {
        const int32 Result = EchoSourceCurves();
        if (Result || !Params.Contains(TEXT("Reference"))) return Result;
    }
    if (Params.Contains(TEXT("Reference"))) return EchoSourceReference();
    FString GroomRoot=TEXT("/Game/AncientContent/Characters/Echo/Hair/");
    FParse::Value(*Params,TEXT("GroomRoot="),GroomRoot);
    if (!GroomRoot.EndsWith(TEXT("/"))) GroomRoot+=TEXT("/");
    const FString Output=FPaths::Combine(FPaths::ProjectSavedDir(),TEXT("echo-source"));
    IFileManager::Get().MakeDirectory(*Output,true);
    for (const TCHAR* Name : {TEXT("Hair_S_UpdoBuns"), TEXT("Eyebrows_L_Echo")}) {
        FString AssetPath = GroomRoot + Name;
        UGroomAsset* Asset = LoadObject<UGroomAsset>(nullptr, *AssetPath);
        if (!Asset) return 2;
        FHairStrandsDatas Strands, Guides;
        if (!Asset->GetHairStrandsDatas(0, Strands, Guides)) return 3;
        FBufferArchive Bytes;
        uint32 Magic = 0x31534745; // EGS1: native little-endian SDK arrays, each FArchive length-prefixed.
        Bytes << Magic;
        Bytes << Strands.StrandsPoints.PointsPosition;
        Bytes << Strands.StrandsPoints.PointsRadius;
        Bytes << Strands.StrandsPoints.PointsCoordU;
        Bytes << Strands.StrandsCurves.CurvesCount;
        Bytes << Strands.StrandsCurves.CurvesOffset;
        Bytes << Strands.StrandsCurves.CurvesRootUV;
        FString File = FPaths::Combine(FPaths::ProjectSavedDir(), TEXT("echo-source"), FString(Name) + TEXT(".strands"));
        if (!FFileHelper::SaveArrayToFile(Bytes, *File)) return 4;
        UE_LOG(LogTemp, Display, TEXT("Echo source %s: %u curves %u points %lld bytes"), Name, Strands.StrandsCurves.Num(), Strands.StrandsPoints.Num(), static_cast<long long>(Bytes.Num()));
    }
    return 0;
}
