using UnrealBuildTool;

public class AethyrProbeTarget : TargetRules
{
	public AethyrProbeTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Game;
		DefaultBuildSettings = BuildSettingsVersion.V8;
		IncludeOrderVersion = EngineIncludeOrderVersion.Unreal5_9;
		ExtraModuleNames.Add("AethyrProbe");
	}
}
