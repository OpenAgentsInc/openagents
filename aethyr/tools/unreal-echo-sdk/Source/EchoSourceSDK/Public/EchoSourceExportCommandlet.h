// Unreal SDK-only source export. Coax owns conversion, packing and rendering.
#pragma once
#include "Commandlets/Commandlet.h"
#include "EchoSourceExportCommandlet.generated.h"
UCLASS()
class UEchoSourceExportCommandlet : public UCommandlet {
    GENERATED_BODY()
public:
    virtual int32 Main(const FString& Params) override;
};
