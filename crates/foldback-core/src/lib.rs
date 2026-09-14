// SPDX-License-Identifier: MIT OR Apache-2.0
//! `foldback-core` — hashing, session file format, and the Level 1
//! bisection engine. See `foldback-plan.md` §3 for the design this
//! implements and `foldback-protocol-spec.md` for the wire formats.

pub mod bisect;
pub mod error;
pub mod format;
pub mod hash;
pub mod ring_buffer;
pub mod session;
pub mod snapshot;

pub use error::Error;

pub mod prelude {
    pub use crate::bisect::DivergenceTick;
    pub use crate::session::{PendingHash, Session, SessionBuilder};
    pub use crate::Error;
}
