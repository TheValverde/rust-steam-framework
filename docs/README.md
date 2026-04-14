# Documentation index

All human-written docs for this repository live under **`docs/`**. The **`game_net`** crate also keeps a full reference **[README.md](../game_net/README.md)** next to its source (usual Rust layout).

| Document | Purpose |
|----------|---------|
| [game_net/integration-guide.md](game_net/integration-guide.md) | Wire `game_net` into another project: `Cargo.toml`, config, frame loop, checklist |
| [../game_net/README.md](../game_net/README.md) | Crate reference: wire formats, API tables, diagrams, features |

## Rust API docs (`cargo doc`)

From the repository root:

```bash
cargo doc --manifest-path game_net/Cargo.toml --no-deps --open
```

If your workspace lists `game_net` as a member:

```bash
cargo doc -p game_net --no-deps
```
