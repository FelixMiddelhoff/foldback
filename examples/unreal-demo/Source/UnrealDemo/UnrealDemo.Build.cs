using UnrealBuildTool;

public class UnrealDemo : ModuleRules
{
	public UnrealDemo(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine", "Foldback" });
	}
}
