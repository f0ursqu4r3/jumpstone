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

- `config/server.(toml|yaml|json)` - optional project-level defaults.
- `config/server.local.*` - optional overrides for local development (not checked in).
- Environment variables prefixed with `OPENGUILD_SERVER__` - highest precedence.

Common environment overrides:

- `OPENGUILD_SERVER__BIND_ADDR` - full socket address (e.g. `0.0.0.0:8080`). Overrides host/port.
- `OPENGUILD_SERVER__HOST` - interface to bind (default `0.0.0.0`).
- `OPENGUILD_SERVER__SERVER_NAME` - canonical homeserver name advertised in events (default `localhost`).
- `OPENGUILD_SERVER__PORT` - port number (default `8080`).
- `OPENGUILD_SERVER__LOG_FORMAT` - `compact` (default) or `json`.
- `OPENGUILD_SERVER__METRICS__ENABLED` - `true`/`false` toggle for the Prometheus exporter.
- `OPENGUILD_SERVER__METRICS__BIND_ADDR` - optional dedicated bind address for the metrics exporter.
- `OPENGUILD_SERVER__DATABASE_URL` - Postgres connection string (optional during bootstrap; required to persist sessions beyond process memory).
- `OPENGUILD_SERVER__SESSION__SIGNING_KEY` - URL-safe base64 ed25519 secret (32 bytes) used to sign session tokens; if omitted the server generates an ephemeral key at startup and logs the verifying key.
- `RUST_LOG` - tracing filter (e.g. `info,openguild_server=debug`).

> Build with `--features metrics` and set `OPENGUILD_SERVER__METRICS__ENABLED=true` to expose the `/metrics` endpoint.
> The exporter currently publishes the `openguild_http_requests_total{route,status}` counter.
> When `OPENGUILD_SERVER__METRICS__BIND_ADDR` is set, Prometheus scraping must target that listener (`/metrics` is removed from the primary router).
> All HTTP responses include an `X-Request-Id` header (propagated from inbound requests when present, otherwise generated per request) to simplify log correlation.

### Command-Line Overrides

You can override most runtime settings via CLI flags (highest precedence):

```bash
cargo run --bin openguild-server -- \
  --bind-addr 0.0.0.0:8081 \
  --server-name api.openguild.test \
  --log-format json \
  --metrics-enabled true \
  --metrics-bind-addr 127.0.0.1:9100 \
  --session-signing-key AbCdEf...==
```

Available flags:

- `--bind-addr <addr>` - overrides bind address (takes precedence over host/port).
- `--host <host>` / `--port <port>` - override individual components.
- `--log-format <compact|json>` - switch logging format without touching env vars.
- `--server-name <name>` - set the canonical homeserver name embedded in events.
- `--metrics-enabled <true|false>` - toggle metrics exporter stub.
- `--metrics-bind-addr <addr>` - dedicate a bind address for metrics (when enabled).
- `--database-url <url>` - supply a Postgres connection string (e.g. `postgres://user:pass@localhost/openguild`); when set the server upserts issued sessions into Postgres.
- `--session-signing-key <key>` - provide the URL-safe base64 ed25519 signing key for issued session tokens.

The `/ready` endpoint reports `database` status using these settings. When a database URL is provided, the server performs an eager connection attempt during startup and surfaces either `configured` (success) or `error` (failure details); otherwise it reports `pending`.

### Credential Bootstrap

Once Postgres is available (`OPENGUILD_SERVER__DATABASE_URL` or `--database-url`), seed at least one account so you can exercise the session/login flow:

```bash
# Seed via CLI (uses the same config/env overrides as runtime)
cargo run --bin openguild-server -- \
  --database-url postgres://app:secret@localhost/openguild \
  seed-user --username admin --password "supersecret"
```

- The command is idempotent: if the username already exists it logs a skip message and exits successfully.
- Passwords must be **at least 8 characters**; credentials are stored using Argon2id hashing.

You can also provision accounts through the HTTP API once the server is running:

```bash
curl -X POST http://127.0.0.1:8080/users/register \
  -H "content-type: application/json" \
  -d '{"username":"admin","password":"supersecret"}'
```

For login/refresh flows, clients must supply a stable device identifier:

```bash
curl -X POST http://127.0.0.1:8080/sessions/login \
  -H "content-type: application/json" \
  -d '{"identifier":"admin","secret":"supersecret","device":{"device_id":"admin-laptop","device_name":"Admin Laptop"}}'
```

> Device IDs should be stable per physical/browser device; refresh tokens are keyed by `username` + `device_id`. Subsequent logins on the same device rotate the stored refresh token.

## Development Shortcuts

A top-level `Makefile` provides common workflows:

```bash
make fmt           # cargo fmt --all
make lint          # cargo clippy with warnings as errors
make check         # cargo check --workspace (backend)
make test          # backend server tests
make test-metrics  # backend server tests with metrics feature
```
