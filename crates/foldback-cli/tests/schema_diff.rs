// SPDX-License-Identifier: MIT OR Apache-2.0
//! `foldback schema-diff` — schema-drift detection
//! (foldback-reflective-hashing.md §7). Built session files on the fly
//! (not checked-in fixtures like `golden.rs`) since these only need
//! `Session::record_schema`'s Metadata frames, not a full hashing run.

use assert_cmd::Command;
use foldback_core::session::Session;

fn write_session(dir: &std::path::Path, name: &str, schemas: &[(&str, &[&str])]) -> String {
    let path = dir.join(name);
    {
        let mut session = Session::builder()
            .peer_count(1)
            .record_to(&path)
            .build()
            .unwrap();
        for (type_name, fields) in schemas {
            session.record_schema(type_name, fields).unwrap();
        }
        session.finish_recording().unwrap();
    }
    path.to_str().unwrap().to_string()
}

#[test]
fn identical_schemas_report_no_drift() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_session(dir.path(), "a.foldback", &[("Unit", &["hp", "pos"])]);
    let b = write_session(dir.path(), "b.foldback", &[("Unit", &["hp", "pos"])]);

    Command::cargo_bin("foldback")
        .unwrap()
        .args(["schema-diff", &a, &b])
        .assert()
        .success()
        .stdout("no schema drift across 1 type(s) common to both files\n");
}

#[test]
fn added_tagged_field_is_reported_as_drift_and_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let before = write_session(dir.path(), "before.foldback", &[("Unit", &["hp"])]);
    let after = write_session(dir.path(), "after.foldback", &[("Unit", &["hp", "shield"])]);

    Command::cargo_bin("foldback")
        .unwrap()
        .args(["schema-diff", &before, &after])
        .assert()
        .failure()
        .code(1)
        .stdout(
            "SCHEMA DRIFT: Unit\n\
             \u{20}\u{20}before : hp\n\
             \u{20}\u{20}after  : hp,shield\n",
        );
}

#[test]
fn type_only_present_on_one_side_is_not_drift() {
    let dir = tempfile::tempdir().unwrap();
    let before = write_session(dir.path(), "before.foldback", &[("Unit", &["hp"])]);
    let after = write_session(
        dir.path(),
        "after.foldback",
        &[("Unit", &["hp"]), ("NewType", &["x"])],
    );

    Command::cargo_bin("foldback")
        .unwrap()
        .args(["schema-diff", &before, &after])
        .assert()
        .success();
}
