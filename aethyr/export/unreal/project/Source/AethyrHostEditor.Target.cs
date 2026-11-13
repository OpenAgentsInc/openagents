using UnrealBuildTool;

public class AethyrHostEditorTarget : TargetRules
{
	public AethyrHostEditorTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Editor;
		DefaultBuildSettings = BuildSettingsVersion.V8;
		IncludeOrderVersion = EngineIncludeOrderVersion.Unreal5_9;
		ExtraModuleNames.Add("AethyrHost");
	}
}
