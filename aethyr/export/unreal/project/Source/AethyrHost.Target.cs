using UnrealBuildTool;

public class AethyrHostTarget : TargetRules
{
	public AethyrHostTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Game;
		DefaultBuildSettings = BuildSettingsVersion.V8;
		IncludeOrderVersion = EngineIncludeOrderVersion.Unreal5_9;
		ExtraModuleNames.Add("AethyrHost");
	}
}
