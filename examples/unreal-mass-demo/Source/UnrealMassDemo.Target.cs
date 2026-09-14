using UnrealBuildTool;
using System.Collections.Generic;

public class UnrealMassDemoTarget : TargetRules
{
	public UnrealMassDemoTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Game;
		DefaultBuildSettings = BuildSettingsVersion.Latest;
		IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
		ExtraModuleNames.Add("UnrealMassDemo");
	}
}
