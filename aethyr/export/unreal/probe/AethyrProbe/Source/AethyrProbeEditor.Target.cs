using UnrealBuildTool;

public class AethyrProbeEditorTarget : TargetRules
{
	public AethyrProbeEditorTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Editor;
		DefaultBuildSettings = BuildSettingsVersion.V8;
		IncludeOrderVersion = EngineIncludeOrderVersion.Unreal5_9;
		ExtraModuleNames.Add("AethyrProbe");
	}
}
