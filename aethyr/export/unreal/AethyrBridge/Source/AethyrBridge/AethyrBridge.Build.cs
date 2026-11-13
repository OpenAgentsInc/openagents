using System.IO;
using UnrealBuildTool;

// The bridge. When an export's rules/ have been copied to Private/Rules/ (the importer does it),
// the world's C is compiled here, as C, and AAethyrGame hosts it. Without them the plugin still
// builds: the importer's classes and checks need no rules.
public class AethyrBridge : ModuleRules
{
	public AethyrBridge(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.NoPCHs;
		bUseUnity = false;
		PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine", "InputCore", "ProceduralMeshComponent", "Slate", "SlateCore", "RenderCore" });
		if (Target.bBuildEditor)
		{
			PrivateDependencyModuleNames.AddRange(new string[] { "UnrealEd", "RenderCore", "RHI" });
		}
		string Rules = Path.Combine(ModuleDirectory, "Private", "Rules");
		bool bHasRules = File.Exists(Path.Combine(Rules, "aethyr_host.h"));
		PrivateDefinitions.Add("AETHYR_HAS_RULES=" + (bHasRules ? "1" : "0"));
		if (bHasRules)
		{
			PrivateIncludePaths.Add(Rules);
			// The Coax compiler's C returns after a call that never returns; Unreal's V7+ build
			// settings make that an error (-Wunreachable-code-aggressive).
			CppCompileWarningSettings.UnreachableCodeWarningLevel = WarningLevel.Off;
		}
	}
}
