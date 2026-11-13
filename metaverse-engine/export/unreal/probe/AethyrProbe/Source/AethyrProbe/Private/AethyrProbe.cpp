// Runs the world's fixed script once when the module starts and logs one line per chunk
// (metaverse-engine/export/unreal/probe/frame-dump.c). The same lines come from the native probe and the
// 32-bit Wasm build; equal lines mean equal bytes.
#include "CoreMinimal.h"
#include "Modules/ModuleManager.h"

extern "C" int aethyr_probe_run(const char* Path, void (*Line)(const char*));

DEFINE_LOG_CATEGORY_STATIC(LogAethyrProbe, Log, All);

static void ProbeLine(const char* Text)
{
	UE_LOG(LogAethyrProbe, Display, TEXT("%hs"), Text);
}

class FAethyrProbeModule : public FDefaultGameModuleImpl
{
	virtual void StartupModule() override
	{
		const int Result = aethyr_probe_run(nullptr, &ProbeLine);
		UE_LOG(LogAethyrProbe, Display, TEXT("AETHYR_PROBE done %d"), Result);
	}
};

IMPLEMENT_PRIMARY_GAME_MODULE(FAethyrProbeModule, AethyrProbe, "AethyrProbe");
