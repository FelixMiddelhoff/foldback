using UnrealBuildTool;
using System.Collections.Generic;

public class UnrealDemoTarget : TargetRules
{
	public UnrealDemoTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Game;
		DefaultBuildSettings = BuildSettingsVersion.Latest;
		IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
		ExtraModuleNames.Add("UnrealDemo");
	}
}
