# rust-steam-framework

Reusable Rust pieces for **Steam lobbies + SteamNetworkingSockets P2P**. The portable crate here is **`game_net`**: handshake, length-prefixed framed payloads, and an optional 15-byte wire header. No game-specific enums or engine bindings.

## Documentation

Start at **[docs/README.md](docs/README.md)** for the full index (integration guide, crate reference, `cargo doc`).

## Build

From the repository root:

```bash
cargo check --manifest-path game_net/Cargo.toml
cargo check --manifest-path game_net/Cargo.toml --no-default-features
```

The second line builds the **stub** (no `steamworks`); useful for CI without the Steam SDK.

## License

This repository is released under the [MIT License](LICENSE). The [game_net/Cargo.toml](game_net/Cargo.toml) `license` field is `MIT`.
