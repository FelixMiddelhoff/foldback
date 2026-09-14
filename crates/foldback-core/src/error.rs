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
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
