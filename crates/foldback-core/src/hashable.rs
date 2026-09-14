// SPDX-License-Identifier: MIT OR Apache-2.0
//! Per-field hashing support (cookbook recipe 5): the [`FoldbackHash`]
//! trait a `#[derive(FoldbackHash)]`'d struct implements, and the small
//! [`FieldBytes`] trait that turns one field's value into the bytes
//! [`crate::session::Session::hash_field`] wants — deliberately scoped to
//! primitives and fixed-size arrays of them (covers the common game-math
//! case, e.g. a `[f32; 3]` position), not a general serialization
//! framework.
//!
//! **Decided: opt-in.** `#[derive(FoldbackHash)]` hashes nothing until a
//! field is explicitly marked `#[foldback(hash)]` — an unmarked field is
//! simply not hashed, never a compile error, so a struct with a
//! genuinely non-deterministic field (a timestamp, a debug label) can't
//! silently become a phantom divergence source just by existing on a
//! `#[derive(FoldbackHash)]` struct. See `FoldbackHash::UNTRACKED_FIELDS`
//! for how an unmarked field stays *visible* rather than silently
//! assumed either way.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::session::Session;
use crate::Error;

/// A struct with `#[derive(FoldbackHash)]` implements this — normally
/// generated, not written by hand. Call it via
/// [`Session::hash_fields`] (`session.hash_fields(tick, entity_id,
/// &value)`), not directly.
pub trait FoldbackHash {
    /// Names of fields present on the struct but not marked
    /// `#[foldback(hash)]` or `#[foldback(skip)]` — an unmarked field is
    /// still not hashed (opt-in stays opt-in), but its name stays here
    /// so a human or a future `foldback-cli lint` can see what was left
    /// out, rather than it disappearing silently either way.
    const UNTRACKED_FIELDS: &'static [&'static str];

    #[doc(hidden)]
    fn write_hashed_fields(
        &self,
        session: &mut Session,
        tick: u64,
        entity_id: u64,
    ) -> Result<(), Error>;
}

/// Converts one field's value into the bytes [`Session::hash_field`]
/// hashes and records. Implemented for the common fixed-size numeric
/// types directly; implement it yourself for a custom type (a `Vec3`
/// newtype, say) if `#[foldback(hash)]` needs to mark a field of that
/// type.
pub trait FieldBytes {
    fn field_bytes(&self) -> Vec<u8>;
}

macro_rules! impl_field_bytes_le_bytes {
    ($($t:ty),* $(,)?) => {
        $(
            impl FieldBytes for $t {
                fn field_bytes(&self) -> Vec<u8> {
                    self.to_le_bytes().to_vec()
                }
            }
        )*
    };
}

impl_field_bytes_le_bytes!(f32, f64, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

impl FieldBytes for bool {
    fn field_bytes(&self) -> Vec<u8> {
        vec![u8::from(*self)]
    }
}

impl<T: FieldBytes, const N: usize> FieldBytes for [T; N] {
    fn field_bytes(&self) -> Vec<u8> {
        self.iter().flat_map(FieldBytes::field_bytes).collect()
    }
}

// Shared reflective-hashing rule (foldback-reflective-hashing.md §3, §6
// step 1): an unordered container's iteration order isn't guaranteed
// across platforms/runs, so hashing it in iteration order would produce
// a platform-dependent hash — a false divergence with nothing actually
// wrong. Every engine's reflective walker bottoms out at these same
// `FieldBytes` impls, so the sort-by-key rule only has to be correct
// once, here, instead of three times per binding.

impl<K: FieldBytes + Ord, V: FieldBytes> FieldBytes for HashMap<K, V> {
    fn field_bytes(&self) -> Vec<u8> {
        let mut entries: Vec<(&K, &V)> = self.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries
            .into_iter()
            .flat_map(|(k, v)| k.field_bytes().into_iter().chain(v.field_bytes()))
            .collect()
    }
}

impl<K: FieldBytes + Ord, V: FieldBytes> FieldBytes for BTreeMap<K, V> {
    fn field_bytes(&self) -> Vec<u8> {
        // Already key-ordered, but re-sorting costs nothing here and
        // keeps this impl correct even if `Ord`'s ordering ever diverges
        // from `BTreeMap`'s internal comparator for some `K`.
        let mut entries: Vec<(&K, &V)> = self.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries
            .into_iter()
            .flat_map(|(k, v)| k.field_bytes().into_iter().chain(v.field_bytes()))
            .collect()
    }
}

impl<T: FieldBytes + Ord> FieldBytes for HashSet<T> {
    fn field_bytes(&self) -> Vec<u8> {
        let mut items: Vec<&T> = self.iter().collect();
        items.sort();
        items.into_iter().flat_map(FieldBytes::field_bytes).collect()
    }
}

impl<T: FieldBytes + Ord> FieldBytes for BTreeSet<T> {
    fn field_bytes(&self) -> Vec<u8> {
        let mut items: Vec<&T> = self.iter().collect();
        items.sort();
        items.into_iter().flat_map(FieldBytes::field_bytes).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_field_bytes_match_to_le_bytes() {
        assert_eq!(1.5f32.field_bytes(), 1.5f32.to_le_bytes().to_vec());
        assert_eq!(42u64.field_bytes(), 42u64.to_le_bytes().to_vec());
        assert_eq!(true.field_bytes(), vec![1u8]);
        assert_eq!(false.field_bytes(), vec![0u8]);
    }

    #[test]
    fn fixed_array_field_bytes_concatenates_each_element() {
        let v: [f32; 3] = [1.0, 2.0, 3.0];
        let mut expected = Vec::new();
        for x in v {
            expected.extend_from_slice(&x.to_le_bytes());
        }
        assert_eq!(v.field_bytes(), expected);
    }

    // Conformance test for foldback-reflective-hashing.md §3's
    // sorted-container rule: insertion order must never affect the
    // resulting bytes, for every unordered container type the reflective
    // walkers will eventually feed through `FieldBytes`.
    #[test]
    fn hashmap_field_bytes_independent_of_insertion_order() {
        let mut a: HashMap<u32, f32> = HashMap::new();
        a.insert(3, 3.0);
        a.insert(1, 1.0);
        a.insert(2, 2.0);

        let mut b: HashMap<u32, f32> = HashMap::new();
        b.insert(2, 2.0);
        b.insert(3, 3.0);
        b.insert(1, 1.0);

        assert_eq!(a.field_bytes(), b.field_bytes());
    }

    #[test]
    fn hashmap_field_bytes_matches_sorted_btreemap() {
        let mut hm: HashMap<u32, u8> = HashMap::new();
        hm.insert(5, 50);
        hm.insert(1, 10);
        hm.insert(3, 30);

        let bm: BTreeMap<u32, u8> = hm.iter().map(|(k, v)| (*k, *v)).collect();

        assert_eq!(hm.field_bytes(), bm.field_bytes());
    }

    #[test]
    fn hashset_field_bytes_independent_of_insertion_order() {
        let a: HashSet<u32> = [3, 1, 4, 1, 5, 9].into_iter().collect();
        let b: HashSet<u32> = [9, 5, 1, 4, 3].into_iter().collect();
        assert_eq!(a.field_bytes(), b.field_bytes());
    }

    #[test]
    fn btreeset_field_bytes_matches_hashset() {
        let hs: HashSet<u32> = [3, 1, 4, 1, 5, 9].into_iter().collect();
        let bs: BTreeSet<u32> = hs.iter().copied().collect();
        assert_eq!(hs.field_bytes(), bs.field_bytes());
    }

    #[test]
    fn empty_hashmap_field_bytes_is_empty() {
        let m: HashMap<u32, u32> = HashMap::new();
        assert!(m.field_bytes().is_empty());
    }
}
