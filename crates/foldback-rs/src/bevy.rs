// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bevy reflective hashing (foldback-reflective-hashing.md §2.1, §6 step
//! 2 — first engine, chosen because `bevy_reflect` already gives
//! declaration-order field iteration for free).
//!
//! **Opt-in survives the reflection layer (§1), enforced, not just
//! documented**: [`hash_reflected`] requires `root`'s type to implement
//! [`FoldbackHash`] (normally via `#[derive(FoldbackHash)]
//! #[derive(Reflect)]`, the cookbook's explicit-hashing recipe 5) and
//! only walks the root struct's fields named in `T::TRACKED_FIELDS` —
//! the exact set `#[foldback(hash)]` marked, whether that tag is read by
//! codegen (the manual API) or, here, by reflection. An untagged
//! top-level field is invisible to the walker, same as it's invisible to
//! `write_hashed_fields`. Once inside a tracked field, the walk recurses
//! into everything reachable from it (matching the Unreal binding's
//! model: a tagged field's own nested structure doesn't need its own
//! separate tagging) — the opt-in boundary is per top-level field, not
//! per leaf.
//!
//! Reflection is an alternate *producer* feeding the same
//! [`foldback_core::session::Session::hash_field`] sink the manual API
//! uses, not a second bisection code path.
//!
//! This is a field-value *walker*, not a Bevy `App`/`Plugin` — the game
//! decides when to call [`hash_reflected`] (typically once per tracked
//! entity per tick, from its own fixed-update system), the same way it
//! decides when to call `Session::hash_fields` for explicit hashing.

use bevy_reflect::{PartialReflect, ReflectRef};
use foldback_core::hashable::FoldbackHash;
use foldback_core::session::Session;
use foldback_core::Error;

pub mod debug;

/// Cycles/shared references (§3): a runaway object graph fails loudly at
/// this depth rather than recursing forever or overflowing the stack. 8
/// matches the plan's documented default.
///
/// **No separate identity-based visited-set guard for Bevy, and this is
/// a deliberate finding, not an oversight**: §3 asks for one alongside
/// the depth guard, for genuine reference cycles. A naive
/// pointer-identity check was tried and removed — a struct's first field
/// shares its address with the struct itself (zero offset, ordinary
/// pointer arithmetic), so it flags *every* struct's first field as a
/// "cycle" back to its own parent, a false positive on the common case,
/// not the rare one. More fundamentally, a plain `bevy_reflect` walk
/// only ever sees *owned* value trees (struct/tuple/array/enum
/// containment) — Rust's ownership model makes a value structurally
/// unable to contain itself, so a real reference cycle can't arise this
/// way at all; it could only appear through an actual indirection type
/// (`Box<dyn Reflect>`, `Rc`) being walked as if it were a plain nested
/// value, which this walker doesn't currently do (see the module docs).
/// The depth guard alone is therefore both correct and sufficient here.
/// Unity/Godot's own reflective walkers, whose engines expose real
/// object-reference graphs (a `GameObject` field pointing at another),
/// will need an actual object-identity check — not this — when their
/// turn comes.
const DEFAULT_MAX_DEPTH: usize = 8;

/// Hashes `root`'s `#[foldback(hash)]`-tracked fields (and everything
/// reachable beneath them) via `bevy_reflect`, recording each leaf as a
/// Level-3 field hash under `entity_id` at `tick` — the reflective
/// counterpart to `Session::hash_fields`. `root` is a
/// `#[derive(FoldbackHash)] #[derive(Reflect)]` component; field names in
/// the recorded frames are dotted/indexed paths from `field_name_prefix`
/// (`"unit.pos.x"`, `"unit.items[2].amount"`) so bisection output stays
/// readable. Returns the `(path, hash)` pairs actually recorded — the
/// data a visibility tool (`lint`, `debug`) or a caller's own logging
/// needs to show what was captured, per §4's "close the loop" guard
/// against invisible auto-hashing.
pub fn hash_reflected<T>(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    field_name_prefix: &str,
    root: &T,
) -> Result<Vec<(String, u64)>, Error>
where
    T: FoldbackHash + PartialReflect,
{
    let ReflectRef::Struct(s) = root.reflect_ref() else {
        return Err(Error::ReflectionRootNotStruct);
    };

    // Schema-drift detection (foldback-reflective-hashing.md §7): records
    // this type's tagged field set once per session, so a later analysis
    // can tell whether the set of `#[foldback(hash)]` fields changed
    // between the build that recorded a session and the one reading it.
    session.record_schema(std::any::type_name::<T>(), T::TRACKED_FIELDS)?;

    let mut preview = Vec::new();
    for i in 0..s.field_len() {
        let name = s.name_at(i).unwrap_or("?");
        if !T::TRACKED_FIELDS.contains(&name) {
            continue;
        }
        let Some(field) = s.field_at(i) else {
            continue;
        };
        let child_path = join_path(field_name_prefix, name);
        walk(
            session,
            tick,
            entity_id,
            &child_path,
            field,
            1,
            &mut preview,
        )?;
    }
    Ok(preview)
}

fn walk(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    path: &str,
    value: &dyn PartialReflect,
    depth: usize,
    preview: &mut Vec<(String, u64)>,
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
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    field,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::TupleStruct(s) => {
            for i in 0..s.field_len() {
                let Some(field) = s.field(i) else {
                    continue;
                };
                let child_path = format!("{path}.{i}");
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    field,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::Tuple(t) => {
            for i in 0..t.field_len() {
                let Some(field) = t.field(i) else {
                    continue;
                };
                let child_path = format!("{path}.{i}");
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    field,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::List(l) => {
            // Declaration/push order is already the game's own
            // deterministic order (§3: the hazard is unordered
            // *containers*, not ordered ones) — no sort needed here.
            for (i, item) in l.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    item,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::Array(a) => {
            for (i, item) in a.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    item,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::Map(m) => {
            // §3's sorted-container rule: a reflected map has no
            // guaranteed iteration order, so entries are sorted by their
            // rendered key text before hashing — same requirement
            // `foldback_core::hashable`'s `FieldBytes` impls enforce for
            // the manual API's own `HashMap`/`HashSet` fields.
            let mut entries: Vec<(String, &dyn PartialReflect)> =
                m.iter().map(|(k, v)| (format!("{k:?}"), v)).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (key_text, val) in entries {
                let child_path = format!("{path}[{key_text}]");
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    val,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::Set(s) => {
            let mut items: Vec<(String, &dyn PartialReflect)> =
                s.iter().map(|v| (format!("{v:?}"), v)).collect();
            items.sort_by(|a, b| a.0.cmp(&b.0));
            for (item_text, item) in items {
                let child_path = format!("{path}{{{item_text}}}");
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    item,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::Enum(e) => {
            // Discriminant + payload, both in a fixed encoding, not the
            // host's in-memory enum layout (§3) — the variant name is
            // stable across compilers/platforms, unlike a raw tag byte.
            let variant_path = format!("{path}::{}", e.variant_name());
            let variant_field = format!("{variant_path}#variant");
            let hash = record(
                session,
                tick,
                entity_id,
                &variant_field,
                e.variant_name().as_bytes(),
            )?;
            preview.push((variant_field, hash));
            for i in 0..e.field_len() {
                let Some(field) = e.field_at(i) else {
                    continue;
                };
                let field_label = e
                    .name_at(i)
                    .map(str::to_string)
                    .unwrap_or_else(|| i.to_string());
                let child_path = format!("{variant_path}.{field_label}");
                walk(
                    session,
                    tick,
                    entity_id,
                    &child_path,
                    field,
                    depth + 1,
                    preview,
                )?;
            }
            Ok(())
        }
        ReflectRef::Opaque(leaf) => hash_leaf(session, tick, entity_id, path, leaf, preview),
    }
}

/// Records one field's hash and returns it — the shared tail every leaf
/// (and the enum variant tag) funnels through, so the returned preview
/// list and the recorded `FieldHash` frame never drift apart.
fn record(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    path: &str,
    value_bytes: &[u8],
) -> Result<u64, Error> {
    session.hash_field(tick, entity_id, path, value_bytes)?;
    Ok(foldback_core::hash::hash_bytes(value_bytes))
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
    preview: &mut Vec<(String, u64)>,
) -> Result<(), Error> {
    macro_rules! try_numeric {
        ($($t:ty),* $(,)?) => {
            $(
                if let Some(v) = leaf.try_downcast_ref::<$t>() {
                    let hash = record(session, tick, entity_id, path, &v.to_le_bytes())?;
                    preview.push((path.to_string(), hash));
                    return Ok(());
                }
            )*
        };
    }
    try_numeric!(f32, f64, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, usize, isize);

    if let Some(v) = leaf.try_downcast_ref::<bool>() {
        let hash = record(session, tick, entity_id, path, &[u8::from(*v)])?;
        preview.push((path.to_string(), hash));
        return Ok(());
    }
    if let Some(v) = leaf.try_downcast_ref::<String>() {
        let hash = record(session, tick, entity_id, path, v.as_bytes())?;
        preview.push((path.to_string(), hash));
        return Ok(());
    }

    let hash = record(
        session,
        tick,
        entity_id,
        path,
        format!("{leaf:?}").as_bytes(),
    )?;
    preview.push((path.to_string(), hash));
    Ok(())
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
    use foldback_core::FoldbackHash;
    use std::fs::File;

    #[derive(Reflect)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Reflect, FoldbackHash)]
    struct Unit {
        // Compound field types (a nested struct, a Vec) opt in via
        // `reflect`, not `hash` — `hash` would require `Position`/
        // `Vec<String>` to implement `FieldBytes`, which they don't and
        // shouldn't have to for a reflection-only field.
        #[foldback(reflect)]
        pos: Position,
        #[foldback(hash)]
        hp: i32,
        #[foldback(reflect)]
        tags: Vec<String>,
        // Deliberately untagged: proves the walker's opt-in filter (not
        // just the derive's codegen) actually excludes it.
        debug_label: String,
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
            debug_label: "not hashed".into(),
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
    fn untagged_field_is_invisible_to_the_walker() {
        // The opt-in enforcement this test is actually for: `debug_label`
        // isn't in `Unit::TRACKED_FIELDS`, so the walker must never touch
        // it — not "hash it and let the caller ignore the frame."
        let unit = Unit {
            pos: Position { x: 0.0, y: 0.0 },
            hp: 1,
            tags: vec![],
            debug_label: "should never appear".into(),
        };
        let frames = hash_and_read_back(|session| {
            hash_reflected(session, 0, 0, "unit", &unit).unwrap();
        });
        assert!(field_names(&frames)
            .iter()
            .all(|n| !n.contains("debug_label")));
    }

    #[test]
    fn preview_return_value_matches_recorded_frames() {
        let unit = Unit {
            pos: Position { x: 1.0, y: 2.0 },
            hp: 7,
            tags: vec!["x".into()],
            debug_label: "ignored".into(),
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preview.foldback");
        let mut session = Session::builder()
            .peer_count(1)
            .record_to(&path)
            .build()
            .unwrap();
        let preview = hash_reflected(&mut session, 0, 0, "unit", &unit).unwrap();
        drop(session);

        let mut file = File::open(&path).unwrap();
        Header::read_from(&mut file).unwrap();
        let frames: Vec<Frame> = FrameReader::new(file).map(|f| f.unwrap()).collect();

        assert_eq!(preview.len(), field_names(&frames).len());
        for (path, hash) in &preview {
            assert!(frames.iter().any(|f| matches!(
                f,
                Frame::FieldHash { field_name, hash: h, .. } if field_name == path && h == hash
            )));
        }
    }

    #[test]
    fn same_struct_state_hashes_identically() {
        let a = Unit {
            pos: Position { x: 1.5, y: -2.0 },
            hp: 42,
            tags: vec!["a".into()],
            debug_label: "a".into(),
        };
        let b = Unit {
            pos: Position { x: 1.5, y: -2.0 },
            hp: 42,
            tags: vec!["a".into()],
            debug_label: "b differs but is untracked".into(),
        };

        let frames_a = hash_and_read_back(|s| {
            hash_reflected(s, 3, 1, "u", &a).unwrap();
        });
        let frames_b = hash_and_read_back(|s| {
            hash_reflected(s, 3, 1, "u", &b).unwrap();
        });
        assert_eq!(hashes(&frames_a), hashes(&frames_b));
    }

    #[test]
    fn differing_tracked_field_value_changes_the_hash() {
        let mk = |x: f32| Unit {
            pos: Position { x, y: -2.0 },
            hp: 42,
            tags: vec![],
            debug_label: String::new(),
        };
        let frames_a = hash_and_read_back(|s| {
            hash_reflected(s, 0, 0, "u", &mk(1.5)).unwrap();
        });
        let frames_b = hash_and_read_back(|s| {
            hash_reflected(s, 0, 0, "u", &mk(1.6)).unwrap();
        });
        assert_ne!(hashes(&frames_a), hashes(&frames_b));
    }

    #[test]
    fn non_struct_root_is_rejected() {
        // hash_reflected requires a struct root so TRACKED_FIELDS has
        // something to filter against — enforced, not just documented.
        // The derive macro only supports named-field structs, so a
        // tuple-struct FoldbackHash impl has to be written by hand here
        // (still real: nothing stops a game from implementing the trait
        // itself instead of using the derive).
        #[derive(Reflect)]
        struct Wrapper(i32);

        impl FoldbackHash for Wrapper {
            const TRACKED_FIELDS: &'static [&'static str] = &["0"];
            const UNTRACKED_FIELDS: &'static [&'static str] = &[];

            fn write_hashed_fields(
                &self,
                session: &mut Session,
                tick: u64,
                entity_id: u64,
            ) -> Result<(), Error> {
                session.hash_field(tick, entity_id, "0", &self.0.to_le_bytes())
            }
        }

        let mut session = Session::builder().peer_count(1).build().unwrap();
        // `Wrapper` is a tuple struct — ReflectRef::TupleStruct, not
        // ::Struct — so this must be rejected even though it implements
        // FoldbackHash.
        let err = hash_reflected(&mut session, 0, 0, "w", &Wrapper(1)).unwrap_err();
        assert!(matches!(err, foldback_core::Error::ReflectionRootNotStruct));
    }

    #[test]
    fn map_field_hashes_independent_of_insertion_order() {
        use std::collections::HashMap;

        #[derive(Reflect, FoldbackHash)]
        struct Inventory {
            #[foldback(reflect)]
            items: HashMap<String, u32>,
        }

        let mut a = HashMap::default();
        a.insert("sword".to_string(), 1u32);
        a.insert("shield".to_string(), 2u32);
        let mut b = HashMap::default();
        b.insert("shield".to_string(), 2u32);
        b.insert("sword".to_string(), 1u32);

        let frames_a = hash_and_read_back(|s| {
            hash_reflected(s, 0, 0, "inv", &Inventory { items: a }).unwrap();
        });
        let frames_b = hash_and_read_back(|s| {
            hash_reflected(s, 0, 0, "inv", &Inventory { items: b }).unwrap();
        });
        assert_eq!(hashes(&frames_a), hashes(&frames_b));
        assert_eq!(field_names(&frames_a), field_names(&frames_b));
    }

    #[test]
    fn depth_guard_rejects_a_runaway_nesting() {
        #[derive(Reflect)]
        struct Deep1(i32);
        #[derive(Reflect)]
        struct Deep2(Deep1);
        #[derive(Reflect)]
        struct Deep3(Deep2);
        #[derive(Reflect)]
        struct Deep4(Deep3);
        #[derive(Reflect)]
        struct Deep5(Deep4);
        #[derive(Reflect)]
        struct Deep6(Deep5);
        #[derive(Reflect)]
        struct Deep7(Deep6);
        #[derive(Reflect)]
        struct Deep8(Deep7);
        #[derive(Reflect)]
        struct Deep9(Deep8);
        #[derive(Reflect, FoldbackHash)]
        struct Root {
            #[foldback(reflect)]
            inner: Deep9,
        }

        let d1 = Deep1(1);
        let d2 = Deep2(d1);
        let d3 = Deep3(d2);
        let d4 = Deep4(d3);
        let d5 = Deep5(d4);
        let d6 = Deep6(d5);
        let d7 = Deep7(d6);
        let d8 = Deep8(d7);
        let d9 = Deep9(d8);
        let value = Root { inner: d9 };
        let mut session = Session::builder().peer_count(1).build().unwrap();
        let err = hash_reflected(&mut session, 0, 0, "r", &value).unwrap_err();
        assert!(matches!(
            err,
            foldback_core::Error::ReflectionDepthExceeded { .. }
        ));
    }

    #[test]
    fn hash_reflected_records_schema_once_per_type_not_once_per_call() {
        let unit = Unit {
            pos: Position { x: 0.0, y: 0.0 },
            hp: 10,
            tags: vec![],
            debug_label: "u".to_string(),
        };
        let frames = hash_and_read_back(|session| {
            hash_reflected(session, 0, 0, "unit", &unit).unwrap();
            hash_reflected(session, 1, 0, "unit", &unit).unwrap();
        });

        let schema_frames: Vec<&Frame> = frames
            .iter()
            .filter(
                |f| matches!(f, Frame::Metadata { key, .. } if key.starts_with("foldback.schema.")),
            )
            .collect();
        // Two `hash_reflected` calls for the same type this session, but
        // the schema is only ever recorded once — the dedup this test is
        // actually about.
        assert_eq!(schema_frames.len(), 1);

        let Frame::Metadata { key, value } = schema_frames[0] else {
            unreachable!()
        };
        assert!(key.ends_with("Unit"));
        // `Unit::TRACKED_FIELDS` includes `pos`/`tags` (`#[foldback(reflect)]`)
        // alongside `hp` (`#[foldback(hash)]`) — the derive macro puts
        // both markings into `TRACKED_FIELDS`, since the schema fingerprint
        // is about "what's opted in at all," not just what `write_hashed_fields`
        // itself serializes.
        assert_eq!(
            value,
            &foldback_core::schema::fingerprint(Unit::TRACKED_FIELDS)
        );
    }

    /// CI regression gate for reflective hashing's performance budget
    /// (foldback-reflective-hashing.md §5): the Criterion benchmark
    /// (`benches/reflective_vs_explicit.rs`) measures the *cost*, but
    /// nothing gated CI on it — a regression (say, an accidentally
    /// quadratic walk) could land unnoticed. This is deliberately not a
    /// Criterion-based check: that would need a stored baseline to
    /// compare against across CI runs, which is its own infrastructure
    /// project. Instead, a coarse, generous ratio bound: reflective
    /// hashing measured ~3.7x explicit's cost locally (docs/src/
    /// integrations/reflective-hashing.md), so a 25x ceiling has wide
    /// margin for CI-runner noise while still catching an order-of-
    /// magnitude regression — a 15x ceiling wasn't generous enough in
    /// practice, confirmed by a real flake (16.1x) on a shared CI
    /// runner. Best-of-5 timing per side, real work (1,000 entities) to
    /// keep the signal well above scheduler-jitter noise.
    #[test]
    fn reflective_hashing_stays_within_a_generous_budget_of_explicit_hashing() {
        use std::time::{Duration, Instant};

        // Mirrors `benches/reflective_vs_explicit.rs`'s own `Unit`/
        // `ReflectiveUnit` pair: same three logical fields (a 2-float
        // position, an int, a velocity array) hashed either fully
        // explicitly or with the position reflected — comparable amounts
        // of actual hashing work on both sides, unlike reusing this
        // module's own `Unit` fixture (whose `hash`-only fields and
        // `reflect`-only fields aren't the same set, so an explicit
        // `hash_fields` call and a reflective `hash_reflected` call on it
        // wouldn't do equivalent work).
        #[derive(Reflect, FoldbackHash, Clone)]
        struct ExplicitUnit {
            #[foldback(hash)]
            pos_x: f32,
            #[foldback(hash)]
            pos_y: f32,
            #[foldback(hash)]
            hp: i32,
        }

        #[derive(Reflect, FoldbackHash, Clone)]
        struct GatePosition {
            x: f32,
            y: f32,
        }

        #[derive(Reflect, FoldbackHash, Clone)]
        struct ReflectiveUnit {
            #[foldback(reflect)]
            pos: GatePosition,
            #[foldback(hash)]
            hp: i32,
        }

        const ENTITY_COUNT: usize = 1_000;
        const MAX_RATIO: u32 = 25;

        let explicit_units: Vec<ExplicitUnit> = (0..ENTITY_COUNT)
            .map(|i| ExplicitUnit {
                pos_x: i as f32,
                pos_y: i as f32 * 2.0,
                hp: 100 - (i % 100) as i32,
            })
            .collect();
        let reflective_units: Vec<ReflectiveUnit> = (0..ENTITY_COUNT)
            .map(|i| ReflectiveUnit {
                pos: GatePosition {
                    x: i as f32,
                    y: i as f32 * 2.0,
                },
                hp: 100 - (i % 100) as i32,
            })
            .collect();

        fn time_once(f: impl Fn()) -> Duration {
            let start = Instant::now();
            f();
            start.elapsed()
        }
        // best-of-3 wasn't enough margin against real CI-runner noise —
        // a shared/loaded runner hit 16.1x against the old 15x ceiling
        // on a PR completely unrelated to this code (an actions/cache
        // version bump), confirmed genuine flakiness, not a regression.
        // best-of-5 plus a wider ceiling below.
        fn best_of_5(f: impl Fn()) -> Duration {
            (0..5).map(|_| time_once(&f)).min().unwrap()
        }

        let explicit = best_of_5(|| {
            let mut session = Session::builder().peer_count(1).build().unwrap();
            for (id, unit) in explicit_units.iter().enumerate() {
                session.hash_fields(0, id as u64, unit).unwrap();
            }
        });
        let reflective = best_of_5(|| {
            let mut session = Session::builder().peer_count(1).build().unwrap();
            for (id, unit) in reflective_units.iter().enumerate() {
                hash_reflected(&mut session, 0, id as u64, "unit", unit).unwrap();
            }
        });

        assert!(
            reflective <= explicit * MAX_RATIO,
            "reflective hashing took {reflective:?} vs explicit's {explicit:?} \
             ({:.1}x, budget is {MAX_RATIO}x) — investigate before landing, \
             this may be a real performance regression",
            reflective.as_secs_f64() / explicit.as_secs_f64().max(f64::EPSILON),
        );
    }
}
