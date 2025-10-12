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

- `config/server.(toml|yaml|json)` â€” optional project-level defaults.
- `config/server.local.*` â€” optional overrides for local development (not checked in).
- Environment variables prefixed with `OPENGUILD_SERVER__` â€” highest precedence.

Common environment overrides:

- `OPENGUILD_SERVER__BIND_ADDR` â€” full socket address (e.g. `0.0.0.0:8080`). Overrides host/port.
- `OPENGUILD_SERVER__HOST` â€” interface to bind (default `0.0.0.0`).
- `OPENGUILD_SERVER__PORT` â€” port number (default `8080`).
- `OPENGUILD_SERVER__LOG_FORMAT` â€” `compact` (default) or `json`.
- `OPENGUILD_SERVER__METRICS__ENABLED` â€” `true`/`false` toggle for the Prometheus exporter.
- `OPENGUILD_SERVER__METRICS__BIND_ADDR` â€” optional dedicated bind address for metrics exporter.
- `OPENGUILD_SERVER__DATABASE_URL` â€” Postgres connection string for the storage layer (optional during bootstrap).
- `RUST_LOG` â€” tracing filter (e.g. `info,openguild_server=debug`).

> Build with `--features metrics` and set `OPENGUILD_SERVER__METRICS__ENABLED=true` to expose the `/metrics` endpoint.
> The exporter currently publishes the `openguild_http_requests_total{route,status}` counter.

### Command-Line Overrides

You can override most runtime settings via CLI flags (highest precedence):

```bash
cargo run --bin openguild-server -- \
  --bind-addr 0.0.0.0:8081 \
  --log-format json \
  --metrics-enabled true \
  --metrics-bind-addr 127.0.0.1:9100
```

Available flags:

- `--bind-addr <addr>` â€” overrides bind address (takes precedence over host/port).
- `--host <host>` / `--port <port>` â€” override individual components.
- `--log-format <compact|json>` â€” switch logging format without touching env vars.
- `--metrics-enabled <true|false>` â€” toggle metrics exporter stub.
- `--metrics-bind-addr <addr>` â€” dedicate a bind address for metrics (when enabled).
- `--database-url <url>` â€” supply Postgres connection string (e.g. `postgres://user:pass@localhost/openguild`).

The `/ready` endpoint reports `database` status using these settings. When a database URL is provided, the server performs an eager connection attempt during startup and surfaces either `configured` (success) or `error` (failure details); otherwise it reports `pending`.

## Development Shortcuts

A top-level `Makefile` provides common workflows:

```bash
make fmt           # cargo fmt --all
make lint          # cargo clippy with warnings as errors
make check         # cargo check --workspace (backend)
make test          # backend server tests
make test-metrics  # backend server tests with metrics feature
```
