// SPDX-License-Identifier: MIT OR Apache-2.0
//! `foldback lint --engine bevy <path>` (foldback-reflective-hashing.md
//! §4's visibility tooling): a static scan for `#[derive(FoldbackHash)]`
//! structs, listing which fields are tracked (`#[foldback(hash)]`) and
//! which sibling fields on the same struct aren't — the "did you mean to
//! include this one too" question the opt-in default deliberately
//! doesn't answer automatically. Bevy/Rust only for now, matching the
//! reflective-hashing plan's Bevy-first rollout order (§6); Unity/Godot
//! scanners are a separate, not-yet-started piece of that same plan.

use std::path::{Path, PathBuf};

use syn::{Fields, Item};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct TrackedType {
    pub name: String,
    pub file: PathBuf,
    pub tracked: Vec<String>,
    pub untracked: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LintError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: syn::Error,
    },
}

/// Scans every `.rs` file under `root` for `#[derive(FoldbackHash)]`
/// structs. A file that fails to parse (e.g. it uses syntax this `syn`
/// version doesn't understand) is reported as an error rather than
/// silently skipped — a scanner that quietly misses a file is worse than
/// no scanner, for a tool whose whole point is visibility.
pub fn scan_bevy(root: &Path) -> Result<Vec<TrackedType>, LintError> {
    let mut results = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.path();
        let content = std::fs::read_to_string(path).map_err(|source| LintError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let file = syn::parse_file(&content).map_err(|source| LintError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

        collect_tracked_structs(&file.items, path, &mut results);
    }

    results.sort_by(|a, b| (&a.file, &a.name).cmp(&(&b.file, &b.name)));
    Ok(results)
}

/// Recurses into `mod foo { ... }` (inline modules) so a type nested a
/// module or two deep — `mod tests { struct Unit { .. } }`, common for
/// this project's own test fixtures — isn't invisible to the scan.
/// Deliberately doesn't descend into function bodies: a struct declared
/// local to a function isn't the kind of reusable, reflected component
/// type this tool exists to make visible.
fn collect_tracked_structs(items: &[Item], path: &Path, out: &mut Vec<TrackedType>) {
    for item in items {
        match item {
            Item::Struct(item_struct) => {
                if !derives_foldback_hash(&item_struct.attrs) {
                    continue;
                }
                let Fields::Named(fields) = &item_struct.fields else {
                    continue;
                };

                let mut tracked = Vec::new();
                let mut untracked = Vec::new();
                for field in &fields.named {
                    let name = field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    match field_marking(&field.attrs) {
                        FieldMarking::Tracked => tracked.push(name),
                        FieldMarking::Skip => {}
                        FieldMarking::Unmarked => untracked.push(name),
                    }
                }

                out.push(TrackedType {
                    name: item_struct.ident.to_string(),
                    file: path.to_path_buf(),
                    tracked,
                    untracked,
                });
            }
            Item::Mod(item_mod) => {
                if let Some((_, inner_items)) = &item_mod.content {
                    collect_tracked_structs(inner_items, path, out);
                }
            }
            _ => {}
        }
    }
}

enum FieldMarking {
    /// `#[foldback(hash)]` or `#[foldback(reflect)]` — both are tracked;
    /// they differ only in *how* they're hashed (manual `FieldBytes` vs
    /// a reflective walker), which is invisible from a static scan and
    /// irrelevant to "is this field tracked at all."
    Tracked,
    Skip,
    Unmarked,
}

/// Mirrors `foldback-derive`'s own `field_marking` — this is a read-only
/// static scan of the same attribute syntax the derive macro consumes at
/// compile time, kept deliberately small rather than sharing a
/// proc-macro-only crate as a dependency here.
fn field_marking(attrs: &[syn::Attribute]) -> FieldMarking {
    for attr in attrs {
        if !attr.path().is_ident("foldback") {
            continue;
        }
        if let Ok(ident) = attr.parse_args::<syn::Ident>() {
            return match ident.to_string().as_str() {
                "hash" | "reflect" => FieldMarking::Tracked,
                "skip" => FieldMarking::Skip,
                _ => FieldMarking::Unmarked,
            };
        }
    }
    FieldMarking::Unmarked
}

fn derives_foldback_hash(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let Ok(paths) =
            attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
        else {
            continue;
        };
        if paths.iter().any(|p| p.is_ident("FoldbackHash")) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tracked_and_untracked_fields_on_a_tagged_struct() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("unit.rs"),
            r#"
            #[derive(Reflect, FoldbackHash)]
            struct Unit {
                #[foldback(hash)]
                pos: Position,
                #[foldback(skip)]
                debug_only: String,
                hp: i32,
            }
            "#,
        )
        .unwrap();

        let types = scan_bevy(dir.path()).unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "Unit");
        assert_eq!(types[0].tracked, vec!["pos".to_string()]);
        assert_eq!(types[0].untracked, vec!["hp".to_string()]);
    }

    #[test]
    fn finds_tagged_structs_nested_inside_a_mod_block() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("nested.rs"),
            r#"
            #[cfg(test)]
            mod tests {
                #[derive(Reflect, FoldbackHash)]
                struct Unit {
                    #[foldback(hash)]
                    hp: i32,
                }
            }
            "#,
        )
        .unwrap();

        let types = scan_bevy(dir.path()).unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "Unit");
    }

    #[test]
    fn ignores_structs_without_the_derive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("plain.rs"),
            r#"
            struct NotTracked {
                x: f32,
            }
            "#,
        )
        .unwrap();

        let types = scan_bevy(dir.path()).unwrap();
        assert!(types.is_empty());
    }

    #[test]
    fn reports_a_parse_error_instead_of_silently_skipping_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("broken.rs"), "this is not valid rust {{{").unwrap();

        let err = scan_bevy(dir.path()).unwrap_err();
        assert!(matches!(err, LintError::Parse { .. }));
    }
}
