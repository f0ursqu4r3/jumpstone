# Federation Protocol Notes (Draft)

This document expands on the federation model outlined in `../BRIEF.md`.

## Event Graph

- Deterministic canonical JSON payloads.
- Event IDs derived from the BLAKE3 hash of the canonical event body and rendered as `$`-prefixed base58.
- Auth chains defined by `auth_events` references.

### Channel Messaging (Week 4 bootstrap)

- Canonical event envelope emitted by the HTTP API and WebSocket fan-out (see `openguild-core::event`):
  - `schema_version`: currently `1`. Bumping this value changes the canonical JSON bytes (and thus event IDs/signatures).
  - `event_id`: `$`-prefixed base58 string derived from the BLAKE3 hash of the canonical event body (excluding `event_id` and `signatures`).
  - `room_id`: stringified `channel_id` (`UUID`), reused for future multi-homeserver rooms.
  - `event_type`: `"message"` for chat payloads (more types will follow).
  - `sender`: authenticated user identifier (currently a UUID string sourced from the access token subject).
  - `origin_server`: hostname reported by the current server instance (`ServerConfig::server_name`).
  - `origin_ts`: millisecond timestamp captured when the event is built.
  - `content`: JSON object containing domain-specific payload (`{ "content": "<body>" }` for MVP). Message bodies exceeding 4,000 Unicode scalar values are rejected by the homeserver.
  - `prev_events`/`auth_events`: currently empty lists; present for future DAG threading.
  - `signatures`: map keyed by origin server with inner keys matching `ed25519:<key_id>`. Locally generated events sign with the active homeserver key; inbound federation events must carry signatures that match trusted peer metadata.

- Optimistic persistence layer (`backend/migrations/0003_messaging.sql`):
  - `guilds` / `channels` tables anchor CRUD metadata.
  - `channel_events` table stores canonical event JSON plus a monotonic `sequence` (BIGSERIAL) per channel for ordering guarantees.
  - All writes are upserts guarded by unique `event_id` to avoid duplicate delivery.
  - An in-memory fallback mirrors the same semantics when Postgres is not configured (dev ergonomics).
  - Refresh session persistence (`backend/migrations/0005_refresh_sessions.sql`) captures per-device refresh UUIDs, inferred IPs, and audit timestamps so auth refresh/revocation can participate in future federation gossip.

  - Refresh rotation exposes `/sessions/refresh` (rotate) and `/sessions/revoke` (logout) so downstream services can mirror state; each refresh token is a base64url UUID tied to device metadata.
- WebSocket fan-out (`GET /channels/{channel_id}/ws`):
  - Replays the most recent 50 events on connect to bootstrap the timeline.
  - Broadcast queue depth is capped at 256 messages; clients lagging beyond that receive a close frame (code `1011` policy violation) and must resubscribe.
  - Idle connections must respond to ping/pong frames; individual writes time out after 10 seconds to prevent backpressure from stalling the broadcaster.
  - A global semaphore limits concurrent sockets (currently 256) until adaptive admission control is added.

- Payload guardrails (current defaults):
  - Message body length validated client-side (server currently enforces non-empty; size limits will be added alongside content moderation work).
  - REST endpoints return structured `400` errors for validation issues and degrade to `503` when messaging storage is unavailable.

## Federation APIs

- `POST /federation/transactions` (Week 8 bootstrap)
  - Body: `{ "origin": "<server name>", "pdus": [CanonicalEvent, ...] }`
  - When `federation.trusted_servers` is empty the handler returns HTTP 501 with `{ "disabled": true }`.
  - Otherwise events are verified by recomputing the canonical hash, comparing the provided `event_id`, and validating the `ed25519:{key_id}` signature with the configured verifying key.
  - Results include `accepted` and `rejected` arrays so callers can retry failed PDUs. Failed events also produce structured warnings (`origin`, `event_id`, `reason`) in the server logs for audit visibility.
- `GET /federation/channels/{channel_id}/events`
  - Query params: `limit` (default 50, max 200) and `since` (sequence watermark).
  - Requires header `X-OpenGuild-Origin` and the origin must exist in `federation.trusted_servers`.
  - Returns `{ "origin": "<this server>", "channel_id": "<uuid>", "events": [{ "sequence": N, "event": CanonicalEventJson }, ...] }`.
  - Unknown channels return 404; missing/untrusted origins return 401/403; when federation is disabled the handler responds 501 with an empty payload.

## State Resolution

Specify linear resolver for MVP and planned progression to Matrix-style conflict resolution.

## Security

- Server key rotation cadence.
- HTTP Signature algorithm negotiation.
- Transparency log requirements.
- Refresh token posture: access tokens are short-lived ed25519-signed sessions (see `SigningKeyRing`), while refresh tokens are base64url UUIDs bound to device identifiers; rotation or revocation invalidates the row in `refresh_sessions` and will need to fan out over federation channels.

_Fill each section with precise specs as features land._
