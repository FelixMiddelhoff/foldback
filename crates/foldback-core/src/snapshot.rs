// SPDX-License-Identifier: MIT OR Apache-2.0
//! zstd compression for full-state snapshots. Explicitly off the per-tick
//! hot path (per foldback-performance-plan.md §2 and the Week 0 spike's
//! secondary-metric findings) — used only at the configured snapshot
//! cadence, not every tick.

use crate::Error;

pub fn compress(bytes: &[u8], level: i32) -> Result<Vec<u8>, Error> {
    zstd::encode_all(bytes, level).map_err(Error::Io)
}

pub fn decompress(compressed: &[u8]) -> Result<Vec<u8>, Error> {
    zstd::decode_all(compressed).map_err(|_| Error::CorruptSnapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) {
        let compressed = compress(data, 3).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn round_trip_empty() {
        round_trip(&[]);
    }

    #[test]
    fn round_trip_tiny() {
        round_trip(b"hi");
    }

    #[test]
    fn round_trip_large() {
        let data: Vec<u8> = (0..5_000_000u32).map(|i| (i % 256) as u8).collect();
        round_trip(&data);
    }

    #[test]
    fn corrupted_data_errors_not_panics() {
        let garbage = vec![0xFFu8; 64];
        let result = decompress(&garbage);
        assert!(result.is_err());
    }

    #[test]
    fn truncated_valid_stream_errors_not_panics() {
        let compressed = compress(b"some real data to compress here", 3).unwrap();
        let truncated = &compressed[..compressed.len() / 2];
        let result = decompress(truncated);
        assert!(result.is_err());
    }
}
