//! Fixed **15-byte** prefix for **inner** game packets: version, kind, tick, and embedded payload length.
//!
//! This is **optional**: [`crate::SteamMultiplayer`] only sees `u32` length + bytes; you may embed this header at the
//! start of those bytes so peers can drop stale or irrelevant packets **without** running an expensive deserializer on
//! large bodies (see crate `README.md` diagram “C”).
//!
//! Layout (little-endian integers):
//!
//! ```text
//! [ u16 version ][ u8 kind ][ u64 tick ][ u32 payload_len ][ payload_len bytes... ]
//! ```
//!
//! Total size = [`HEADER_LEN`] + `payload_len`. [`parse`] requires the slice length to match exactly.

/// Byte length of the on-wire header (before the variable-length payload).
pub const HEADER_LEN: usize = 15;

/// Parsed view of the fixed header; `payload_len` is the length of the trailing opaque region only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireHeader {
    /// Application protocol version (game-defined).
    pub version: u16,
    /// Message category for cheap routing / coalescing (game-defined).
    pub kind: u8,
    /// Sim or snapshot tick (game-defined; may be `0` for non-tick messages).
    pub tick: u64,
    /// Length in bytes of the opaque trailer after this 15-byte header.
    pub payload_len: usize,
}

/// Parse and validate a full packet `header || payload`. Returns `None` if truncated, length mismatch, or
/// `payload_len > max_payload`.
pub fn parse(bytes: &[u8], max_payload: usize) -> Option<WireHeader> {
    if bytes.len() < HEADER_LEN {
        return None;
    }
    let version = u16::from_le_bytes(bytes[0..2].try_into().ok()?);
    let kind = bytes[2];
    let tick = u64::from_le_bytes(bytes[3..11].try_into().ok()?);
    let payload_len = u32::from_le_bytes(bytes[11..15].try_into().ok()?) as usize;
    if payload_len > max_payload {
        return None;
    }
    if bytes.len() != HEADER_LEN + payload_len {
        return None;
    }
    Some(WireHeader {
        version,
        kind,
        tick,
        payload_len,
    })
}

/// Allocate `header || payload`. Returns `None` if `payload.len() > max_payload`.
pub fn build_frame(version: u16, kind: u8, tick: u64, payload: &[u8], max_payload: usize) -> Option<Vec<u8>> {
    if payload.len() > max_payload {
        return None;
    }
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&version.to_le_bytes());
    out.push(kind);
    out.extend_from_slice(&tick.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    Some(out)
}
