// SPDX-License-Identifier: MIT OR Apache-2.0
//! Loads a `.foldback` file into the flat, JSON-serializable shape the
//! frontend renders. Read-only, offline — no live connection in v1.

use std::fs::File;
use std::path::Path;

use foldback_core::bisect::{self, DivergenceTick, TickHashRecord};
use foldback_core::format::{Frame, FrameReader, Header};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("could not open '{path}': {source}")]
    Open {
        path: String,
        source: std::io::Error,
    },
    #[error("failed reading '{path}': {source}")]
    Format {
        path: String,
        source: foldback_core::Error,
    },
}

impl serde::Serialize for LoadError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct TickHashEntry {
    pub tick: u64,
    pub peer_id: u16,
    pub hash: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct EntityHashEntry {
    pub tick: u64,
    pub peer_id: u16,
    pub entity_id: u64,
    pub hash: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FieldHashEntry {
    pub tick: u64,
    pub peer_id: u16,
    pub entity_id: u64,
    pub field_name: String,
    pub hash: String,
    /// Raw field value bytes, hex-encoded — the UI shows these when
    /// bisecting a divergence down to the differing field.
    pub value_hex: String,
}

#[derive(Debug, Serialize)]
pub struct SessionData {
    pub file_name: String,
    pub tick_rate_hz: u32,
    pub peer_count: u32,
    pub build_id: String,
    pub min_tick: u64,
    pub max_tick: u64,
    pub ended_cleanly: bool,
    pub tick_hashes: Vec<TickHashEntry>,
    pub entity_hashes: Vec<EntityHashEntry>,
    pub field_hashes: Vec<FieldHashEntry>,
    pub snapshot_ticks: Vec<u64>,
    /// `.foldback` metadata frames carry no tick — Level 1 only lets the
    /// file attach key/value pairs, not a timeline position — so this is a
    /// count for the summary panel, not a set of timeline markers.
    pub metadata_count: usize,
    pub divergence_tick: Option<u64>,
}

fn hex(bytes: u64) -> String {
    format!("{bytes:016x}")
}

pub fn load(path: &Path) -> Result<SessionData, LoadError> {
    let path_str = path.display().to_string();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.clone());

    let mut file = File::open(path).map_err(|source| LoadError::Open {
        path: path_str.clone(),
        source,
    })?;
    let header = Header::read_from(&mut file).map_err(|source| LoadError::Format {
        path: path_str.clone(),
        source,
    })?;

    let mut tick_records: Vec<TickHashRecord> = Vec::new();
    let mut tick_hashes = Vec::new();
    let mut entity_hashes = Vec::new();
    let mut field_hashes = Vec::new();
    let mut snapshot_ticks = Vec::new();
    let mut metadata_count = 0usize;
    let mut ended_cleanly = false;
    let (mut min_tick, mut max_tick) = (u64::MAX, 0u64);

    for frame in FrameReader::new(file) {
        let frame = frame.map_err(|source| LoadError::Format {
            path: path_str.clone(),
            source,
        })?;
        match frame {
            Frame::TickHash {
                tick,
                peer_id,
                hash,
            } => {
                min_tick = min_tick.min(tick);
                max_tick = max_tick.max(tick);
                tick_records.push(TickHashRecord {
                    tick,
                    peer_id,
                    hash,
                });
                tick_hashes.push(TickHashEntry {
                    tick,
                    peer_id,
                    hash: hex(hash),
                });
            }
            Frame::EntityHash {
                tick,
                peer_id,
                entity_id,
                hash,
            } => entity_hashes.push(EntityHashEntry {
                tick,
                peer_id,
                entity_id,
                hash: hex(hash),
            }),
            Frame::FieldHash {
                tick,
                peer_id,
                entity_id,
                field_name,
                hash,
                value,
            } => field_hashes.push(FieldHashEntry {
                tick,
                peer_id,
                entity_id,
                field_name,
                hash: hex(hash),
                value_hex: value.iter().map(|b| format!("{b:02x}")).collect(),
            }),
            Frame::Snapshot { tick, .. } => snapshot_ticks.push(tick),
            Frame::Metadata { .. } => metadata_count += 1,
            Frame::EndOfStream => ended_cleanly = true,
        }
    }

    let DivergenceTick(divergence_tick) = match bisect::find_first_divergence(&tick_records) {
        Some(d) => d,
        None => {
            return Ok(SessionData {
                file_name,
                tick_rate_hz: header.tick_rate_hz,
                peer_count: header.peer_count,
                build_id: header.build_id.iter().map(|b| format!("{b:02x}")).collect(),
                min_tick: if min_tick == u64::MAX { 0 } else { min_tick },
                max_tick,
                ended_cleanly,
                tick_hashes,
                entity_hashes,
                field_hashes,
                snapshot_ticks,
                metadata_count,
                divergence_tick: None,
            })
        }
    };
    snapshot_ticks.sort_unstable();
    snapshot_ticks.dedup();

    Ok(SessionData {
        file_name,
        tick_rate_hz: header.tick_rate_hz,
        peer_count: header.peer_count,
        build_id: header.build_id.iter().map(|b| format!("{b:02x}")).collect(),
        min_tick: if min_tick == u64::MAX { 0 } else { min_tick },
        max_tick,
        ended_cleanly,
        tick_hashes,
        entity_hashes,
        field_hashes,
        snapshot_ticks,
        metadata_count,
        divergence_tick: Some(divergence_tick),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../foldback-cli/tests/fixtures")
            .join(name)
    }

    #[test]
    fn loads_clean_fixture_with_no_divergence() {
        let data = load(&fixture("clean.foldback")).unwrap();
        assert!(data.divergence_tick.is_none());
        assert!(!data.tick_hashes.is_empty());
    }

    #[test]
    fn loads_diverging_fixture_and_finds_divergence() {
        let data = load(&fixture("diverging.foldback")).unwrap();
        assert!(data.divergence_tick.is_some());
    }

    #[test]
    fn loads_truncated_fixture_without_error() {
        let data = load(&fixture("truncated.foldback")).unwrap();
        assert!(!data.ended_cleanly);
    }
}
