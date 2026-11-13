using System.IO;
using UnrealBuildTool;

// The world's C (Private/World/, copied in by the exporter: the Coax compiler's output, the
// world's -ue-face.c, aethyr_host.h and the probe's frame-dump.c) is compiled by this module as C.
public class AethyrProbe : ModuleRules
{
	public AethyrProbe(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.NoPCHs;
		bUseUnity = false;
		bWarningsAsErrors = false;
		// The Coax compiler's C returns after a call that never returns; Unreal's V7+ build settings
		// make that an error (-Wunreachable-code-aggressive).
		CppCompileWarningSettings.UnreachableCodeWarningLevel = WarningLevel.Off;
		PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine" });
		PrivateIncludePaths.Add(Path.Combine(ModuleDirectory, "Private", "World"));
		PrivateDefinitions.Add("AETHYR_PROBE_NO_MAIN=1");
	}
}
