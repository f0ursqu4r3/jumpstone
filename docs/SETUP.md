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
# run the full backend test suite
cargo xtask test
```

_Add detailed steps as services are implemented._

> Looking for coverage details? See `docs/TESTING.md` for a week-by-week breakdown of automated tests and the commands used to execute them.

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
- `OPENGUILD_SERVER__SESSION__FALLBACK_VERIFYING_KEYS__{N}` - optional URL-safe base64 ed25519 public keys that remain valid during key rotation; set one entry per index (e.g. `_0`, `_1`).
- `OPENGUILD_SERVER__MESSAGING__MAX_MESSAGES_PER_USER_PER_WINDOW` - per-user rate limit budget for the configured window (default `60`).
- `OPENGUILD_SERVER__MESSAGING__MAX_MESSAGES_PER_IP_PER_WINDOW` - per-IP rate limit budget for the configured window (default `200`).
- `OPENGUILD_SERVER__MESSAGING__RATE_LIMIT_WINDOW_SECS` - sliding window length in seconds applied to the limits above (default `60`).
- `OPENGUILD_SERVER__MLS__ENABLED` - set to `true` to expose MLS key package APIs.
- `OPENGUILD_SERVER__MLS__CIPHERSUITE` - override the MLS ciphersuite identifier (default `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`).
- `OPENGUILD_SERVER__MLS__IDENTITIES__{N}` - pre-provision MLS client identities for which the server will publish and rotate key packages (repeat per entry).
- `OPENGUILD_SERVER__FEDERATION__TRUSTED_SERVERS__{N}__SERVER_NAME` - declare a homeserver that may submit federation transactions (repeat per entry).
- `OPENGUILD_SERVER__FEDERATION__TRUSTED_SERVERS__{N}__KEY_ID` - expected ed25519 key identifier referenced in incoming signatures for the server above.
- `OPENGUILD_SERVER__FEDERATION__TRUSTED_SERVERS__{N}__VERIFYING_KEY` - URL-safe base64 ed25519 public key used to verify PDUs from the server above.
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
- `--session-fallback-verifying-key <key>` - append one or more base64 ed25519 public keys that remain valid while rotating the active signing key (repeat flag per key).

The `/ready` endpoint reports `database` status using these settings. When a database URL is provided, the server performs an eager connection attempt during startup and surfaces either `configured` (success) or `error` (failure details); otherwise it reports `pending`.

> Federation trusted peers are configured via config/env today (see below). CLI flags will join once we need runtime mutability.

### Federation Trusted Servers

To accept remote PDUs you must declare which homeservers you trust. Each entry requires the peer's canonical server name, an ed25519 key identifier (e.g. `"1"`), and the URL-safe base64 verifying key that matches the peer's private signing key. Example `config/server.toml` excerpt:

```toml
[federation]
trusted_servers = [
  { server_name = "remote.example.org", key_id = "1", verifying_key = "l_fTdwEVGikNH87d...==" }
]
```

Environment-based setup uses indexed variables:

```
OPENGUILD_SERVER__FEDERATION__TRUSTED_SERVERS__0__SERVER_NAME=remote.example.org
OPENGUILD_SERVER__FEDERATION__TRUSTED_SERVERS__0__KEY_ID=1
OPENGUILD_SERVER__FEDERATION__TRUSTED_SERVERS__0__VERIFYING_KEY=l_fTdwEVGikNH87d...==
```

Leaving `trusted_servers` empty keeps `/federation/transactions` disabled (HTTP 501 + `{"disabled":true}`). When populated, the server verifies that the request `origin` matches a trusted entry, that each event was emitted by that origin, and that the `signatures` map includes `ed25519:{key_id}` with a valid ed25519 signature over the canonical event hash.

### MLS Key Packages

When MLS is enabled (via config or environment), the server hosts `/mls/key-packages` for authenticated clients and `/federation/channels/{id}/events` for trusted homeservers. Provide at least one identity:

```toml
[mls]
enabled = true
ciphersuite = "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519"
identities = ["alice", "bob"]
```

Env variant:

```
OPENGUILD_SERVER__MLS__ENABLED=true
OPENGUILD_SERVER__MLS__IDENTITIES__0=alice
OPENGUILD_SERVER__MLS__IDENTITIES__1=bob
```

Key packages are derived on boot and can be rotated via future admin APIs.

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

Need to manage roles? The CLI exposes scoped helpers:

```bash
# Server-wide role (highest precedence)
cargo run --bin openguild-server -- \
  --database-url postgres://app:secret@localhost/openguild \
  assign-server-role --username admin --role owner
cargo run --bin openguild-server -- \
  --database-url postgres://app:secret@localhost/openguild \
  revoke-server-role --username admin --role owner

# Guild-level role
cargo run --bin openguild-server -- \
  --database-url postgres://app:secret@localhost/openguild \
  assign-guild-role --username admin --guild-id 62a8da03-1e15-46a1-9a2e-0cfd5d1cf5a5 --role admin

# Channel-level role
cargo run --bin openguild-server -- \
  --database-url postgres://app:secret@localhost/openguild \
  assign-channel-role --username admin --channel-id 28fbd745-4514-4bab-bfd5-acc8a2c2abd0 --role moderator
```

Supported role labels (case-insensitive): `owner`, `admin`, `moderator`, `maintainer`, `member`, `contributor`, `viewer`, `guest`. Server roles outrank guild roles, which outrank channel roles when permissions are evaluated.

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
