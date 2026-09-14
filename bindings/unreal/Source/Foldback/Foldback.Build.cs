// SPDX-License-Identifier: MIT OR Apache-2.0
using System.IO;
using UnrealBuildTool;

public class Foldback : ModuleRules
{
	public Foldback(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;

		PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine" });

		string ThirdPartyPath = Path.Combine(PluginDirectory, "Source", "ThirdParty", "Foldback");
		PublicIncludePaths.Add(Path.Combine(ThirdPartyPath, "include"));

		// foldback_sys is a Rust `staticlib` (crates/foldback-sys, crate-type
		// includes "staticlib") — vendored per-platform, not pulled from a
		// package registry, per the Unreal binding plan's standard
		// third-party-library integration pattern. Staged by CI/local
		// builds (never committed — see this repo's .gitignore).
		if (Target.Platform == UnrealTargetPlatform.Win64)
		{
			PublicAdditionalLibraries.Add(Path.Combine(ThirdPartyPath, "Win64", "foldback_sys.lib"));

			// The Rust standard library, statically linked into
			// foldback_sys.lib, needs these Windows system libs itself
			// (sockets, environment/user profile queries, the Windows
			// cryptographic RNG, and NT APIs backtrace/panic machinery
			// touches) — found by actually linking, not guessed up front.
			PublicSystemLibraries.AddRange(new string[] { "ws2_32.lib", "userenv.lib", "bcrypt.lib", "ntdll.lib" });
		}
		else if (Target.Platform == UnrealTargetPlatform.Linux)
		{
			PublicAdditionalLibraries.Add(Path.Combine(ThirdPartyPath, "Linux", "libfoldback_sys.a"));
			PublicSystemLibraries.AddRange(new string[] { "dl", "pthread" });
		}
		else if (Target.Platform == UnrealTargetPlatform.Mac)
		{
			PublicAdditionalLibraries.Add(Path.Combine(ThirdPartyPath, "Mac", "libfoldback_sys.a"));
		}
	}
}
