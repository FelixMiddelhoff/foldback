// SPDX-License-Identifier: MIT OR Apache-2.0
//! `foldback-core` — hashing, session file format, and the Level 1
//! bisection engine. See `foldback-plan.md` §3 for the design this
//! implements and `foldback-protocol-spec.md` for the wire formats.

pub mod bisect;
pub mod error;
pub mod format;
pub mod hash;
pub mod hashable;
#[cfg(feature = "live")]
pub mod live;
pub mod ring_buffer;
pub mod session;
pub mod snapshot;

pub use error::Error;
pub use hashable::FoldbackHash;

/// `#[derive(FoldbackHash)]` (cookbook recipe 5, feature `derive`) —
/// generates the [`FoldbackHash`] trait impl a struct needs for
/// [`session::Session::hash_fields`]. Re-exported here so `derive
/// (foldback_core::FoldbackHash)` and the trait resolve from the same
/// path — this is a macro, [`FoldbackHash`] above is the trait it
/// implements, and Rust's separate macro/type namespaces let both share
/// the name without conflict (the same pattern `serde::Serialize` uses).
#[cfg(feature = "derive")]
pub use foldback_derive::FoldbackHash;

pub mod prelude {
    pub use crate::bisect::DivergenceTick;
    pub use crate::hashable::FoldbackHash;
    pub use crate::session::{PendingHash, Session, SessionBuilder};
    pub use crate::Error;
}
