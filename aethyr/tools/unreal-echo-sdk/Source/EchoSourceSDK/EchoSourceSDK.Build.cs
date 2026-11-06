// Firm reason: 6 (Unreal Build Tool's module SDK).
using UnrealBuildTool;
public class EchoSourceSDK : ModuleRules {
    public EchoSourceSDK(ReadOnlyTargetRules Target) : base(Target) {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
        PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine", "HairStrandsCore", "UnrealEd", "RenderCore", "Json" });
    }
}
