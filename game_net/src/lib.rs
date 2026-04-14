//! # `game_net`
//!
//! Portable **Steam lobbies + P2P** session layer: configurable **handshake bytes**, **u32 LE length + inner payload**
//! framing for all game traffic, **reliable / unreliable** send flags, and an optional **15-byte** `(version, kind,
//! tick, len)` header helper for opaque inner bodies.
//!
//! ## Documentation
//!
//! - **README.md** at the crate root: architecture, on-wire layouts, full API tables, mermaid diagrams.
//! - **docs/integration-guide.md**: checklist for wiring into a **new** Rust project.
//!
//! ## What belongs here vs in your game
//!
//! | In `game_net` | In your game crate |
//! |----------------|-------------------|
//! | Steam init, lobby, listen, connect, handshake | `NetMsg*` enums, serde, protobuf, etc. |
//! | Length framing, recv batch cap, max payload | Coalescing, prediction, world replication rules |
//! | `wire_header` parse/build | Mapping `kind` → your message types |
//!
//! ## Features
//!
//! - **`steam`** (default): real `SteamMultiplayer` backed by `steamworks`.
//! - **No default features**: stub implementation (compile without Steam SDK).
//!
//! ## Minimal usage
//!
//! ```ignore
//! let mut s = game_net::SteamMultiplayer::new(my_config());
//! s.run_callbacks();
//! s.tick_multiplayer();
//! if s.p2p_session_ready() {
//!     let _ = s.try_send_framed_payload(&bytes);
//!     for p in s.poll_framed_payloads() { /* decode p.bytes */ }
//! }
//! ```

pub mod wire_header;

mod framing;
mod payload;
mod reliability;
mod session;

#[cfg(feature = "steam")]
mod steam_live;
#[cfg(not(feature = "steam"))]
mod steam_stub;

pub use payload::FramedPayload;
pub use reliability::NetSendReliability;
pub use session::SteamSessionConfig;

#[cfg(feature = "steam")]
pub use steam_live::SteamMultiplayer;
#[cfg(not(feature = "steam"))]
pub use steam_stub::SteamMultiplayer;
