using UnrealBuildTool;

public class UnrealMassDemo : ModuleRules
{
	public UnrealMassDemo(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PublicDependencyModuleNames.AddRange(
			new string[] { "Core", "CoreUObject", "Engine", "Foldback", "MassEntity", "MassCore" });
	}
}
