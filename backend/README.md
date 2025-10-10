# OpenGuild Backend Workspace

Rust-based backend implementing the homeserver, federation layer, media services, and supporting libraries.

## Crates

- `openguild-core` — canonical events, signatures, shared domain logic.
- `openguild-server` — HTTP + WS gateway.
- `openguild-storage` — Postgres access helpers.
- `openguild-media` — Object storage configuration and validation.
- `openguild-crypto` — cryptographic primitives for signing and verification.
- `openguild-sfu-client` — voice signaling DTOs.

## Development

```bash
cargo fmt
cargo clippy --workspace --all-targets
cargo check --workspace
cargo test -p openguild-server
```

See `../deploy/docker-compose.yml` for local infrastructure services.
