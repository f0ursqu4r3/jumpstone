# Development Environment Setup (Draft)

> This document will be expanded as services land. For now it captures base prerequisites.

## Prerequisites

- Rust toolchain (`rustup`, stable channel) + `cargo`.
- Node.js 18+ with `pnpm`.
- Docker (Compose v2) for local Postgres, MinIO, and NATS services.

## Quick Start

```bash
# bootstrap workspace tooling
rustup component add clippy rustfmt
cargo install sqlx-cli --no-default-features --features postgres

npm install -g pnpm

# spin up infrastructure once docker-compose is defined
pnpm install --prefix frontend
cargo check --workspace --manifest-path backend/Cargo.toml
```

_Add detailed steps as services are implemented._

## Server Runtime Config

The server honors basic environment variables for local development:

- `BIND_ADDR` — full socket address to bind (e.g. `0.0.0.0:8080`). If set, overrides `HOST`/`PORT`.
- `HOST` — interface to bind (default `0.0.0.0`).
- `PORT` — port to bind (default `8080`).
- `RUST_LOG` — tracing filter (e.g. `info,openguild_server=debug`).
- `LOG_FORMAT` — set to `json` for JSON logs (default is human-readable compact).
