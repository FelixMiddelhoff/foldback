// SPDX-License-Identifier: MIT OR Apache-2.0
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("session file header is truncated")]
    TruncatedHeader,
    #[error("session file magic bytes do not match FBK1")]
    BadMagic,
    #[error("frame payload is malformed or shorter than its declared fields")]
    MalformedFrame,
    #[error("unrecognized frame type 0x{0:02x}")]
    UnknownFrameType(u8),
    #[error("snapshot data is corrupt or truncated")]
    CorruptSnapshot,
    /// A reflective hash walker (foldback-reflective-hashing.md §3) hit
    /// its depth guard — a cyclic or self-referential reflected object
    /// graph, surfaced loudly rather than hanging or overflowing the
    /// stack.
    #[error(
        "reflective hash walk exceeded max depth {max_depth} at '{path}' — likely a cyclic or \
         self-referential reflected object graph"
    )]
    ReflectionDepthExceeded { path: String, max_depth: usize },
    /// A reflective walker re-entered a value it's already hashing on
    /// the current path — a genuine reference cycle (as opposed to
    /// `ReflectionDepthExceeded`, which fires on a merely deep-but-finite
    /// graph). Identity-based: two distinct values with equal content
    /// never trigger this, only the same value visited twice on one
    /// path.
    #[error("reflective hash walk found a reference cycle at '{path}'")]
    ReflectionCycleDetected { path: String },
    /// The root value handed to a reflective walker doesn't have the
    /// shape (a named-field struct) the field-name opt-in list can be
    /// applied to.
    #[error(
        "reflective hashing requires a struct root (so its #[derive(FoldbackHash)] tracked-field \
         list has something to filter) — got a different reflected shape"
    )]
    ReflectionRootNotStruct,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
