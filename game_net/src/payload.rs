/// One **inner** game payload after Steam’s message body has been validated as `u32 LE len || bytes` and the length
/// wrapper stripped.
///
/// The bytes are **opaque** to this crate: your game decodes them (optionally after peeling [`crate::wire_header`]).
#[derive(Debug)]
pub struct FramedPayload {
    /// **Client:** always `0` (single host connection). **Host:** index into the internal peer list **among
    /// handshaken** connections, matching the order used when iterating peers for broadcast send (stable for the
    /// session while connections live).
    pub peer_index: usize,
    /// Inner payload only; length prefix already removed.
    pub bytes: Vec<u8>,
}
