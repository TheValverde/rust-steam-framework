//! **Outer** game framing inside each Steam P2P message **after** the application handshake completes.
//!
//! Every framed game datagram is:
//!
//! ```text
//! [ u32 LE inner_len ][ inner_len bytes ]
//! ```
//!
//! [`crate::SteamMultiplayer`] uses these helpers for send and recv. Games may also call them for tests or alternate
//! transports.
#![cfg_attr(not(feature = "steam"), allow(dead_code))]

/// Validate `data` is exactly `4 + inner_len` bytes and `inner_len <= max_inner`; return the inner slice.
pub fn strip_length_prefix(data: &[u8], max_inner: usize) -> Option<&[u8]> {
    if data.len() < 4 {
        return None;
    }
    let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if len > max_inner || data.len() != 4 + len {
        return None;
    }
    Some(&data[4..])
}

/// Allocate `4 + inner.len()` bytes: length as `u32` LE then `inner`. Returns `None` if `inner.len() > max_inner`.
pub fn prepend_length_prefix(inner: &[u8], max_inner: usize) -> Option<Vec<u8>> {
    if inner.len() > max_inner {
        return None;
    }
    let len = inner.len() as u32;
    let mut buf = Vec::with_capacity(4 + inner.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(inner);
    Some(buf)
}
