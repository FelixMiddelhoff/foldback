// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bevy reflective hashing (foldback-reflective-hashing.md §2.1, §6 step
//! 2 — first engine, chosen because `bevy_reflect` already gives
//! declaration-order field iteration for free). Opt-in stays opt-in per
//! §1: this walks only types that implement
//! [`foldback_core::hashable::FoldbackHash`] (normally via
//! `#[derive(FoldbackHash)] #[derive(Reflect)]`, per the cookbook's
//! explicit-hashing recipe 5) — reflection is an alternate *producer*
//! feeding the same [`foldback_core::session::Session::hash_field`] sink
//! the manual API uses, not a second bisection code path.
//!
//! This is a field-value *walker*, not a Bevy `App`/`Plugin` — the game
//! decides when to call [`hash_reflected`] (typically once per tracked
//! entity per tick, from its own fixed-update system), the same way it
//! decides when to call `Session::hash_fields` for explicit hashing.

use bevy_reflect::{PartialReflect, ReflectRef};
use foldback_core::session::Session;
use foldback_core::Error;

/// Cycles/shared references (§3): a runaway or self-referential object
/// graph fails loudly at this depth rather than recursing forever or
/// overflowing the stack. 8 matches the plan's documented default.
const DEFAULT_MAX_DEPTH: usize = 8;

/// Hashes every field reachable from `root` via `bevy_reflect`, recording
/// each as a Level-3 field hash under `entity_id` at `tick` — the
/// reflective counterpart to `Session::hash_fields`. `root` is typically
/// a `#[derive(FoldbackHash)] #[derive(Reflect)]` component; the walk
/// itself doesn't consult `FoldbackHash` (that's the opt-in gate the
/// caller already passed by choosing to call this at all) but does
/// prefix nested field names with the path to them (`"velocity.x"`,
/// `"items[2].amount"`) so bisection output stays readable.
pub fn hash_reflected(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    field_name_prefix: &str,
    root: &dyn PartialReflect,
) -> Result<(), Error> {
    walk(session, tick, entity_id, field_name_prefix, root, 0)
}

fn walk(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    path: &str,
    value: &dyn PartialReflect,
    depth: usize,
) -> Result<(), Error> {
    if depth > DEFAULT_MAX_DEPTH {
        return Err(Error::ReflectionDepthExceeded {
            path: path.to_string(),
            max_depth: DEFAULT_MAX_DEPTH,
        });
    }

    match value.reflect_ref() {
        ReflectRef::Struct(s) => {
            for i in 0..s.field_len() {
                let Some(field) = s.field_at(i) else {
                    continue;
                };
                let name = s.name_at(i).unwrap_or("?");
                let child_path = join_path(path, name);
                walk(session, tick, entity_id, &child_path, field, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::TupleStruct(s) => {
            for i in 0..s.field_len() {
                let Some(field) = s.field(i) else {
                    continue;
                };
                let child_path = format!("{path}.{i}");
                walk(session, tick, entity_id, &child_path, field, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::Tuple(t) => {
            for i in 0..t.field_len() {
                let Some(field) = t.field(i) else {
                    continue;
                };
                let child_path = format!("{path}.{i}");
                walk(session, tick, entity_id, &child_path, field, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::List(l) => {
            // Declaration/push order is already the game's own
            // deterministic order (§3: the hazard is unordered
            // *containers*, not ordered ones) — no sort needed here.
            for (i, item) in l.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                walk(session, tick, entity_id, &child_path, item, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::Array(a) => {
            for (i, item) in a.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                walk(session, tick, entity_id, &child_path, item, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::Map(m) => {
            // §3's sorted-container rule: a reflected map has no
            // guaranteed iteration order, so entries are sorted by their
            // rendered key text before hashing — same requirement
            // `foldback_core::hashable`'s `FieldBytes` impls enforce for
            // the manual API's own `HashMap`/`HashSet` fields.
            let mut entries: Vec<(String, &dyn PartialReflect)> = m
                .iter()
                .map(|(k, v)| (format!("{k:?}"), v))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (key_text, val) in entries {
                let child_path = format!("{path}[{key_text}]");
                walk(session, tick, entity_id, &child_path, val, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::Set(s) => {
            let mut items: Vec<(String, &dyn PartialReflect)> =
                s.iter().map(|v| (format!("{v:?}"), v)).collect();
            items.sort_by(|a, b| a.0.cmp(&b.0));
            for (item_text, item) in items {
                let child_path = format!("{path}{{{item_text}}}");
                walk(session, tick, entity_id, &child_path, item, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::Enum(e) => {
            // Discriminant + payload, both in a fixed encoding, not the
            // host's in-memory enum layout (§3) — the variant name is
            // stable across compilers/platforms, unlike a raw tag byte.
            let variant_path = format!("{path}::{}", e.variant_name());
            session.hash_field(
                tick,
                entity_id,
                &format!("{variant_path}#variant"),
                e.variant_name().as_bytes(),
            )?;
            for i in 0..e.field_len() {
                let Some(field) = e.field_at(i) else {
                    continue;
                };
                let field_label = e
                    .name_at(i)
                    .map(str::to_string)
                    .unwrap_or_else(|| i.to_string());
                let child_path = format!("{variant_path}.{field_label}");
                walk(session, tick, entity_id, &child_path, field, depth + 1)?;
            }
            Ok(())
        }
        ReflectRef::Opaque(leaf) => hash_leaf(session, tick, entity_id, path, leaf),
    }
}

/// A reflected value with no further structure to walk (a primitive, or
/// any type that only implements `Reflect` as an opaque leaf). Handles
/// the common game-math primitives directly via downcast; anything else
/// falls back to `Debug` formatting so an unrecognized leaf type still
/// participates in the hash instead of being silently skipped (visible
/// wrong is better than invisible wrong, matching `UNTRACKED_FIELDS`'s
/// own philosophy).
fn hash_leaf(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    path: &str,
    leaf: &dyn PartialReflect,
) -> Result<(), Error> {
    macro_rules! try_numeric {
        ($($t:ty),* $(,)?) => {
            $(
                if let Some(v) = leaf.try_downcast_ref::<$t>() {
                    session.hash_field(tick, entity_id, path, &v.to_le_bytes())?;
                    return Ok(());
                }
            )*
        };
    }
    try_numeric!(f32, f64, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, usize, isize);

    if let Some(v) = leaf.try_downcast_ref::<bool>() {
        session.hash_field(tick, entity_id, path, &[u8::from(*v)])?;
        return Ok(());
    }
    if let Some(v) = leaf.try_downcast_ref::<String>() {
        session.hash_field(tick, entity_id, path, v.as_bytes())?;
        return Ok(());
    }

    session.hash_field(tick, entity_id, path, format!("{leaf:?}").as_bytes())
}

fn join_path(prefix: &str, field: &str) -> String {
    if prefix.is_empty() {
        field.to_string()
    } else {
        format!("{prefix}.{field}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_reflect::Reflect;
    use foldback_core::format::{Frame, FrameReader, Header};
    use std::fs::File;

    #[derive(Reflect)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Reflect)]
    struct Unit {
        pos: Position,
        hp: i32,
        tags: Vec<String>,
    }

    /// Records to a temp `.foldback` file and reads back the `FieldHash`
    /// frames it produced — mirrors the pattern `foldback_core::session`'s
    /// own `hash_field` tests use, since field/entity hashes are
    /// recording-only (no in-memory queue like `hash_tick`'s pending list).
    fn hash_and_read_back(f: impl FnOnce(&mut Session)) -> Vec<Frame> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reflect.foldback");
        {
            let mut session = Session::builder()
                .peer_count(1)
                .record_to(&path)
                .build()
                .unwrap();
            f(&mut session);
        }
        let mut file = File::open(&path).unwrap();
        Header::read_from(&mut file).unwrap();
        FrameReader::new(file).map(|f| f.unwrap()).collect()
    }

    fn field_names(frames: &[Frame]) -> Vec<String> {
        frames
            .iter()
            .filter_map(|f| match f {
                Frame::FieldHash { field_name, .. } => Some(field_name.clone()),
                _ => None,
            })
            .collect()
    }

    fn hashes(frames: &[Frame]) -> Vec<u64> {
        frames
            .iter()
            .filter_map(|f| match f {
                Frame::FieldHash { hash, .. } => Some(*hash),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn walks_nested_struct_fields_with_dotted_paths() {
        let unit = Unit {
            pos: Position { x: 1.5, y: -2.0 },
            hp: 42,
            tags: vec!["a".into(), "b".into()],
        };
        let frames = hash_and_read_back(|session| {
            hash_reflected(session, 0, 7, "unit", &unit).unwrap();
        });

        let names = field_names(&frames);
        assert!(names.contains(&"unit.pos.x".to_string()));
        assert!(names.contains(&"unit.pos.y".to_string()));
        assert!(names.contains(&"unit.hp".to_string()));
        assert!(names.contains(&"unit.tags[0]".to_string()));
        assert!(names.contains(&"unit.tags[1]".to_string()));
    }

    #[test]
    fn same_struct_state_hashes_identically() {
        let a = Unit {
            pos: Position { x: 1.5, y: -2.0 },
            hp: 42,
            tags: vec!["a".into()],
        };
        let b = Unit {
            pos: Position { x: 1.5, y: -2.0 },
            hp: 42,
            tags: vec!["a".into()],
        };

        let frames_a = hash_and_read_back(|s| hash_reflected(s, 3, 1, "u", &a).unwrap());
        let frames_b = hash_and_read_back(|s| hash_reflected(s, 3, 1, "u", &b).unwrap());
        assert_eq!(hashes(&frames_a), hashes(&frames_b));
    }

    #[test]
    fn differing_field_value_changes_the_hash() {
        let a = Position { x: 1.5, y: -2.0 };
        let b = Position { x: 1.5, y: -2.1 };

        let frames_a = hash_and_read_back(|s| hash_reflected(s, 0, 0, "p", &a).unwrap());
        let frames_b = hash_and_read_back(|s| hash_reflected(s, 0, 0, "p", &b).unwrap());
        assert_ne!(hashes(&frames_a), hashes(&frames_b));
    }

    #[test]
    fn map_field_hashes_independent_of_insertion_order() {
        use std::collections::HashMap;

        #[derive(Reflect)]
        struct Inventory {
            items: HashMap<String, u32>,
        }

        let mut a = HashMap::default();
        a.insert("sword".to_string(), 1u32);
        a.insert("shield".to_string(), 2u32);
        let mut b = HashMap::default();
        b.insert("shield".to_string(), 2u32);
        b.insert("sword".to_string(), 1u32);

        let frames_a =
            hash_and_read_back(|s| hash_reflected(s, 0, 0, "inv", &Inventory { items: a }).unwrap());
        let frames_b =
            hash_and_read_back(|s| hash_reflected(s, 0, 0, "inv", &Inventory { items: b }).unwrap());
        assert_eq!(hashes(&frames_a), hashes(&frames_b));
        assert_eq!(field_names(&frames_a), field_names(&frames_b));
    }
}
