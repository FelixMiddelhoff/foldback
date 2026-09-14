# RFCs

Foldback's RFC process formally activates once a second regular contributor is merging PRs (see [Governance](../governance.md)) — until then, these are the founding maintainer's own design-decision record, written in the same format an actual RFC process would use, so a later contributor gets the same reasoned paper trail a real process would have produced.

Template: a lightweight skeleton (Summary, Motivation, Design, Drawbacks, Alternatives considered, Prior art, Unresolved questions, History) — deliberately lighter than Rust's own RFC template, calibrated for a young tool rather than a language with millions of users.

## Index

| RFC | Title | Status |
|---|---|---|
| [0001](0001-session-file-format.md) | The `.foldback` session file format | accepted |
| [0002](0002-c-abi-surface.md) | The C ABI surface (`foldback-sys`) | accepted |
| [0003](0003-field-hashing-opt-in.md) | Field-hashing derive macro defaults to opt-in | accepted |
| [0004](0004-live-mode-transport.md) | Live-mode transport (loopback WebSocket) | accepted |
| [0005](0005-bisection-granularity-model.md) | Bisection granularity model and its snapshot-restore limitation | accepted |

Numbering is sequential and never reused, even if an early-numbered RFC is later rejected — a gap in the sequence is normal.
