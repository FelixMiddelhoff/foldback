// SPDX-License-Identifier: MIT OR Apache-2.0
//! Schema-drift detection (foldback-reflective-hashing.md §7): a
//! reflective walker's tagged-field set for a type can change between the
//! build that recorded a `.foldback` session and the build now analyzing
//! it (someone added or removed a `#[foldback(hash)]` field
//! mid-development). Each binding records its tagged fields as a
//! `Metadata` frame once per type per session
//! ([`crate::session::Session::record_schema`]); comparing two files'
//! recorded schemas — old recording vs new, or recorded vs a
//! currently-running build's own fingerprints — catches the drift instead
//! of silently treating a stale comparison as a real divergence.

use std::collections::BTreeMap;

use crate::format::Frame;

/// The `Metadata` frame key prefix a schema entry is recorded under.
pub const SCHEMA_KEY_PREFIX: &str = "foldback.schema.";

pub fn schema_metadata_key(type_name: &str) -> String {
    format!("{SCHEMA_KEY_PREFIX}{type_name}")
}

/// Canonical, order-independent representation of a type's tagged field
/// set — sorted and comma-joined, so the same field set always produces
/// the same string regardless of declaration order.
pub fn fingerprint(tracked_fields: &[&str]) -> String {
    let mut fields: Vec<&str> = tracked_fields.to_vec();
    fields.sort_unstable();
    fields.join(",")
}

/// One type's schema drift between two recordings: same type name,
/// different tagged-field fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDrift {
    pub type_name: String,
    pub before: String,
    pub after: String,
}

/// Extracts every `foldback.schema.*` entry from a frame stream into
/// `type_name -> fingerprint`. Later entries for the same type overwrite
/// earlier ones (a session only ever writes one per type anyway, per
/// `Session::record_schema`'s once-per-type dedup).
pub fn extract_schemas<'a>(
    frames: impl IntoIterator<Item = &'a Frame>,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for frame in frames {
        if let Frame::Metadata { key, value } = frame {
            if let Some(type_name) = key.strip_prefix(SCHEMA_KEY_PREFIX) {
                out.insert(type_name.to_string(), value.clone());
            }
        }
    }
    out
}

/// Compares two schema maps (e.g. from two `.foldback` files, or a
/// recorded file against a live binding's current fingerprints) and
/// returns every type present on both sides whose fingerprint differs. A
/// type present on only one side isn't drift by this definition — it's a
/// type that wasn't reflectively hashed (yet, or anymore) on the other
/// side, not the "field added/removed on a type both sides do hash"
/// hazard this function targets.
pub fn detect_drift(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<SchemaDrift> {
    let mut drift = Vec::new();
    for (type_name, before_fp) in before {
        if let Some(after_fp) = after.get(type_name) {
            if after_fp != before_fp {
                drift.push(SchemaDrift {
                    type_name: type_name.clone(),
                    before: before_fp.clone(),
                    after: after_fp.clone(),
                });
            }
        }
    }
    drift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_order_independent() {
        assert_eq!(fingerprint(&["b", "a"]), fingerprint(&["a", "b"]));
        assert_eq!(fingerprint(&["a", "b"]), "a,b");
    }

    #[test]
    fn extract_schemas_reads_only_schema_metadata() {
        let frames = vec![
            Frame::Metadata {
                key: schema_metadata_key("Unit"),
                value: "hp,pos".to_string(),
            },
            Frame::Metadata {
                key: "input:P1".to_string(),
                value: "jump".to_string(),
            },
            Frame::TickHash {
                tick: 1,
                peer_id: 0,
                hash: 0,
            },
        ];
        let schemas = extract_schemas(&frames);
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas.get("Unit").unwrap(), "hp,pos");
    }

    #[test]
    fn detect_drift_flags_changed_fingerprints_only() {
        let before = BTreeMap::from([
            ("Unit".to_string(), "hp,pos".to_string()),
            ("Stable".to_string(), "x".to_string()),
            ("OnlyBefore".to_string(), "y".to_string()),
        ]);
        let after = BTreeMap::from([
            ("Unit".to_string(), "hp,pos,shield".to_string()),
            ("Stable".to_string(), "x".to_string()),
            ("OnlyAfter".to_string(), "z".to_string()),
        ]);
        let drift = detect_drift(&before, &after);
        assert_eq!(
            drift,
            vec![SchemaDrift {
                type_name: "Unit".to_string(),
                before: "hp,pos".to_string(),
                after: "hp,pos,shield".to_string(),
            }]
        );
    }
}
