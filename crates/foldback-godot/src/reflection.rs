// SPDX-License-Identifier: MIT OR Apache-2.0
//! Godot reflective hashing (foldback-reflective-hashing.md §2.3, §6 step
//! 4 — last of the three, after Bevy and Unity).
//!
//! **Marker mechanism, resolved not assumed (closes the plan's §7 open
//! question)**: the plan named two options — a custom `@export_foldback`
//! property hint, or a `foldback_` naming-convention fallback "for pure
//! GDScript users who can't easily add a custom PropertyInfo flag" —
//! and flagged it as needing a spike before committing. Checked against
//! gdext 0.5's actual generated API rather than assumed: `PropertyHint`
//! is a fixed engine enum (`godot::global::PropertyHint`), and nothing
//! in GDExtension's registration surface lets a plugin add a *new* hint
//! value the editor or `@export` would recognize — that would need an
//! engine-side (C++) change, not something expressible from a
//! GDExtension addon. So the naming-convention fallback isn't a
//! fallback here, it's the only mechanism that's actually buildable from
//! GDExtension: a property is tracked iff its name starts with
//! [`TRACKED_PREFIX`].
//!
//! **Determinism note verified from gdext's own source, not assumed**:
//! unlike Rust's `HashMap` or C#'s `Dictionary<K,V>`, Godot's built-in
//! `Dictionary` is explicitly insertion-ordered — `godot-core`'s own
//! dictionary.rs says so directly ("Godot dictionaries are ordered").
//! `Array` is an ordered vector by construction. So neither of Godot's
//! two built-in compound `Variant` container types has the §3 unordered-
//! iteration hazard the Bevy/Unity walkers have to guard against with an
//! explicit sort — this walker deliberately does *not* sort Dictionary
//! entries, and that's a verified finding, not an oversight.
//!
//! **Opt-in applied per level, not just at the root — a deliberate
//! deviation from the Bevy/Unity model**: those walkers filter tagged
//! fields once at the root type, then walk everything beneath a tagged
//! field unconditionally. Doing the same here would mean "once you tag
//! one property on a `Node`, walk that whole nested `Node`'s entire
//! engine property surface" — Godot objects carry a large, noisy,
//! mostly-irrelevant built-in property set (scripts, editor metadata,
//! internal engine state) that a plain Rust struct or C# POCO doesn't.
//! So the `foldback_` prefix filter is re-applied at every nested Object
//! encountered during the walk, not just the root — narrower and less
//! surprising for Godot's actual object model, at the cost of requiring
//! every level of a nested tracked hierarchy to use the same prefix
//! convention (which a `foldback_`-prefixed property inherently already
//! does).

use std::collections::HashSet;

use godot::builtin::{GString, StringName, VarArray, VarDictionary, Variant, VariantType};
use godot::classes::Object;
use godot::meta::ToGodot;
use godot::obj::{Gd, InstanceId};

use foldback_core::hash::hash_bytes;
use foldback_core::session::Session;

/// Cycles/shared references (§3): a runaway object graph fails loudly at
/// this depth. Matches the Bevy/Unity walkers' default.
pub const DEFAULT_MAX_DEPTH: usize = 8;

/// The opt-in marker — see the module docs for why this, not a custom
/// `PropertyHint`, is what's actually buildable from GDExtension.
pub const TRACKED_PREFIX: &str = "foldback_";

/// Hashes every `foldback_`-prefixed property reachable from `target`
/// (re-checking the prefix at each nested `Object` reached along the
/// way — see the module docs), recording each leaf as a Level-3 field
/// hash under `entity_id` at `tick`. Returns the `(path, hash)` pairs
/// actually recorded — the data `list_tracked` and any future editor
/// tooling need to show what was captured.
pub fn hash_reflected(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    field_name_prefix: &str,
    target: Gd<Object>,
) -> Result<Vec<(String, u64)>, String> {
    let mut visited = HashSet::new();
    let mut preview = Vec::new();
    let value = target.to_variant();
    walk(
        session,
        tick,
        entity_id,
        field_name_prefix,
        &value,
        0,
        &mut visited,
        &mut preview,
    )?;
    Ok(preview)
}

/// The visibility-tooling data source (§4): every `foldback_`-prefixed
/// ("tracked") and non-prefixed script-declared ("untracked") property
/// on `target`'s own property list — the "did you mean to include this
/// one too" check. Doesn't recurse; that's `hash_reflected`'s job.
pub fn list_tracked(target: &Gd<Object>) -> (Vec<String>, Vec<String>) {
    let mut tracked = Vec::new();
    let mut untracked = Vec::new();
    for name in property_names(target) {
        if name.starts_with(TRACKED_PREFIX) {
            tracked.push(name);
        } else {
            untracked.push(name);
        }
    }
    (tracked, untracked)
}

#[allow(clippy::too_many_arguments)]
fn walk(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    path: &str,
    value: &Variant,
    depth: usize,
    visited: &mut HashSet<InstanceId>,
    preview: &mut Vec<(String, u64)>,
) -> Result<(), String> {
    if depth > DEFAULT_MAX_DEPTH {
        return Err(format!(
            "reflective hash walk exceeded max depth {DEFAULT_MAX_DEPTH} at '{path}' \
             — likely a cyclic or self-referential reflected object graph"
        ));
    }

    match value.get_type() {
        VariantType::NIL => {
            record(session, tick, entity_id, path, &[0xFF], preview)?;
            Ok(())
        }
        VariantType::OBJECT => {
            let Ok(obj) = value.try_to::<Gd<Object>>() else {
                // A freed or otherwise-invalid object reference — treat
                // it the same as null rather than failing the whole walk.
                record(session, tick, entity_id, path, &[0xFF], preview)?;
                return Ok(());
            };
            let id = obj.instance_id();
            if !visited.insert(id) {
                return Err(format!(
                    "reflective hash walk found a reference cycle at '{path}'"
                ));
            }
            let result = walk_object(session, tick, entity_id, path, &obj, depth, visited, preview);
            visited.remove(&id);
            result
        }
        VariantType::ARRAY => {
            let arr: VarArray = value.to();
            for (i, item) in arr.iter_shared().enumerate() {
                let child_path = format!("{path}[{i}]");
                walk(session, tick, entity_id, &child_path, &item, depth + 1, visited, preview)?;
            }
            Ok(())
        }
        VariantType::DICTIONARY => {
            // Not sorted — see the module docs: Godot's Dictionary is
            // engine-guaranteed insertion-ordered, so there's no
            // unordered-iteration hazard here to defend against.
            let dict: VarDictionary = value.to();
            for (key, val) in dict.iter_shared() {
                let child_path = format!("{path}[{}]", key.stringify());
                walk(session, tick, entity_id, &child_path, &val, depth + 1, visited, preview)?;
            }
            Ok(())
        }
        _ => {
            let bytes = leaf_bytes(value);
            record(session, tick, entity_id, path, &bytes, preview)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_object(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    path: &str,
    obj: &Gd<Object>,
    depth: usize,
    visited: &mut HashSet<InstanceId>,
    preview: &mut Vec<(String, u64)>,
) -> Result<(), String> {
    for name in property_names(obj) {
        if !name.starts_with(TRACKED_PREFIX) {
            continue;
        }
        let prop_name = StringName::from(name.as_str());
        let value = obj.get(&prop_name);
        let child_path = format!("{path}.{name}");
        walk(session, tick, entity_id, &child_path, &value, depth + 1, visited, preview)?;
    }
    Ok(())
}

fn property_names(obj: &Gd<Object>) -> Vec<String> {
    let mut names = Vec::new();
    for prop in obj.get_property_list().iter_shared() {
        let Some(name_variant) = prop.get("name") else {
            continue;
        };
        if let Ok(name) = name_variant.try_to::<GString>() {
            names.push(name.to_string());
        }
    }
    names
}

fn record(
    session: &mut Session,
    tick: u64,
    entity_id: u64,
    path: &str,
    bytes: &[u8],
    preview: &mut Vec<(String, u64)>,
) -> Result<(), String> {
    session
        .hash_field(tick, entity_id, path, bytes)
        .map_err(|e| format!("hash_field failed for '{path}': {e}"))?;
    preview.push((path.to_string(), hash_bytes(bytes)));
    Ok(())
}

/// Handles the common game-math Variant types directly; anything else
/// falls back to `Variant::stringify()`'s UTF-8 bytes so an unrecognized
/// leaf type still participates in the hash rather than being silently
/// skipped.
fn leaf_bytes(value: &Variant) -> Vec<u8> {
    use godot::builtin::{Color, Vector2, Vector3, Vector4};

    if let Ok(v) = value.try_to::<bool>() {
        return vec![u8::from(v)];
    }
    if let Ok(v) = value.try_to::<i64>() {
        return v.to_le_bytes().to_vec();
    }
    if let Ok(v) = value.try_to::<f64>() {
        return v.to_le_bytes().to_vec();
    }
    if let Ok(v) = value.try_to::<GString>() {
        return v.to_string().into_bytes();
    }
    if let Ok(v) = value.try_to::<Vector2>() {
        return [v.x.to_le_bytes(), v.y.to_le_bytes()].concat();
    }
    if let Ok(v) = value.try_to::<Vector3>() {
        return [v.x.to_le_bytes(), v.y.to_le_bytes(), v.z.to_le_bytes()].concat();
    }
    if let Ok(v) = value.try_to::<Vector4>() {
        return [v.x.to_le_bytes(), v.y.to_le_bytes(), v.z.to_le_bytes(), v.w.to_le_bytes()].concat();
    }
    if let Ok(v) = value.try_to::<Color>() {
        return [v.r.to_le_bytes(), v.g.to_le_bytes(), v.b.to_le_bytes(), v.a.to_le_bytes()].concat();
    }

    value.stringify().to_string().into_bytes()
}
