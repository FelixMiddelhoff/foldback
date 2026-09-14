using System.IO;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class BuildScript
{
    private const string ScenePath = "Assets/Scenes/Main.unity";

    public static void BuildIl2Cpp()
    {
        // Only create the scene the first time — regenerating it on every
        // build gives it fresh GUIDs each run, which is pure diff noise in
        // git for a scene whose content never actually changes.
        if (!File.Exists(ScenePath))
        {
            var scene = EditorSceneManager.NewScene(NewSceneSetup.DefaultGameObjects, NewSceneMode.Single);
            EditorSceneManager.SaveScene(scene, ScenePath);
        }

        PlayerSettings.SetScriptingBackend(NamedBuildTarget.Standalone, ScriptingImplementation.IL2CPP);

        var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
        {
            scenes = new[] { ScenePath },
            locationPathName = "Build/FoldbackIl2CppTest.exe",
            target = BuildTarget.StandaloneWindows64,
            options = BuildOptions.None,
        });

        if (report.summary.result != UnityEditor.Build.Reporting.BuildResult.Succeeded)
        {
            Debug.LogError("IL2CPP build failed: " + report.summary.result);
            EditorApplication.Exit(1);
        }
    }
}
