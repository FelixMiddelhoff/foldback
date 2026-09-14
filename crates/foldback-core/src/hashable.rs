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
}
