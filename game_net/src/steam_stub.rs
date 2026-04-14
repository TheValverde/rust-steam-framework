//! Stub [`super::SteamMultiplayer`] when the **`steam`** feature is **disabled** (CI, no SDK).
//!
//! Methods are no-ops or return empty / error; `host_lobby` / `join_lobby` print one stderr hint.

use crate::payload::FramedPayload;
use crate::reliability::NetSendReliability;
use crate::session::SteamSessionConfig;

/// Inert stand-in for [`crate::SteamMultiplayer`] without linking `steamworks`.
#[derive(Debug)]
pub struct SteamMultiplayer {
    config: SteamSessionConfig,
}

impl SteamMultiplayer {
    pub fn new(config: SteamSessionConfig) -> Self {
        Self { config }
    }

    pub fn run_callbacks(&self) {}

    pub fn tick_multiplayer(&mut self) {}

    pub fn host_lobby(&mut self) {
        eprintln!(
            "game_net: built without `steam` feature; enable `steam` or default features on the depending crate."
        );
    }

    pub fn join_lobby(&mut self, _lobby_id_text: &str) {
        eprintln!(
            "game_net: built without `steam` feature; enable `steam` or default features on the depending crate."
        );
    }

    pub fn leave_multiplayer(&mut self) {}

    pub fn open_overlay_invite(&self) {}

    pub fn overlay_invite_available(&self) -> bool {
        false
    }

    pub fn status_banner(&self) -> String {
        "Steam: disabled in this build (`steam` feature off).".to_string()
    }

    pub fn multiplayer_detail_lines(&self) -> Vec<String> {
        Vec::new()
    }

    pub fn connection_panel_lines(&self) -> Vec<String> {
        Vec::new()
    }

    pub fn multiplayer_error(&self) -> Option<&str> {
        None
    }

    pub fn handshaken_peer_count(&self) -> usize {
        0
    }

    pub fn p2p_session_ready(&self) -> bool {
        false
    }

    pub fn p2p_is_host(&self) -> bool {
        false
    }

    pub fn try_send_framed_payload(&mut self, payload: &[u8]) -> Result<(), ()> {
        self.try_send_framed_payload_reliability(payload, NetSendReliability::Reliable)
    }

    pub fn try_send_framed_payload_reliability(
        &mut self,
        payload: &[u8],
        _reliability: NetSendReliability,
    ) -> Result<(), ()> {
        if payload.len() > self.config.max_game_payload_bytes {
            return Err(());
        }
        Err(())
    }

    pub fn poll_framed_payloads(&mut self) -> Vec<FramedPayload> {
        Vec::new()
    }
}
