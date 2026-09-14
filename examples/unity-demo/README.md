# examples/unity-demo

A minimal real Unity project proving the [Unity binding](../../bindings/unity) actually works under IL2CPP AOT compilation — the one thing the C# `dotnet` harness (`bindings/unity/Tests~/FoldbackSys.Tests`) can't test, since the Editor's own scripting runtime isn't IL2CPP. `Assets/Il2cppVerify.cs` runs the same checks as that harness inside a built Standalone Windows IL2CPP player and writes a pass/fail result file; `Assets/Editor/BuildScript.cs` drives the build from the command line.

References `com.foldback.unity` via a relative `file:` path in `Packages/manifest.json` — no code duplicated here, the real package.

## Building and running locally

Requires Unity Editor with the Windows Build Support (IL2CPP) module installed — see [docs/src/integrations/unity.md](../../docs/src/integrations/unity.md).

```bash
cargo build -p foldback-sys
mkdir -p examples/unity-demo/Assets/Plugins/x86_64
cp target/debug/foldback_sys.dll examples/unity-demo/Assets/Plugins/x86_64/

"<Unity install path>/Editor/Unity.exe" -batchmode -nographics -quit \
  -projectPath examples/unity-demo \
  -executeMethod BuildScript.BuildIl2Cpp \
  -logFile examples/unity-demo/build.log

FOLDBACK_IL2CPP_RESULT_PATH=examples/unity-demo/il2cpp-result.txt \
  examples/unity-demo/Build/FoldbackIl2CppTest.exe -batchmode -nographics -silent-crashes

cat examples/unity-demo/il2cpp-result.txt  # should end with PASS
```

`Assets/Plugins/`, `Library/`, `Build/`, and the other Unity-generated directories are gitignored — nothing here depends on them being committed.
