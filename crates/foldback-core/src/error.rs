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
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
