# OpenGuild Delivery Timeline

This living document tracks backend-focused milestones, weekly targets, and shared to-do lists. Check off items as we complete them and append dates, owners, or notes inline.

## Working Assumptions

- [ ] Scope emphasises the Rust backend; call out frontend work only when it blocks validation.
- [ ] Maintain weekly checkpoints so scope stays bite-sized and collaborative.
- [ ] Ensure local infra (Postgres, MinIO, NATS via `deploy/docker-compose.yml`) is available before backend sprints start.
- [ ] Treat security, observability, and docs as first-class deliverables rather than follow-up chores.

## Week 1-2: Server Foundation Hardening (Milestone M0)

- [x] Expand `openguild-server` configuration and telemetry.
  - [x] Finalize `build_subscriber` coverage, including JSON log assertions with a captured writer.
  - [x] Expose `/metrics` behind a feature flag and document how to enable it locally.
  - [x] Add graceful shutdown integration test that simulates Ctrl+C.
- [x] Introduce `ServerConfig` loader via the `config` crate.
  - [x] Model configuration struct with defaults, validation, and error messaging.
  - [x] Layer config file and environment sources; ensure precedence is deterministic.
  - [x] Design CLI override story (flags or subcommand) if needed.
  - [x] Create table-driven tests covering valid/invalid permutations.
- [x] Refresh developer ergonomics and docs.
  - [x] Add lint/test shortcuts (make targets or cargo aliases) to the repo.
  - [x] Update `docs/SETUP.md` with configuration schema and troubleshooting tips.
  - [x] Outline CI matrix (Linux + Windows runners) for future GitHub Actions wiring (see docs/CI_PLAN.md).

## Week 3: Persistence & Session APIs (Milestone M0)

- [x] Scaffold Postgres connectivity and migrations.
  - [x] Create SQLx migration directory with baseline schema checked in.
  - [x] Implement pooled connection manager plus readiness probe hook (storage status in /ready).
  - [x] Add migration smoke test invoking `sqlx::migrate!()` during `cargo test` (skips unless `OPENGUILD_TEST_DATABASE_URL`/`DATABASE_URL` set).
- [x] Bootstrap session/auth flows.
  - [x] Define login DTOs, validation, and error mapping (`POST /sessions/login` plus structured error responses).
  - [x] Integrate signing via `openguild-crypto` with configurable key source (URL-safe base64 ed25519 key via config/env/CLI).
  - [x] Persist sessions via Postgres when configured (fallback to in-memory for auth; server tests cover issuance + storage).
- [x] Align with frontend expectations.
  - [x] Sync with `frontend/stores/session.ts` owners on contract details (documented login contract in `docs/API.md` and circulated to frontend repo owners).
  - [x] Publish request/response samples in `docs/API.md`.
  - [x] Add curl/HTTPie snippets to accelerate manual QA.

## Week 4: Messaging Core (Milestone M0)

- [x] Deliver room/channel CRUD with optimistic event persistence.
  - [x] Extend schema for guilds, channels, messages, and memberships (`backend/migrations/0003_messaging.sql`).
  - [x] Build repository layer plus optimistic event writer in `openguild-core`/`openguild-storage` (see `MessagingRepository` + `MessagePayload`).
  - [x] Write property tests for event IDs and ordering guarantees (proptest in `openguild-core::messaging`).
- [x] Stand up WebSocket gateway for single-server fan-out.
  - [x] Implement join/leave semantics and broadcast channel wiring (bounded broadcast channel per room).
  - [x] Enforce backpressure, connection caps, and timeout policies (256-slot buffer, ping/pong, send timeouts, global semaphore).
  - [x] Create integration test using `tokio_tungstenite` that exercises message flow.
- [x] Document messaging contracts.
  - [x] Update `docs/PROTOCOL.md` with event envelopes, persistence semantics, and guardrails.
  - [x] Capture sample payloads and error responses in `docs/API.md`.
  - [x] Note operational guardrails (payload limits roadmap, connection caps, replay window).

## Week 5: Observability & Reliability (Milestone M0 to M1 prep)

- [x] Implement structured tracing propagation and request IDs.
  - [x] Add middleware injecting correlation IDs and span context.
  - [x] Propagate identifiers through HTTP responses and WebSocket frames.
  - [x] Snapshot logs within tests to prove propagation.
- [x] Establish metrics and alerting baseline.
  - [x] Integrate Prometheus exporter and expose `/metrics` route.
  - [x] Instrument DB pool, WebSocket clients, and request latencies.
  - [x] Write tests that scrape metrics and assert key counters/gauges.
- [ ] Upgrade local + CI observability tooling.
  - [ ] Extend Docker Compose with Prometheus and Grafana services.
  - [ ] Document Grafana dashboards plus default alert thresholds.
  - [ ] Prototype CI sanity check that fails when metrics regress (stretch goal).
- [ ] Draft operations playbook.
  - [ ] Create `docs/OPERATIONS.md` (or expand `docs/SETUP.md`) with deploy/rollback workflows.
  - [ ] Include monitoring runbooks and alert escalation paths.
  - [ ] Outline incident response expectations for on-call rotations.

## Week 6-7: Security/Posture Hardening (Milestone M1 setup)

- [ ] Formalize auth token lifecycle with refresh + revocation.
  - [ ] Implement signing key rotation plumbing in `openguild-crypto` + server config.
  - [ ] Persist refresh tokens with device binding metadata and auditing hooks.
  - [ ] Add integration tests for refresh/revoke flows, including clock skew handling.
- [ ] Level up threat modelling and security headers.
  - [ ] Extend `docs/THREATMODEL.md` with new attack surfaces and mitigations.
  - [ ] Add middleware for CSP, rate limiting, and audit logging stubs.
  - [ ] Write tests asserting security headers and rate limiting behaviour under burst.

## Week 8 and Beyond: Federation & MLS (Milestones M1-M2)

- [ ] Integrate MLS key management.
  - [ ] Evaluate `openmls` versus alternatives and lock dependency choice.
  - [ ] Define provisioning API plus persistent key store schema.
  - [ ] Build handshake and signing verification test vectors.
- [ ] Deliver DAG-backed federation pipeline.
  - [ ] Finalize canonical event structure in `openguild-core` with versioning plan.
  - [ ] Implement signature verification service and failure telemetry emitters.
  - [ ] Build `/federation/*` endpoints and local peer integration harness.
- [ ] Explore SFU client signalling (stretch).
  - [ ] Map signalling requirements against existing SFU client crate.
  - [ ] Draft design doc for voice federation handshake flows.
  - [ ] Prototype DTOs shared between voice and federation services.

## Ongoing Backlog (Parallel Streams)

- [ ] Establish automated load/perf testing harness (wrk, k6, or Rust bench) with nightly execution.
  - [ ] Draft scenarios covering messaging, auth, media upload, and presence bursts.
  - [ ] Pipe reports into CI dashboard or metrics store.
- [ ] Introduce secrets management strategy (Vault or SOPS).
  - [ ] Evaluate tooling fit, bootstrap dev secrets workflow, and document onboarding.
  - [ ] Define rotation procedures, ownership, and compliance considerations.
- [ ] Expand developer tooling footprint.
  - [ ] Provide `cargo xtask` (or similar) commands for migrations, fixtures, smoke tests.
  - [ ] Add onboarding scripts to bootstrap new workstations end-to-end.
- [ ] Harden CI quality gates.
  - [ ] Add coverage reporting and enforce minimum threshold.
  - [ ] Schedule `cargo audit`/`cargo deny` with triage guidance.
  - [ ] Track flaky tests and establish remediation workflow.
Keep this document livingâ€”after each weekly sync, update status, adjust scope, annotate owners, and log new discoveries so we maintain momentum and clarity.
