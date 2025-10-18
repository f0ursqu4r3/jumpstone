# Testing Guide (Weeks 1-7)

This document maps each shipped backend feature in Weeks 1 through 7 to the automated tests that exercise it, plus the commands you should run locally and in CI. Treat it as the source of truth when adding new coverage or investigating regressions.

## How to Run the Full Suite

| Command                                           | When to Use               | Notes                                                                                                     |
| ------------------------------------------------- | ------------------------- | --------------------------------------------------------------------------------------------------------- |
| `cargo xtask test`                                | Default local run         | Executes `cargo test --workspace` across all crates (core, crypto, storage, server).                      |
| `cargo xtask ci`                                  | Pre-PR sanity             | Runs fmt + clippy + full test suite with warnings denied.                                                 |
| `cargo xtask ci-metrics-smoke`                    | Observability regressions | Builds the server with `--features metrics`, boots it on a random port, verifies `/ready` and `/metrics`. |
| `PROPTEST_CASES=256 cargo test -p openguild-core` | Stress property tests     | Increases proptest iteration count for event ID uniqueness.                                               |

> Need to target a specific crate? Use `cargo test -p <crate>` as listed in the sections below. All integration tests live in `openguild-server`, but depend on helper modules in storage and session harnesses.

## Feature Coverage by Week

### Week 1-2 · Server Foundation Hardening

| Area                                | Tests                                                                                                                                                                                                                               | Commands                                                                          |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Config loader defaults & precedence | `backend/crates/server/src/config.rs::environment_overrides_take_effect`, `::listener_addr_prefers_bind_addr`, `::apply_overrides_updates_fields`                                                                                   | `cargo test -p openguild-server config` (or full suite)                           |
| Readiness, uptime & telemetry       | `backend/crates/server/src/main.rs::readiness_route_reports_degraded_until_dependencies_exist`, `::readiness_reports_elapsed_uptime`, `::readiness_reports_configured_when_database_url_present`                                    | `cargo test -p openguild-server readiness`                                        |
| Metrics defaults                    | `backend/crates/server/src/main.rs::metrics_route_exposed_when_enabled`, `::metrics_route_absent_when_disabled`, `::metrics_route_served_on_dedicated_listener_when_configured` (plus `cargo xtask ci-metrics-smoke` runtime check) | `cargo test -p openguild-server metrics_route` and `cargo xtask ci-metrics-smoke` |

### Week 3 · Persistence & Session APIs

| Area                                       | Tests                                                                                                                                                       | Commands                                                                      |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Session DTO validation                     | `backend/crates/server/src/main.rs::login_route_rejects_blank_inputs`, `::login_route_returns_unauthorized_on_invalid_credentials`                          | `cargo test -p openguild-server login_route_*`                                |
| Happy-path login + refresh issuance        | `backend/crates/server/src/main.rs::login_route_returns_token_on_success`                                                                                   | `cargo test -p openguild-server login_route_returns_token_on_success`         |
| Session store lifecycle (access + refresh) | `backend/crates/server/src/session.rs::tests::login_emits_refresh_tokens`, `::refresh_rotates_tokens`, `::revoke_refresh_token_rejects_future_use`          | `cargo test -p openguild-server session::tests`                               |
| Storage readiness                          | `backend/crates/storage/src/lib.rs::tests::storage_pool_smoke`, `backend/crates/server/src/main.rs::readiness_reports_configured_when_database_url_present` | `cargo test -p openguild-storage`, `cargo test -p openguild-server readiness` |

### Week 4 · Messaging Core

| Area                                     | Tests                                                                                                                                                                                                                                                                             | Commands                                                                                                       |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Canonical event IDs & ordering           | `backend/crates/core/src/messaging.rs::tests::event_builder_produces_unique_event_ids` (proptest)                                                                                                                                                                                 | `cargo test -p openguild-core messaging`                                                                       |
| REST messaging flow & auth guards        | `backend/crates/server/src/main.rs::guild_channel_crud_endpoints`, `::create_guild_requires_bearer_token`, `::create_channel_requires_bearer_token`, `::post_message_requires_bearer_token`, `::post_message_rejects_sender_mismatch`, `::post_message_rejects_oversized_content` | `cargo test -p openguild-server guild_channel_crud_endpoints`, `cargo test -p openguild-server post_message_*` |
| WebSocket gateway join/leave + broadcast | `backend/crates/server/src/main.rs::websocket_requires_bearer_token`, `::websocket_rejects_when_capacity_reached`, `::websocket_broadcasts_events`                                                                                                                                | `cargo test -p openguild-server websocket_*`                                                                   |

### Week 5 · Observability & Reliability

| Area                            | Tests                                                                                                                                                                                                                                                                           | Commands                                                                                                                                |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| HTTP request IDs in traces      | `backend/crates/server/src/main.rs::request_id_propagates_into_traces_for_http`                                                                                                                                                                                                 | `cargo test -p openguild-server request_id_propagates_into_traces_for_http`                                                             |
| WebSocket request IDs & logging | `backend/crates/server/src/main.rs::websocket_connection_limit_logs_request_id`                                                                                                                                                                                                 | `cargo test -p openguild-server websocket_connection_limit_logs_request_id`                                                             |
| Metrics surfacing               | `backend/crates/server/src/main.rs::metrics_route_exposed_when_enabled`, `::metrics_route_absent_when_disabled`, `::metrics_route_served_on_dedicated_listener_when_configured`, `::messaging_metrics_reflect_event_activity`; runtime check via `cargo xtask ci-metrics-smoke` | `cargo test -p openguild-server metrics_route_*`, `cargo xtask ci-metrics-smoke`                                                        |
| Rate-limit burst protection     | `backend/crates/server/src/main.rs::post_message_hits_rate_limit`, `::websocket_rejects_when_capacity_reached`                                                                                                                                                                  | `cargo test -p openguild-server post_message_hits_rate_limit`, `cargo test -p openguild-server websocket_rejects_when_capacity_reached` |

### Week 6-7 · Security & Posture

| Area                              | Tests                                                                                                                                                  | Commands                                                                                                                                                                                                     |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Messaging authz & sender matching | `backend/crates/server/src/main.rs::post_message_requires_bearer_token`, `::post_message_rejects_sender_mismatch`, `::websocket_requires_bearer_token` | `cargo test -p openguild-server post_message_requires_bearer_token`, `cargo test -p openguild-server post_message_rejects_sender_mismatch`, `cargo test -p openguild-server websocket_requires_bearer_token` |
| Per-user rate limits              | `backend/crates/server/src/main.rs::post_message_hits_rate_limit`                                                                                      | `cargo test -p openguild-server post_message_hits_rate_limit`                                                                                                                                                |
| Per-IP rate limits                | `backend/crates/server/src/main.rs::post_message_hits_ip_rate_limit`                                                                                   | `cargo test -p openguild-server post_message_hits_ip_rate_limit`                                                                                                                                             |
| Security headers middleware       | `backend/crates/server/src/main.rs::health_route_returns_ok` (asserts CSP, referrer, nosniff, frame guard)                                             | `cargo test -p openguild-server health_route_returns_ok`                                                                                                                                                     |
| Refresh + revoke lifecycle        | `backend/crates/server/src/session.rs::tests::login_emits_refresh_tokens`, `::refresh_rotates_tokens`, `::revoke_refresh_token_rejects_future_use`     | `cargo test -p openguild-server session::tests`                                                                                                                                                              |
| Credential bootstrap validation   | `backend/crates/server/src/users.rs::tests::validation_rejects_short_passwords`, `::validation_accepts_valid_payload`                                  | `cargo test -p openguild-server users::tests`                                                                                                                                                                |

## Manual / Exploratory Checks

- **Docker Compose smoke** — `docker compose up server` (see `docs/SETUP.md`) and hit `/ready`, `/metrics`, `/channels/*` with HTTPie for manual regression checks when touching networking or auth.
- **Proptest amplification** — set `PROPTEST_CASES` environment variable higher when investigating flaky event ID generation: `PROPTEST_CASES=1024 cargo test -p openguild-core messaging`.
- **Load testing placeholder** — Week 8+ will add k6/wrk scenarios; track status in `docs/TIMELINE.md`.

## Expectations for New Work

1. Add or update tests in the feature-owning crate when behaviour changes. Mirror naming conventions (`post_message_*`, `websocket_*`, etc.) to keep discoverability high.
2. Update this document with new rows so onboarding engineers know where coverage lives.
3. Ensure `cargo xtask ci` and `cargo xtask ci-metrics-smoke` stay green before merging.
