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

The server now loads configuration via the `config` crate with layered sources:

- `config/server.(toml|yaml|json)` — optional project-level defaults.
- `config/server.local.*` — optional overrides for local development (not checked in).
- Environment variables prefixed with `OPENGUILD_SERVER__` — highest precedence.

Common environment overrides:

- `OPENGUILD_SERVER__BIND_ADDR` — full socket address (e.g. `0.0.0.0:8080`). Overrides host/port.
- `OPENGUILD_SERVER__HOST` — interface to bind (default `0.0.0.0`).
- `OPENGUILD_SERVER__PORT` — port number (default `8080`).
- `OPENGUILD_SERVER__LOG_FORMAT` — `compact` (default) or `json`.
- `OPENGUILD_SERVER__METRICS__ENABLED` — `true`/`false` toggle for metrics stub (future work).
- `OPENGUILD_SERVER__METRICS__BIND_ADDR` — optional dedicated bind address for metrics exporter.
- `RUST_LOG` — tracing filter (e.g. `info,openguild_server=debug`).

> Build with `--features metrics` and set `OPENGUILD_SERVER__METRICS__ENABLED=true` to expose the `/metrics` stub while instrumentation is under construction.
