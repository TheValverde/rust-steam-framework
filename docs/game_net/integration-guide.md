# Integrating `game_net` into another Rust project

This guide assumes you already use or will use **Steamworks** for lobbies and **SteamNetworkingSockets** P2P. `game_net` does not replace Steam; it wraps the common **listen / connect / handshake / framed byte pipe** pattern.

---

## 1. Add the dependency

### Path (monorepo or vendored copy)

```toml
[dependencies]
game_net = { path = "../path/to/game_net", default-features = false }

[features]
default = ["steam"]
steam = ["dep:steamworks", "game_net/steam"]

[dependencies.steamworks]
version = "0.12.2"
optional = true
```

Match **`steamworks`** versions between your crate and `game_net` if you also depend on `steamworks` directly.

### Git submodule / copy

Vendor this folder, then use the same `path =` pattern. There is no crates.io publish in-tree; publishing under another name is your choice.

---

## 2. Choose handshake bytes

Pick two **distinct** byte sequences that will **never** appear as a prefix of your real game traffic by accident.

- **Host → client (first message on the connection):** `handshake_ping` (e.g. `b"MYGAME_PING"`).
- **Client → host (reply):** `handshake_pong` (e.g. `b"MYGAME_PONG"`).

They are sent as **raw** Steam messages (reliable), **not** inside the u32 length framing. After both sides agree, **only** length-framed payloads should carry game data.

---

## 3. Build `SteamSessionConfig`

| Field | Guidance |
|-------|----------|
| `app_id` | Your Steam App ID (u32). Dev often uses **480** (Spacewar) for testing. |
| `handshake_ping` / `handshake_pong` | See §2. Use `&'static [u8]` literals. |
| `recv_batch_max` | Max messages per `receive_messages` call per peer per **your** tick (e.g. 16–64). Caps worst-case work per frame. |
| `max_game_payload_bytes` | Max length of the **inner** game blob (after the 4-byte length prefix is stripped). Must fit your largest envelope + compression slack. |
| `lobby_max_members` | Passed to `create_lobby` (Steam API). |
| `init_failed_log_prefix` | Short tag for `eprintln!` when `Client::init_app` fails (offline / no SDK). |

Example:

```rust
use game_net::SteamSessionConfig;

fn my_net_config() -> SteamSessionConfig {
    SteamSessionConfig {
        app_id: 480,
        handshake_ping: b"MYGAME_PING",
        handshake_pong: b"MYGAME_PONG",
        recv_batch_max: 32,
        max_game_payload_bytes: 4 * 1024 * 1024,
        lobby_max_members: 8,
        init_failed_log_prefix: "my_game",
    }
}
```

---

## 4. Drive the session every frame

Recommended order (matches typical Steam + game loop usage):

1. **`steam_client.run_callbacks()`** (your `steamworks::Client`, if you hold one separately) — or ensure callbacks run; `SteamMultiplayer::run_callbacks()` forwards to its internal client when Steam initialized.
2. **`session.tick_multiplayer()`** — resolves async lobby create/join, accepts incoming P2P connections, advances handshake.
3. **Game send path:** `try_send_framed_payload` / `try_send_framed_payload_reliability` only when `p2p_session_ready()` is true.
4. **Game recv path:** `poll_framed_payloads()` — drain all pending framed payloads for this frame; then decode in your game layer.

Host **broadcasts** the same framed bytes to every handshaken peer. Client sends to the single host connection.

---

## 5. Encode what goes inside the frame

`game_net` treats the **inner** bytes as opaque. A typical layout in a game client:

1. Optional **application wire header** (e.g. `game_net::wire_header` — 15 bytes: version, kind, tick, payload length).
2. **Your** serialized blob (bincode, protobuf, raw, etc.).

The **outer** wrapping is always:

```text
[ u32 LE length_of_inner ][ inner bytes... ]
```

`length` must equal `inner.len()` and `inner.len() <= max_game_payload_bytes`.

You can use **`wire_header::build_frame`** without using `SteamMultiplayer` (e.g. tests, non-Steam transports).

---

## 6. Host vs client API summary

| Concern | Host (`Hosting`) | Client (`ClientLobby`) |
|---------|------------------|-------------------------|
| Lobby | `host_lobby()` | `join_lobby("aB3xYz")` with the host’s **6-character** room code (`a-z`, `A-Z`, `0-9`), resolved via lobby list before join |
| Peers | Multiple `PeerSlot`s | Single `NetConnection` |
| `FramedPayload::peer_index` | Index into handshaken peer list | Always `0` |
| Send | Broadcast to all handshaken peers | Single host |
| Recv | `poll_framed_payloads` merges all peers | Single stream |

Use **`handshaken_peer_count`** / **`p2p_session_ready`** / **`p2p_is_host`** for UI and game logic gating.

---

## 7. Stub builds (CI, no SDK)

```toml
game_net = { path = "../game_net", default-features = false }
# Do not enable game_net/steam
```

`SteamMultiplayer::new(config)` still works; sends fail, polls empty, lobby calls log one line to stderr. Use for **compile-only** CI or headless tests.

---

## 8. Checklist before shipping

- [ ] App ID and `steam_appid.txt` / launch environment match Steam partner docs.
- [ ] Handshake strings are unique and documented for any alternate clients.
- [ ] `max_game_payload_bytes` ≥ largest encoded game packet (including any header).
- [ ] `recv_batch_max` tuned for your frame budget under spam / relay.
- [ ] You handle `multiplayer_error()` for user-visible errors.
- [ ] You never interpret `poll_framed_payloads` bytes as game traffic **before** handshake completes (this crate already gates recv on handshake).

---

## 9. Where to look in source

Paths are under the **`game_net`** crate directory in this repository.

| Topic | File |
|-------|------|
| Live Steam logic | `game_net/src/steam_live.rs` |
| Stub | `game_net/src/steam_stub.rs` |
| Length prefix | `game_net/src/framing.rs` |
| 15-byte header helper | `game_net/src/wire_header.rs` |
| Config struct | `game_net/src/session.rs` |

See also **[game_net/README.md](../../game_net/README.md)** for diagrams and design boundaries.
