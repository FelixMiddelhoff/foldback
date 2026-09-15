# Governance & Trademark

## License

**MIT OR Apache-2.0**, dual-licensed — the Rust-ecosystem convention, so anyone pulling in `foldback-core` as a dependency already expects this pairing. Apache-2.0 adds an explicit patent grant and patent-retaliation clause that MIT alone lacks; dual-licensing keeps the barrier to adoption as low as MIT while still making that protection available to anyone who wants it. A contributor's PR is licensed under both automatically — see `CONTRIBUTING.md`'s "unless you explicitly state otherwise" clause, the actual mechanism, not just a norm.

Applied uniformly across every surface (Rust crates, the Tauri UI's Rust and frontend code, Unity/Godot/Unreal packages) — no split-license monorepo confusion.

**DCO, not a CLA.** A contributor signs off (`git commit -s`) attesting they have the right to submit under the project's license — no copyright assignment, ever. This is deliberately harder to change later (a license change would need affirmative agreement from every contributor who holds copyright on code still in the tree) — an accepted tradeoff for keeping contribution low-friction now.

## Trademark

Protects the *name*, not the code — so a user who downloads something called Foldback can trust it's the real project, which matters more than usual for a debugging tool specifically (a malicious fork mishandling session data would damage trust in the real project's core promise).

**Current posture: defer formal registration, don't skip it forever.** Relying on common-law trademark rights (which attach automatically through actual use) until there's meaningful adoption to justify the cost of a real registered-trademark search and filing. A `TRADEMARK.md` (modeled on the Rust Foundation's policy — permissive for describing compatibility, restrictive on implying official endorsement for a modified fork) gets written once that trigger fires, not before.

## Contributor governance

**Currently: solo maintainer.** No formal governance document needed yet — imposing a committee structure on a one-person project doesn't buy anything at this size. The RFC-shaped planning docs already in this repo (see [RFCs](rfcs/README.md)) are what deliberate design decision-making looks like in practice, even solo.

**Trigger to formalize a `GOVERNANCE.md`**: a second contributor merging PRs regularly, not just submitting them. At that point: maintainer tiers (core-crate vs. per-binding, following the repo's own seams), an RFC process for anything touching the protocol spec or C ABI (a markdown doc in `project/rfcs/`, a stated minimum comment period, maintainer consensus or the founding maintainer's tie-break), and normal review-and-merge with no RFC needed for day-to-day PRs.

**A higher bar than a normal RFC** applies to two things, both close to irreversible for the whole community: changing the license (practically very hard given no copyright assignment — deliberate), and transferring trademark/project ownership.

**Code of conduct enforcement**: a named contact from day one, even solo — see [`CODE_OF_CONDUCT.md`](https://github.com/FelixMiddelhoff/foldback/blob/main/CODE_OF_CONDUCT.md) in the repo.

**Bus-factor / succession**: set up at bootstrap, not deferred — a second trusted crates.io owner once the crate is published, a GitHub org (not a personal account) with a second org owner once one exists. Cheap now, expensive to improvise during an actual emergency.

## Funding

GitHub Sponsors / Open Collective, opt-in, disclosed plainly, once there's real usage to justify it — doesn't change the license or create a paid tier. Not a current priority.
