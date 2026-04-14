/// Parameters for [`crate::SteamMultiplayer`]: Steam app identity, handshake discrimination, and safety limits.
///
/// Construct once and pass to [`crate::SteamMultiplayer::new`]. All fields are required; there is no default impl so
/// games explicitly choose limits (portable across projects with different packet sizes).
///
/// # Handshake contract
///
/// After a P2P connection is established, the **host** sends [`Self::handshake_ping`] as a **raw** Steam message (not
/// length-framed). The **client** waits for bytes equal to `handshake_ping`, then replies with `handshake_pong`.
/// Until the host observes `handshake_pong`, [`crate::SteamMultiplayer`] does not expose framed game payloads from
/// that connection.
#[derive(Clone, Debug)]
pub struct SteamSessionConfig {
    /// Steam App ID passed to `Client::init_app` (e.g. `480` for Spacewar during development).
    pub app_id: u32,
    /// First application-defined message the **host** sends on a new connection (must not collide with your framed
    /// packets; use a unique ASCII tag).
    pub handshake_ping: &'static [u8],
    /// **Client** reply to [`Self::handshake_ping`]; must differ from `handshake_ping`.
    pub handshake_pong: &'static [u8],
    /// Upper bound passed to `NetConnection::receive_messages` when draining framed traffic (per peer, per call).
    pub recv_batch_max: usize,
    /// Maximum allowed **inner** game payload size in bytes (after the 4-byte length prefix is removed).
    pub max_game_payload_bytes: usize,
    /// Maximum lobby members for `create_lobby` (Steam API).
    pub lobby_max_members: u32,
    /// Short label for `eprintln!` when Steam client initialization fails (offline / missing SDK).
    pub init_failed_log_prefix: &'static str,
}
