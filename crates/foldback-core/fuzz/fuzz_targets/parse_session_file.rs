// SPDX-License-Identifier: MIT OR Apache-2.0
//! The highest-value fuzz target per foldback-testing-plan.md §1.3: the
//! `.foldback` session-file parser will eventually be fed real files
//! from the wild (users attaching them to bug reports), including
//! corrupted, truncated, or outright adversarial ones. The format's own
//! contract (protocol-spec.md §1.2) is that a malformed or truncated
//! file is handled gracefully — every complete frame before the damage
//! stays valid, a bad frame returns `Err`, never a panic or a hang — so
//! this target's only job is proving that contract holds against
//! arbitrary bytes, not asserting anything about the parsed content.
#![no_main]

use std::io::Cursor;

use foldback_core::format::{FrameReader, Header};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    if Header::read_from(&mut cursor).is_err() {
        return;
    }

    // Ok and Err are both acceptable outcomes per the frame-stream
    // contract above — the only failure this target looks for is a
    // panic or a hang, which `cargo fuzz run` catches on its own.
    for frame in FrameReader::new(cursor) {
        let _ = frame;
    }
});
