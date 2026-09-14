check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features

fmt:
    cargo fmt --all

test:
    cargo test --workspace --all-features

# Builds foldback-sys and runs the Unity C# binding's P/Invoke harness
# against it (bindings/unity/README.md) — not part of `check`/`test` since
# it needs the dotnet SDK, not just the Rust toolchain.
unity-test:
    cargo build -p foldback-sys
    dotnet build -c Release bindings/unity/Tests~/FoldbackSys.Tests
    cp target/debug/foldback_sys.dll bindings/unity/Tests~/FoldbackSys.Tests/bin/Release/net8.0/ 2>/dev/null || \
    cp target/debug/libfoldback_sys.so bindings/unity/Tests~/FoldbackSys.Tests/bin/Release/net8.0/ 2>/dev/null || \
    cp target/debug/libfoldback_sys.dylib bindings/unity/Tests~/FoldbackSys.Tests/bin/Release/net8.0/
    dotnet bindings/unity/Tests~/FoldbackSys.Tests/bin/Release/net8.0/FoldbackSys.Tests.dll
