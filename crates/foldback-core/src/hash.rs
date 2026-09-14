// SPDX-License-Identifier: MIT OR Apache-2.0
//! xxHash3 wrapper — the one place the hash algorithm is named, so an
//! accidental swap or seed drift shows up as a one-line diff, not a
//! silent behavior change scattered across the codebase.

/// Hash a pre-serialized state blob. Callers own serialization — this
/// never allocates or does any I/O, per the hot-path design rules
/// validated in the Week 0 performance spike.
#[inline]
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pinned regression vectors: catches an accidental algorithm swap or
    // seed drift across a refactor. Values are this crate's own xxh3_64
    // output for fixed inputs, checked once and pinned here — a changed
    // value means the hash function changed, which is exactly what this
    // test exists to catch.
    #[test]
    fn known_vectors() {
        assert_eq!(hash_bytes(b""), 0x2d06_8005_38d3_94c2);
        assert_eq!(hash_bytes(b"foldback"), 0x0a7d_36d7_5a45_942d);
    }

    #[test]
    fn same_input_same_hash() {
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(hash_bytes(&data), hash_bytes(&data));
    }

    #[test]
    fn different_input_different_hash() {
        assert_ne!(hash_bytes(b"a"), hash_bytes(b"b"));
    }
}
