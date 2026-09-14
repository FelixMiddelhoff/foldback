// SPDX-License-Identifier: MIT OR Apache-2.0
//! Exercises `#[derive(FoldbackHash)]` end to end — real generated code,
//! not just that it compiles: records a struct's hashed fields to a real
//! `.foldback` file and reads them back, and checks the untracked field
//! is named rather than silently dropped.
#![cfg(feature = "derive")]

use foldback_core::format::{Frame, FrameReader, Header};
use foldback_core::session::Session;
use foldback_core::FoldbackHash;

#[derive(FoldbackHash)]
struct PlayerState {
    #[foldback(hash)]
    position: [f32; 3],
    #[foldback(hash)]
    velocity: [f32; 3],
    // Deliberately unmarked and unread — this is the point of the test:
    // an unmarked field is skipped, not hashed, and shows up in
    // UNTRACKED_FIELDS instead of silently vanishing either way.
    #[allow(dead_code)]
    debug_name: String,
}

#[test]
fn untracked_fields_names_the_one_unmarked_field() {
    assert_eq!(PlayerState::UNTRACKED_FIELDS, &["debug_name"]);
}

#[test]
fn hash_fields_records_only_the_hash_marked_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("derive.foldback");

    {
        let mut session = Session::builder()
            .peer_count(2)
            .record_to(&path)
            .build()
            .unwrap();
        let state = PlayerState {
            position: [1.0, 2.0, 3.0],
            velocity: [0.0, -1.0, 0.0],
            debug_name: "not hashed".to_string(),
        };
        session.hash_fields(10, 3, &state).unwrap();
    }

    let mut file = std::fs::File::open(&path).unwrap();
    Header::read_from(&mut file).unwrap();
    let reader = FrameReader::new(file);
    let frames: Vec<Frame> = reader.map(|f| f.unwrap()).collect();

    let field_names: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::FieldHash { field_name, .. } => Some(field_name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(field_names, vec!["position", "velocity"]);

    let position_frame = frames
        .iter()
        .find(|f| matches!(f, Frame::FieldHash { field_name, .. } if field_name == "position"))
        .unwrap();
    let Frame::FieldHash {
        tick,
        entity_id,
        value,
        ..
    } = position_frame
    else {
        unreachable!()
    };
    assert_eq!(*tick, 10);
    assert_eq!(*entity_id, 3);
    let mut expected = Vec::new();
    for x in [1.0f32, 2.0, 3.0] {
        expected.extend_from_slice(&x.to_le_bytes());
    }
    assert_eq!(value, &expected);
}
