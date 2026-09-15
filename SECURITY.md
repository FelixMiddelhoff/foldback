# Security Policy

## Reporting a vulnerability

Please use GitHub's [private vulnerability reporting](https://github.com/FelixMiddelhoff/foldback/security/advisories/new) (Security tab → "Report a vulnerability") rather than a public issue — it opens a private advisory only the maintainer can see until a fix is ready.

If you'd rather not use GitHub, DM [@FelixMiddelhoff](https://github.com/FelixMiddelhoff) on GitHub.

Please include:
- What you found and why it's a security issue, not just a bug.
- Steps to reproduce, or a minimal `.foldback` file / test case if the report involves parsing untrusted input.
- The affected crate/binding and commit or version.

## Scope

Foldback is a debugging tool, not something that runs in a trust boundary by design (no network service beyond an opt-in localhost-only live-mode WebSocket, no execution of untrusted code) — but it does parse untrusted input in one real place: `.foldback` session files, which the docs explicitly expect users to attach to bug reports and therefore treat as adversarial input (see [Testing Strategy](https://felixmiddelhoff.github.io/foldback/project/testing.html) and the `parse_session_file` fuzz target). A crash, hang, or memory-safety issue triggered by a crafted `.foldback` file, or by the live-mode WebSocket's control-message parsing, is in scope.

Out of scope: issues that require an attacker to already have local code execution on the machine running Foldback, and vulnerabilities in third-party dependencies without a demonstrated impact on Foldback itself (report those upstream — Dependabot already tracks known-vulnerable dependencies here).

## Supported versions

Pre-1.0 (`0.x`), not yet published to crates.io — fixes land on `main` and the latest commit is the only supported one. This section will be replaced with a real version table once there's a tagged release history to support.

## Response

This is currently a solo-maintained project (see [Governance](https://felixmiddelhoff.github.io/foldback/project/governance.html)) — no formal SLA, but security reports get priority over routine issues. Expect an initial response within a few days.
