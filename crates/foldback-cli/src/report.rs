// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared `.foldback` file loading + summarization, used by both the
//! `analyze` and `ci-check` subcommands so their reports never drift
//! apart from each other.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use foldback_core::bisect::{self, DivergenceTick, TickHashRecord};
use foldback_core::format::{Frame, FrameReader, Header};
use foldback_core::schema;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
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

#[derive(Debug)]
pub struct Report {
    pub header: Header,
    pub tick_hash_count: usize,
    pub entity_hash_count: usize,
    pub field_hash_count: usize,
    pub snapshot_count: usize,
    pub metadata_count: usize,
    pub ended_cleanly: bool,
    pub divergence: Option<DivergenceTick>,
}

pub fn analyze(path: &Path) -> Result<Report, CliError> {
    let path_str = path.display().to_string();
    let mut file = File::open(path).map_err(|source| CliError::Open {
        path: path_str.clone(),
        source,
    })?;
    let header = Header::read_from(&mut file).map_err(|source| CliError::Format {
        path: path_str.clone(),
        source,
    })?;

    let mut tick_records = Vec::new();
    let (mut entity_hash_count, mut field_hash_count) = (0, 0);
    let (mut snapshot_count, mut metadata_count) = (0, 0);
    let mut ended_cleanly = false;

    for frame in FrameReader::new(file) {
        let frame = frame.map_err(|source| CliError::Format {
            path: path_str.clone(),
            source,
        })?;
        match frame {
            Frame::TickHash {
                tick,
                peer_id,
                hash,
            } => tick_records.push(TickHashRecord {
                tick,
                peer_id,
                hash,
            }),
            Frame::EntityHash { .. } => entity_hash_count += 1,
            Frame::FieldHash { .. } => field_hash_count += 1,
            Frame::Snapshot { .. } => snapshot_count += 1,
            Frame::Metadata { .. } => metadata_count += 1,
            Frame::EndOfStream => ended_cleanly = true,
        }
    }

    let divergence = bisect::find_first_divergence(&tick_records);

    Ok(Report {
        tick_hash_count: tick_records.len(),
        header,
        entity_hash_count,
        field_hash_count,
        snapshot_count,
        metadata_count,
        ended_cleanly,
        divergence,
    })
}

/// Reads a `.foldback` file's recorded schema metadata (`foldback.schema.*`
/// `Metadata` frames, per `foldback_core::schema`) — the type-name ->
/// tagged-field-fingerprint map a schema-drift comparison needs.
pub fn extract_schema(path: &Path) -> Result<BTreeMap<String, String>, CliError> {
    let path_str = path.display().to_string();
    let mut file = File::open(path).map_err(|source| CliError::Open {
        path: path_str.clone(),
        source,
    })?;
    Header::read_from(&mut file).map_err(|source| CliError::Format {
        path: path_str.clone(),
        source,
    })?;

    let mut frames = Vec::new();
    for frame in FrameReader::new(file) {
        frames.push(frame.map_err(|source| CliError::Format {
            path: path_str.clone(),
            source,
        })?);
    }
    Ok(schema::extract_schemas(&frames))
}
