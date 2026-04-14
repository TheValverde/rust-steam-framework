/// Delivery preference for **length-framed** game payloads (after the Steam handshake).
///
/// Maps to SteamNetworkingSockets `SendFlags` (`RELIABLE_NO_NAGLE` vs `UNRELIABLE_NO_NAGLE`). Unreliable datagrams may
/// be dropped or reordered; use only when your game protocol tolerates loss (e.g. visual snapshots).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetSendReliability {
    /// Ordered reliable delivery (TCP-like for this channel).
    Reliable,
    /// Best-effort; lower latency under load but no ordering guarantee vs other unreliable sends.
    Unreliable,
}
