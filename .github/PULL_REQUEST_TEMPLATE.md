## What this changes

<!-- One or two sentences: what changed and why. -->

## Changelog-relevant description

<!-- One line suitable for the changelog, or "N/A" if this PR has no user-visible effect
     (internal refactor, CI, docs-only). -->

- [ ] This PR is changelog-relevant (description above will be used in release notes)
- [ ] This PR is internal-only, no changelog entry needed

## Checklist

- [ ] `just check` passes locally (fmt, clippy, tests)
- [ ] If this touches hashing/serialization, FFI/C ABI, reflective-hashing walkers, or `.foldback` file/protocol parsing: reviewed against determinism hazards (see `CONTRIBUTING.md`)
- [ ] If this changes public API: cookbook/docs updated in the same PR, not deferred
