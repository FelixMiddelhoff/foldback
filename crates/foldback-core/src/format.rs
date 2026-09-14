//! `.foldback` session file format: fixed 32-byte header, then a stream of
//! length-prefixed frames. Append-only, streaming-writable, tolerant of
//! truncation — a crash mid-recording must still yield a parseable prefix.
//!
//! Per foldback-protocol-spec.md §1. `format_version` bumps only on a
//! breaking change to frame layout; new frame types can be added without a
//! version bump as long as readers skip unknown types by length.

use std::io::{self, Read, Write};

use crate::Error;

pub const MAGIC: &[u8; 4] = b"FBK1";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_LEN: usize = 32;

/// Fixed 32-byte session file header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub format_version: u16,
    pub tick_rate_hz: u32,
    pub peer_count: u32,
    /// Opaque, game-supplied build identifier (e.g. a hash of the game
    /// build) — lets a reader warn "this session was recorded against a
    /// different build than the one you're diffing against."
    pub build_id: [u8; 16],
}

impl Header {
    pub fn new(tick_rate_hz: u32, peer_count: u32, build_id: [u8; 16]) -> Self {
        Header {
            format_version: FORMAT_VERSION,
            tick_rate_hz,
            peer_count,
            build_id,
        }
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<(), Error> {
        let mut buf = [0u8; HEADER_LEN];
        buf[0..4].copy_from_slice(MAGIC);
        buf[4..6].copy_from_slice(&self.format_version.to_le_bytes());
        // bytes 6..8 (flags): reserved, must be 0 in v1 — left zeroed.
        buf[8..12].copy_from_slice(&self.tick_rate_hz.to_le_bytes());
        buf[12..16].copy_from_slice(&self.peer_count.to_le_bytes());
        buf[16..32].copy_from_slice(&self.build_id);
        w.write_all(&buf)?;
        Ok(())
    }

    pub fn read_from<R: Read>(r: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; HEADER_LEN];
        r.read_exact(&mut buf).map_err(|_| Error::TruncatedHeader)?;
        if &buf[0..4] != MAGIC {
            return Err(Error::BadMagic);
        }
        let format_version = u16::from_le_bytes([buf[4], buf[5]]);
        let tick_rate_hz = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let peer_count = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        let mut build_id = [0u8; 16];
        build_id.copy_from_slice(&buf[16..32]);
        Ok(Header {
            format_version,
            tick_rate_hz,
            peer_count,
            build_id,
        })
    }
}

/// A single frame in the stream. See protocol spec §1.2 for the wire
/// layout of each variant's payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    TickHash {
        tick: u64,
        peer_id: u16,
        hash: u64,
    },
    EntityHash {
        tick: u64,
        peer_id: u16,
        entity_id: u64,
        hash: u64,
    },
    FieldHash {
        tick: u64,
        peer_id: u16,
        entity_id: u64,
        field_name: String,
        hash: u64,
        value: Vec<u8>,
    },
    Snapshot {
        tick: u64,
        peer_id: u16,
        compressed: Vec<u8>,
    },
    Metadata {
        key: String,
        value: String,
    },
    EndOfStream,
}

const TYPE_TICK_HASH: u8 = 0x01;
const TYPE_ENTITY_HASH: u8 = 0x02;
const TYPE_FIELD_HASH: u8 = 0x03;
const TYPE_SNAPSHOT: u8 = 0x10;
const TYPE_METADATA: u8 = 0x20;
const TYPE_END_OF_STREAM: u8 = 0xFF;

impl Frame {
    fn frame_type(&self) -> u8 {
        match self {
            Frame::TickHash { .. } => TYPE_TICK_HASH,
            Frame::EntityHash { .. } => TYPE_ENTITY_HASH,
            Frame::FieldHash { .. } => TYPE_FIELD_HASH,
            Frame::Snapshot { .. } => TYPE_SNAPSHOT,
            Frame::Metadata { .. } => TYPE_METADATA,
            Frame::EndOfStream => TYPE_END_OF_STREAM,
        }
    }

    fn encode_payload(&self) -> Vec<u8> {
        let mut p = Vec::new();
        match self {
            Frame::TickHash {
                tick,
                peer_id,
                hash,
            } => {
                p.extend_from_slice(&tick.to_le_bytes());
                p.extend_from_slice(&peer_id.to_le_bytes());
                p.extend_from_slice(&hash.to_le_bytes());
            }
            Frame::EntityHash {
                tick,
                peer_id,
                entity_id,
                hash,
            } => {
                p.extend_from_slice(&tick.to_le_bytes());
                p.extend_from_slice(&peer_id.to_le_bytes());
                p.extend_from_slice(&entity_id.to_le_bytes());
                p.extend_from_slice(&hash.to_le_bytes());
            }
            Frame::FieldHash {
                tick,
                peer_id,
                entity_id,
                field_name,
                hash,
                value,
            } => {
                p.extend_from_slice(&tick.to_le_bytes());
                p.extend_from_slice(&peer_id.to_le_bytes());
                p.extend_from_slice(&entity_id.to_le_bytes());
                p.extend_from_slice(&(field_name.len() as u32).to_le_bytes());
                p.extend_from_slice(field_name.as_bytes());
                p.extend_from_slice(&hash.to_le_bytes());
                p.extend_from_slice(&(value.len() as u32).to_le_bytes());
                p.extend_from_slice(value);
            }
            Frame::Snapshot {
                tick,
                peer_id,
                compressed,
            } => {
                p.extend_from_slice(&tick.to_le_bytes());
                p.extend_from_slice(&peer_id.to_le_bytes());
                p.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
                p.extend_from_slice(compressed);
            }
            Frame::Metadata { key, value } => {
                p.extend_from_slice(&(key.len() as u32).to_le_bytes());
                p.extend_from_slice(key.as_bytes());
                p.extend_from_slice(&(value.len() as u32).to_le_bytes());
                p.extend_from_slice(value.as_bytes());
            }
            Frame::EndOfStream => {}
        }
        p
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<(), Error> {
        let payload = self.encode_payload();
        w.write_all(&[self.frame_type()])?;
        w.write_all(&(payload.len() as u32).to_le_bytes())?;
        w.write_all(&payload)?;
        Ok(())
    }

    fn decode_payload(frame_type: u8, payload: &[u8]) -> Result<Self, Error> {
        let mut c = Cursor {
            buf: payload,
            pos: 0,
        };
        Ok(match frame_type {
            TYPE_TICK_HASH => Frame::TickHash {
                tick: c.take_u64()?,
                peer_id: c.take_u16()?,
                hash: c.take_u64()?,
            },
            TYPE_ENTITY_HASH => Frame::EntityHash {
                tick: c.take_u64()?,
                peer_id: c.take_u16()?,
                entity_id: c.take_u64()?,
                hash: c.take_u64()?,
            },
            TYPE_FIELD_HASH => {
                let tick = c.take_u64()?;
                let peer_id = c.take_u16()?;
                let entity_id = c.take_u64()?;
                let field_name = c.take_string()?;
                let hash = c.take_u64()?;
                let value = c.take_bytes()?;
                Frame::FieldHash {
                    tick,
                    peer_id,
                    entity_id,
                    field_name,
                    hash,
                    value,
                }
            }
            TYPE_SNAPSHOT => Frame::Snapshot {
                tick: c.take_u64()?,
                peer_id: c.take_u16()?,
                compressed: c.take_bytes()?,
            },
            TYPE_METADATA => {
                let key = c.take_string()?;
                let value = c.take_string()?;
                Frame::Metadata { key, value }
            }
            TYPE_END_OF_STREAM => Frame::EndOfStream,
            other => return Err(Error::UnknownFrameType(other)),
        })
    }
}

/// Minimal cursor over an in-memory payload, used only during decode of an
/// already-length-delimited frame — never reads past `payload`.
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.pos + n > self.buf.len() {
            return Err(Error::MalformedFrame);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn take_u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn take_u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn take_u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn take_bytes(&mut self) -> Result<Vec<u8>, Error> {
        let len = self.take_u32()? as usize;
        Ok(self.take(len)?.to_vec())
    }
    fn take_string(&mut self) -> Result<String, Error> {
        let bytes = self.take_bytes()?;
        String::from_utf8(bytes).map_err(|_| Error::MalformedFrame)
    }
}

/// Streaming frame reader. Per protocol spec §1.2: a reader that hits EOF
/// mid-frame (impossible `payload_len`, or a short read) discards that
/// partial frame and stops — everything before it is still valid. Also
/// per §1.3: an unrecognized frame type is skipped by length, never an
/// error, so old readers stay forward-compatible with new frame types.
pub struct FrameReader<R: Read> {
    inner: R,
}

impl<R: Read> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        FrameReader { inner }
    }

    /// Returns `Ok(Some(frame))` for the next frame, `Ok(None)` at a clean
    /// end (EOF exactly on a frame boundary, or after `EndOfStream`), and
    /// never an `Err` for truncation — truncation just ends iteration.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, Error> {
        let mut type_buf = [0u8; 1];
        match self.inner.read_exact(&mut type_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        let mut len_buf = [0u8; 4];
        if self.inner.read_exact(&mut len_buf).is_err() {
            return Ok(None); // truncated mid-header — discard, stop.
        }
        let payload_len = u32::from_le_bytes(len_buf) as usize;
        let mut payload = vec![0u8; payload_len];
        if self.inner.read_exact(&mut payload).is_err() {
            return Ok(None); // truncated mid-payload — discard, stop.
        }
        match Frame::decode_payload(type_buf[0], &payload) {
            Ok(frame) => Ok(Some(frame)),
            // Unknown type: already consumed exactly `payload_len` bytes
            // above (that's the point of length-prefixing), so just skip
            // and continue rather than erroring.
            Err(Error::UnknownFrameType(_)) => self.next_frame(),
            Err(e) => Err(e),
        }
    }
}

impl<R: Read> Iterator for FrameReader<R> {
    type Item = Result<Frame, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_frame() {
            Ok(Some(f)) => Some(Ok(f)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor as IoCursor;

    fn sample_frames() -> Vec<Frame> {
        vec![
            Frame::TickHash {
                tick: 1,
                peer_id: 0,
                hash: 0xdead_beef,
            },
            Frame::EntityHash {
                tick: 2,
                peer_id: 1,
                entity_id: 42,
                hash: 0xfeed_face,
            },
            Frame::FieldHash {
                tick: 3,
                peer_id: 0,
                entity_id: 7,
                field_name: "velocity.x".to_string(),
                hash: 0x1234_5678,
                value: vec![0x40, 0x49, 0x0f, 0xdb],
            },
            Frame::Snapshot {
                tick: 4,
                peer_id: 0,
                compressed: vec![1, 2, 3, 4, 5],
            },
            Frame::Metadata {
                key: "input:P1".to_string(),
                value: "jump".to_string(),
            },
            Frame::EndOfStream,
        ]
    }

    #[test]
    fn header_round_trip() {
        let header = Header::new(60, 2, [7u8; 16]);
        let mut buf = Vec::new();
        header.write_to(&mut buf).unwrap();
        assert_eq!(buf.len(), HEADER_LEN);
        let read_back = Header::read_from(&mut IoCursor::new(buf)).unwrap();
        assert_eq!(header, read_back);
    }

    #[test]
    fn header_rejects_bad_magic() {
        let buf = [0u8; HEADER_LEN];
        assert!(matches!(
            Header::read_from(&mut IoCursor::new(buf)),
            Err(Error::BadMagic)
        ));
    }

    #[test]
    fn header_rejects_truncated() {
        let buf = [0u8; 10];
        assert!(matches!(
            Header::read_from(&mut IoCursor::new(buf)),
            Err(Error::TruncatedHeader)
        ));
    }

    #[test]
    fn every_frame_type_round_trips() {
        for frame in sample_frames() {
            let mut buf = Vec::new();
            frame.write_to(&mut buf).unwrap();
            let mut reader = FrameReader::new(IoCursor::new(buf));
            let read_back = reader.next_frame().unwrap().unwrap();
            assert_eq!(frame, read_back);
            assert_eq!(reader.next_frame().unwrap(), None);
        }
    }

    #[test]
    fn full_stream_round_trips_in_order() {
        let frames = sample_frames();
        let mut buf = Vec::new();
        for f in &frames {
            f.write_to(&mut buf).unwrap();
        }
        let reader = FrameReader::new(IoCursor::new(buf));
        let read_back: Result<Vec<_>, _> = reader.collect();
        assert_eq!(read_back.unwrap(), frames);
    }

    #[test]
    fn truncated_file_yields_all_complete_frames_before_the_cut() {
        let frames = sample_frames();
        let mut buf = Vec::new();
        for f in &frames {
            f.write_to(&mut buf).unwrap();
        }
        // Simulate a crash mid-recording: cut off partway through the
        // last frame's payload, well past all earlier complete frames.
        let cut = buf.len() - 2;
        buf.truncate(cut);

        let reader = FrameReader::new(IoCursor::new(buf));
        let read_back: Vec<Frame> = reader.filter_map(|r| r.ok()).collect();
        // Every frame except the truncated last one must still parse.
        assert_eq!(read_back, frames[..frames.len() - 1]);
    }

    #[test]
    fn truncated_immediately_after_frame_boundary_yields_nothing_more() {
        let frames = sample_frames();
        let mut buf = Vec::new();
        for f in &frames[..2] {
            f.write_to(&mut buf).unwrap();
        }
        // No partial trailing bytes — a clean cut exactly on a boundary.
        let reader = FrameReader::new(IoCursor::new(buf));
        let read_back: Result<Vec<_>, _> = reader.collect();
        assert_eq!(read_back.unwrap(), frames[..2]);
    }

    #[test]
    fn unknown_frame_type_is_skipped_not_errored() {
        let mut buf = Vec::new();
        // A well-formed but unrecognized frame type (0x7E), skippable by length.
        buf.push(0x7E);
        let payload = b"anything, any length";
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(payload);
        // Followed by a real, recognized frame.
        Frame::EndOfStream.write_to(&mut buf).unwrap();

        let reader = FrameReader::new(IoCursor::new(buf));
        let read_back: Result<Vec<_>, _> = reader.collect();
        assert_eq!(read_back.unwrap(), vec![Frame::EndOfStream]);
    }
}
