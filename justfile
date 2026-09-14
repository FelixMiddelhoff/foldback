check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

fmt:
    cargo fmt --all

test:
    cargo test --workspace
