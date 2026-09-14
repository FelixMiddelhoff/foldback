// SPDX-License-Identifier: MIT OR Apache-2.0
//! `foldback-rs` — engine/library integration helpers on top of
//! `foldback-core`. Each integration lives behind its own feature flag so
//! a project only pulls in the dependencies for the binding it actually
//! uses; see `foldback-plan.md` §6 Phase 2 for the roadmap this fulfills.

#[cfg(feature = "ggrs")]
pub mod ggrs;
