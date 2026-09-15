// SPDX-License-Identifier: MIT OR Apache-2.0
//! Not a substitute for the real `cargo-fuzz` target
//! (`fuzz/fuzz_targets/parse_session_file.rs`) — libFuzzer's coverage-
//! guided search finds inputs no hand-written list would — but this
//! exercises the same code path against a battery of adversarial byte
//! sequences that a plain `cargo test` can run anywhere (including this
//! Windows dev environment, where cargo-fuzz's libFuzzer build doesn't
//! link — see the `fuzz-smoke` CI job's own comment for why). Every case
//! here must not panic; `Ok`/`Err` are both fine outcomes.

use std::io::Cursor;

use foldback_core::format::{FrameReader, Header};

fn assert_no_panic(data: &[u8]) {
    let mut cursor = Cursor::new(data);
    if Header::read_from(&mut cursor).is_err() {
        return;
    }
    for frame in FrameReader::new(cursor) {
        let _ = frame;
    }
}

#[test]
fn empty_input() {
    assert_no_panic(&[]);
}

#[test]
fn header_only_no_frames() {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"FBK1");
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&60u32.to_le_bytes());
    buf.extend_from_slice(&2u32.to_le_bytes());
    buf.extend_from_slice(&[0u8; 16]);
    assert_no_panic(&buf);
}

#[test]
fn truncated_header() {
    for len in 0..32 {
        assert_no_panic(&vec![0xAA; len]);
    }
}

#[test]
fn valid_header_garbage_frames() {
    let mut header = vec![0u8; 32];
    header[0..4].copy_from_slice(b"FBK1");
    header[4..6].copy_from_slice(&1u16.to_le_bytes());

    // Every byte value as a frame type, with a payload_len claiming far
    // more than what actually follows — the "adversarial length prefix"
    // case the format's truncation-tolerance contract exists for.
    for frame_type in 0u8..=255 {
        let mut buf = header.clone();
        buf.push(frame_type);
        buf.extend_from_slice(&u32::MAX.to_le_bytes());
        buf.extend_from_slice(&[0x11; 8]);
        assert_no_panic(&buf);
    }
}

#[test]
fn valid_header_zero_length_frames_of_every_type() {
    let mut header = vec![0u8; 32];
    header[0..4].copy_from_slice(b"FBK1");
    header[4..6].copy_from_slice(&1u16.to_le_bytes());

    for frame_type in 0u8..=255 {
        let mut buf = header.clone();
        buf.push(frame_type);
        buf.extend_from_slice(&0u32.to_le_bytes());
        assert_no_panic(&buf);
    }
}

#[test]
fn bad_magic_and_random_length_prefixes_inside_field_hash_payload() {
    // A `FieldHash` frame (type 0x03) whose inner `field_name_len`/
    // `value_len` fields claim more than the outer `payload_len`
    // actually delivered — must surface as `Error::MalformedFrame`, not
    // an out-of-bounds read.
    let mut header = vec![0u8; 32];
    header[0..4].copy_from_slice(b"FBK1");
    header[4..6].copy_from_slice(&1u16.to_le_bytes());

    let mut payload = Vec::new();
    payload.extend_from_slice(&0u64.to_le_bytes()); // tick
    payload.extend_from_slice(&0u16.to_le_bytes()); // peer_id
    payload.extend_from_slice(&0u64.to_le_bytes()); // entity_id
    payload.extend_from_slice(&u32::MAX.to_le_bytes()); // field_name_len (lies)
    payload.extend_from_slice(b"x"); // far short of the claimed length

    let mut buf = header.clone();
    buf.push(0x03);
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(&payload);
    assert_no_panic(&buf);
}

#[test]
fn arbitrary_short_byte_soup_after_a_valid_header() {
    let mut header = vec![0u8; 32];
    header[0..4].copy_from_slice(b"FBK1");
    header[4..6].copy_from_slice(&1u16.to_le_bytes());

    // A spread of small pseudo-random-ish tails — cheap stand-ins for
    // what a coverage-guided fuzzer would eventually generate anyway.
    let mut seed = 0x1234_5678_9abc_def0u64;
    for _ in 0..500 {
        let mut buf = header.clone();
        let tail_len = (seed % 40) as usize;
        for _ in 0..tail_len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            buf.push((seed >> 56) as u8);
        }
        assert_no_panic(&buf);
    }
}
