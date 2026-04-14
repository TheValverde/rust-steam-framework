//! Live [`super::SteamMultiplayer`] when the **`steam`** Cargo feature is enabled.
//!
//! Uses Steam lobbies, `create_listen_socket_p2p` / `connect_p2p` on **virtual port 0** (not configurable). All
//! session-specific strings and limits come from [`crate::SteamSessionConfig`].

use std::sync::mpsc::{self, Receiver};

use rand::Rng;
use steamworks::networking_sockets::{ListenSocket, NetConnection};
use steamworks::networking_types::{
    ListenSocketEvent, NetworkingConnectionState, NetworkingIdentity, SendFlags,
};
use steamworks::{
    Client, DistanceFilter, LobbyId, LobbyListFilter, LobbyKey, LobbyType, Matchmaking, SteamError,
    StringFilter, StringFilterKind,
};

use crate::framing;
use crate::payload::FramedPayload;
use crate::reliability::NetSendReliability;
use crate::session::SteamSessionConfig;

const P2P_VIRTUAL_PORT: i32 = 0;

/// Lobby metadata key for the joinable 6-character room code.
const WOL_LOBBY_ROOM_KEY: &str = "wol_room";
const ROOM_CODE_LEN: usize = 6;

fn random_room_code() -> String {
    const ALPHABET: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..ROOM_CODE_LEN)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

fn is_valid_room_code(s: &str) -> bool {
    s.len() == ROOM_CODE_LEN && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn networking_identity_same_peer(a: &NetworkingIdentity, b: &NetworkingIdentity) -> bool {
    match (a.steam_id(), b.steam_id()) {
        (Some(sa), Some(sb)) => sa == sb,
        _ => a.debug_string() == b.debug_string() && !a.debug_string().is_empty(),
    }
}

struct PeerSlot {
    remote: NetworkingIdentity,
    conn: NetConnection,
    ping_sent: bool,
    handshake_done: bool,
}

enum MultiplayerState {
    Idle,
    PendingCreate {
        rx: Receiver<Result<LobbyId, SteamError>>,
    },
    Hosting {
        lobby_id: LobbyId,
        listen: ListenSocket,
        peers: Vec<PeerSlot>,
    },
    /// `RequestLobbyList` in flight; do not call `JoinLobby` inside that callback (Steam deadlock).
    PendingLobbyList {
        rx: Receiver<Result<Option<LobbyId>, ()>>,
    },
    PendingJoin {
        rx: Receiver<Result<LobbyId, ()>>,
    },
    ClientLobby {
        lobby_id: LobbyId,
        conn: Option<NetConnection>,
        handshake_done: bool,
        /// P2P connection dropped after joining (timeout, host quit, etc.).
        p2p_session_dead: bool,
    },
    ErrorLine(String),
}

/// Steam-backed session: see crate README for lifecycle and framing rules.
pub struct SteamMultiplayer {
    client: Option<Client>,
    mp: MultiplayerState,
    config: SteamSessionConfig,
}

impl SteamMultiplayer {
    pub fn new(config: SteamSessionConfig) -> Self {
        match Client::init_app(steamworks::AppId(config.app_id)) {
            Ok(client) => {
                client.networking_utils().init_relay_network_access();
                let _ = client.networking_sockets().init_authentication();
                Self {
                    client: Some(client),
                    mp: MultiplayerState::Idle,
                    config,
                }
            }
            Err(e) => {
                eprintln!(
                    "{}: Steam API init failed (offline mode): {e}",
                    config.init_failed_log_prefix
                );
                Self {
                    client: None,
                    mp: MultiplayerState::Idle,
                    config,
                }
            }
        }
    }

    pub fn run_callbacks(&self) {
        if let Some(c) = &self.client {
            c.run_callbacks();
        }
    }

    pub fn status_banner(&self) -> String {
        match &self.client {
            None => "Steam: offline (API not initialized).".to_string(),
            Some(c) => {
                let name = c.friends().name();
                format!("Steam: signed in as {name}")
            }
        }
    }

    pub fn multiplayer_error(&self) -> Option<&str> {
        match &self.mp {
            MultiplayerState::ErrorLine(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn overlay_invite_available(&self) -> bool {
        matches!(self.mp, MultiplayerState::Hosting { .. })
    }

    pub fn open_overlay_invite(&self) {
        let (Some(c), MultiplayerState::Hosting { lobby_id, .. }) = (&self.client, &self.mp) else {
            return;
        };
        c.friends().activate_invite_dialog(*lobby_id);
    }

    pub fn host_lobby(&mut self) {
        self.clear_error_to_idle();
        let Some(client) = self.client.clone() else {
            self.mp = MultiplayerState::ErrorLine("Steam not available.".to_string());
            return;
        };
        self.leave_multiplayer();
        let (tx, rx) = mpsc::channel();
        let max_m = self.config.lobby_max_members;
        client
            .matchmaking()
            .create_lobby(LobbyType::Public, max_m, move |r| {
                let _ = tx.send(r);
            });
        self.mp = MultiplayerState::PendingCreate { rx };
    }

    pub fn join_lobby(&mut self, lobby_id_text: &str) {
        self.clear_error_to_idle();
        let Some(client) = self.client.clone() else {
            self.mp = MultiplayerState::ErrorLine("Steam not available.".to_string());
            return;
        };
        let trimmed = lobby_id_text.trim();
        if !is_valid_room_code(trimmed) {
            self.mp = MultiplayerState::ErrorLine(
                "Enter the 6-character room code (a-z, A-Z, 0-9).".to_string(),
            );
            return;
        }
        self.leave_multiplayer();
        let (tx, rx) = mpsc::channel();
        let filter = LobbyListFilter {
            string: Some(vec![StringFilter(
                LobbyKey::new(WOL_LOBBY_ROOM_KEY),
                trimmed,
                StringFilterKind::Equal,
            )]),
            distance: Some(DistanceFilter::Worldwide),
            count: Some(16),
            ..Default::default()
        };
        client
            .matchmaking()
            .set_lobby_list_filter(filter)
            .request_lobby_list(move |result| {
                let out = match result {
                    Ok(lobbies) => Ok(lobbies.first().copied()),
                    Err(_) => Err(()),
                };
                let _ = tx.send(out);
            });
        self.mp = MultiplayerState::PendingLobbyList { rx };
    }

    pub fn leave_multiplayer(&mut self) {
        if let (Some(client), mp) = (&self.client, &self.mp) {
            Self::leave_lobby_if_needed(client, mp);
        }
        self.mp = MultiplayerState::Idle;
    }

    fn clear_error_to_idle(&mut self) {
        if matches!(self.mp, MultiplayerState::ErrorLine(_)) {
            self.mp = MultiplayerState::Idle;
        }
    }

    fn leave_lobby_if_needed(client: &Client, mp: &MultiplayerState) {
        match mp {
            MultiplayerState::Hosting { lobby_id, .. }
            | MultiplayerState::ClientLobby { lobby_id, .. } => {
                client.matchmaking().leave_lobby(*lobby_id);
            }
            _ => {}
        }
    }

    pub fn tick_multiplayer(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };

        let ping = self.config.handshake_ping;
        let pong = self.config.handshake_pong;

        match &mut self.mp {
            MultiplayerState::Idle | MultiplayerState::ErrorLine(_) => {}

            MultiplayerState::PendingLobbyList { rx } => {
                if let Ok(res) = rx.try_recv() {
                    match res {
                        Ok(Some(lid)) => {
                            let (tx_join, rx_join) = mpsc::channel();
                            client.matchmaking().join_lobby(lid, move |r| {
                                let _ = tx_join.send(r);
                            });
                            self.mp = MultiplayerState::PendingJoin { rx: rx_join };
                        }
                        Ok(None) => {
                            self.mp = MultiplayerState::ErrorLine(
                                "No lobby found with that room code.".to_string(),
                            );
                        }
                        Err(()) => {
                            self.mp = MultiplayerState::ErrorLine(
                                "Could not search lobbies (Steam).".to_string(),
                            );
                        }
                    }
                }
            }

            MultiplayerState::PendingCreate { rx } => {
                if let Ok(res) = rx.try_recv() {
                    match res {
                        Ok(lid) => {
                            let room_code = random_room_code();
                            let _ = client
                                .matchmaking()
                                .set_lobby_data(lid, WOL_LOBBY_ROOM_KEY, &room_code);
                            let listen = match client
                                .networking_sockets()
                                .create_listen_socket_p2p(P2P_VIRTUAL_PORT, [])
                            {
                                Ok(l) => l,
                                Err(_) => {
                                    self.mp = MultiplayerState::ErrorLine(
                                        "create_listen_socket_p2p failed.".to_string(),
                                    );
                                    return;
                                }
                            };
                            self.mp = MultiplayerState::Hosting {
                                lobby_id: lid,
                                listen,
                                peers: Vec::new(),
                            };
                        }
                        Err(e) => {
                            self.mp = MultiplayerState::ErrorLine(format!(
                                "Create lobby failed: {e:?}"
                            ));
                        }
                    }
                }
            }

            MultiplayerState::PendingJoin { rx } => {
                if let Ok(res) = rx.try_recv() {
                    match res {
                        Ok(lid) => {
                            let owner = client.matchmaking().lobby_owner(lid);
                            let identity = NetworkingIdentity::new_steam_id(owner);
                            match client
                                .networking_sockets()
                                .connect_p2p(identity, P2P_VIRTUAL_PORT, [])
                            {
                                Ok(conn) => {
                                    self.mp = MultiplayerState::ClientLobby {
                                        lobby_id: lid,
                                        conn: Some(conn),
                                        handshake_done: false,
                                        p2p_session_dead: false,
                                    };
                                }
                                Err(_) => {
                                    self.mp = MultiplayerState::ErrorLine(
                                        "connect_p2p failed.".to_string(),
                                    );
                                    client.matchmaking().leave_lobby(lid);
                                }
                            }
                        }
                        Err(()) => {
                            self.mp =
                                MultiplayerState::ErrorLine("Could not join that lobby.".to_string());
                        }
                    }
                }
            }

            MultiplayerState::Hosting {
                listen,
                peers,
                lobby_id: _,
            } => {
                while let Some(ev) = listen.try_receive_event() {
                    match ev {
                        ListenSocketEvent::Connecting(req) => {
                            let _ = req.accept();
                        }
                        ListenSocketEvent::Connected(c) => {
                            peers.push(PeerSlot {
                                remote: c.remote(),
                                conn: c.take_connection(),
                                ping_sent: false,
                                handshake_done: false,
                            });
                        }
                        ListenSocketEvent::Disconnected(ev) => {
                            let lost = ev.remote();
                            peers.retain(|p| !networking_identity_same_peer(&p.remote, &lost));
                        }
                    }
                }

                for p in peers.iter_mut() {
                    if p.handshake_done {
                        continue;
                    }
                    if !p.ping_sent {
                        let _ = p
                            .conn
                            .send_message(ping, SendFlags::RELIABLE_NO_NAGLE);
                        p.ping_sent = true;
                    }
                    if Self::drain_pong(&mut p.conn, pong) {
                        p.handshake_done = true;
                    }
                }
            }

            MultiplayerState::ClientLobby {
                conn,
                handshake_done,
                p2p_session_dead,
                ..
            } => {
                if let Some(ref c_conn) = conn {
                    let sockets = client.networking_sockets();
                    let dead = match sockets.get_connection_info(c_conn) {
                        Ok(info) => matches!(
                            info.state(),
                            Ok(NetworkingConnectionState::ClosedByPeer)
                                | Ok(NetworkingConnectionState::ProblemDetectedLocally)
                                | Ok(NetworkingConnectionState::None)
                        ),
                        Err(_) => true,
                    };
                    if dead {
                        *conn = None;
                        *handshake_done = false;
                        *p2p_session_dead = true;
                    }
                }
                if let Some(ref mut c) = conn {
                    if !*handshake_done {
                        if let Ok(msgs) = c.receive_messages(16) {
                            for m in msgs {
                                if m.data() == ping {
                                    let _ = c.send_message(pong, SendFlags::RELIABLE_NO_NAGLE);
                                    *handshake_done = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn drain_pong(conn: &mut NetConnection, pong: &[u8]) -> bool {
        let Ok(msgs) = conn.receive_messages(16) else {
            return false;
        };
        for m in msgs {
            if m.data() == pong {
                return true;
            }
        }
        false
    }

    pub fn multiplayer_detail_lines(&self) -> Vec<String> {
        let Some(c) = &self.client else {
            return Vec::new();
        };
        let mm = c.matchmaking();
        match &self.mp {
            MultiplayerState::Hosting {
                lobby_id,
                peers,
                ..
            } => {
                let mut lines = Self::lobby_common_lines(c, &mm, *lobby_id);
                let ready = peers.iter().filter(|p| p.handshake_done).count();
                let total = peers.len();
                lines.push(if total == 0 {
                    "P2P: waiting for peer / handshake…".to_string()
                } else {
                    format!("P2P: {ready}/{total} peer(s) handshook (SteamNetworkingSockets).")
                });
                lines
            }
            MultiplayerState::ClientLobby {
                lobby_id,
                handshake_done,
                ..
            } => {
                let mut lines = Self::lobby_common_lines(c, &mm, *lobby_id);
                lines.push(if *handshake_done {
                    "P2P: handshake OK (SteamNetworkingSockets).".to_string()
                } else {
                    "P2P: connecting…".to_string()
                });
                lines
            }
            MultiplayerState::PendingCreate { .. } => {
                vec!["Creating lobby…".to_string()]
            }
            MultiplayerState::PendingLobbyList { .. } => {
                vec!["Looking up room code…".to_string()]
            }
            MultiplayerState::PendingJoin { .. } => {
                vec!["Joining lobby…".to_string()]
            }
            _ => Vec::new(),
        }
    }

    pub fn connection_panel_lines(&self) -> Vec<String> {
        let Some(c) = &self.client else {
            return vec!["Steam offline.".to_string()];
        };
        let mm = c.matchmaking();
        match &self.mp {
            MultiplayerState::Hosting { lobby_id, .. }
            | MultiplayerState::ClientLobby { lobby_id, .. } => {
                let self_id = c.user().steam_id();
                let friends = c.friends();
                let mut lines = Vec::new();
                let mut peer_idx = 0_u32;
                for m in mm.lobby_members(*lobby_id) {
                    let name = friends.get_friend(m).name();
                    let tag = if m == self_id {
                        "You".to_string()
                    } else {
                        peer_idx += 1;
                        format!("Peer {}", peer_idx)
                    };
                    lines.push(format!("{tag}: {name}"));
                }
                if lines.is_empty() {
                    lines.push("No lobby members listed.".to_string());
                }
                lines
            }
            _ => vec!["Not in a Steam lobby.".to_string()],
        }
    }

    fn lobby_common_lines(client: &Client, mm: &Matchmaking, lobby_id: LobbyId) -> Vec<String> {
        let mut lines = Vec::new();
        let room = mm
            .lobby_data(lobby_id, WOL_LOBBY_ROOM_KEY)
            .filter(|s| is_valid_room_code(s));
        lines.push(match room {
            Some(c) => format!("Room code: {c}"),
            None => "Room code: (loading…)".to_string(),
        });
        lines.push(format!("Members: {}", mm.lobby_member_count(lobby_id)));
        let owner = mm.lobby_owner(lobby_id);
        lines.push(format!("Lobby owner SteamID64: {}", owner.raw()));
        let friends = client.friends();
        for m in mm.lobby_members(lobby_id) {
            let label = friends.get_friend(m).name();
            lines.push(format!("  • {} ({})", label, m.raw()));
        }
        lines
    }

    pub fn handshaken_peer_count(&self) -> usize {
        match &self.mp {
            MultiplayerState::Hosting { peers, .. } => peers.iter().filter(|p| p.handshake_done).count(),
            MultiplayerState::ClientLobby {
                conn: Some(_),
                handshake_done: true,
                p2p_session_dead: false,
                ..
            } => 1,
            _ => 0,
        }
    }

    pub fn p2p_session_ready(&self) -> bool {
        matches!(
            &self.mp,
            MultiplayerState::Hosting { peers, .. }
                if peers.iter().any(|p| p.handshake_done)
        ) | matches!(
            &self.mp,
            MultiplayerState::ClientLobby {
                conn: Some(_),
                handshake_done: true,
                p2p_session_dead: false,
                ..
            }
        )
    }

    pub fn p2p_is_host(&self) -> bool {
        matches!(
            &self.mp,
            MultiplayerState::Hosting { peers, .. }
                if peers.iter().any(|p| p.handshake_done)
        )
    }

    pub fn try_send_framed_payload(&mut self, payload: &[u8]) -> Result<(), ()> {
        self.try_send_framed_payload_reliability(payload, NetSendReliability::Reliable)
    }

    pub fn try_send_framed_payload_reliability(
        &mut self,
        payload: &[u8],
        reliability: NetSendReliability,
    ) -> Result<(), ()> {
        let max = self.config.max_game_payload_bytes;
        if payload.len() > max {
            return Err(());
        }
        let Some(buf) = framing::prepend_length_prefix(payload, max) else {
            return Err(());
        };
        let flags = match reliability {
            NetSendReliability::Reliable => SendFlags::RELIABLE_NO_NAGLE,
            NetSendReliability::Unreliable => SendFlags::UNRELIABLE_NO_NAGLE,
        };

        match &mut self.mp {
            MultiplayerState::Hosting { peers, .. } => {
                let mut any = false;
                for p in peers.iter_mut() {
                    if p.handshake_done {
                        let _ = p.conn.send_message(&buf, flags);
                        any = true;
                    }
                }
                if !any {
                    return Err(());
                }
            }
            MultiplayerState::ClientLobby {
                conn: Some(c),
                handshake_done: true,
                p2p_session_dead: false,
                ..
            } => {
                let _ = c.send_message(&buf, flags);
            }
            _ => return Err(()),
        }
        Ok(())
    }

    pub fn poll_framed_payloads(&mut self) -> Vec<FramedPayload> {
        let mut out = Vec::new();
        let max = self.config.max_game_payload_bytes;
        let batch = self.config.recv_batch_max;
        let mut drain = |peer_index: usize, conn: &mut NetConnection| {
            let Ok(msgs) = conn.receive_messages(batch) else {
                return;
            };
            for m in msgs {
                let data = m.data();
                if let Some(inner) = framing::strip_length_prefix(data, max) {
                    out.push(FramedPayload {
                        peer_index,
                        bytes: inner.to_vec(),
                    });
                }
            }
        };
        match &mut self.mp {
            MultiplayerState::Hosting { peers, .. } => {
                for (i, p) in peers.iter_mut().enumerate() {
                    if p.handshake_done {
                        drain(i, &mut p.conn);
                    }
                }
            }
            MultiplayerState::ClientLobby {
                conn: Some(c),
                handshake_done: true,
                p2p_session_dead: false,
                ..
            } => drain(0, c),
            _ => {}
        }
        out
    }
}
