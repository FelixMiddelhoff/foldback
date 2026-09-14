using UnrealBuildTool;
using System.Collections.Generic;

public class UnrealMassDemoEditorTarget : TargetRules
{
	public UnrealMassDemoEditorTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Editor;
		DefaultBuildSettings = BuildSettingsVersion.Latest;
		IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
		ExtraModuleNames.Add("UnrealMassDemo");
	}
}
