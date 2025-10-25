# OpenGuild Client API (Skeleton)

This document will capture the REST and WebSocket contracts exposed by the homeserver.

## Planned Sections

1. Authentication & session lifecycle
2. Guild and channel management endpoints
3. Messaging and reactions
4. Media upload and retrieval
5. Voice signaling
6. Bot and webhook integration

Populate each section as the implementation progresses.

## Implemented (bootstrap)

- `GET /health` - liveness probe, returns `ok`.
- `GET /ready` - readiness probe, returns status, uptime and component list.
- `GET /version` - returns `{ "version": "<semver>" }` from package metadata.
- `POST /sessions/login` - issues a signed session token when credentials validate.

### Response Headers

- Every HTTP response includes an `X-Request-Id` header. If the client supplies one it is echoed back; otherwise the server generates a UUIDv4 so logs, metrics, and client traces can be correlated.

## Authentication & Session Lifecycle

> **Bearer tokens:** Unless documented otherwise, all mutable messaging and guild endpoints require clients to supply an `Authorization: Bearer <access_token>` header using a token issued by `POST /sessions/login`. Requests missing the header, presenting an expired token, or using an invalid signature return HTTP 401. Verification failures inside the server surface as HTTP 500.

### `POST /sessions/login`

Initiates a session for the supplied identifier/secret pair and provisions a refresh token that binds the client device. The current prototype authenticates against an in-memory store (suitable for local development); issued sessions persist to Postgres when a database pool is available (see `backend/migrations/0002_create_sessions.sql` for access tokens and `backend/migrations/0005_refresh_sessions.sql` for refresh tokens). When Postgres is absent, both access and refresh records remain in-memory for the process lifetime.

- **Success**: returns HTTP 200 with a JSON payload containing a signed access token, its expiry, a refresh token (base64url UUID), and refresh expiry.
- **Validation error**: returns HTTP 400 with a list of field errors (identifier, secret, device metadata).
- **Invalid credentials**: returns HTTP 401 with `{"error":"invalid_credentials"}`.

#### Request Body

```json
{
  "identifier": "alice@example.org",
  "secret": "supersecret",
  "device": {
    "device_id": "alice-laptop",
    "device_name": "Alice's Dev Laptop"
  }
}
```

- `device.device_id` - **required**, caller-supplied stable identifier per physical/browser device. Used as the natural key for the refresh session (per-user unique).
- `device.device_name` - optional display label persisted for audit tooling.
- IP metadata is inferred from the first entry in `X-Forwarded-For` when the header is present; otherwise the remote socket address is recorded server-side.

#### Successful Response

```json
{
  "access_token": "eyJzZXNzaW9uX2lkIjoiYmI4NzJiZjAt...snip...SzM1MjQifQ.7QAuPNJxjZO2q6WmyRjGy_qKSLqoTj_xdG9aQa2bjRw",
  "access_expires_at": "2025-10-12T21:34:26.123456Z",
  "refresh_token": "9cS8nB_zV7rVk7H4q4TRCQ",
  "refresh_expires_at": "2025-11-11T21:34:26.123456Z"
}
```

> The access token is a base64url-encoded JSON payload followed by an ed25519 signature. The refresh token is a base64url UUID referencing `refresh_sessions.refresh_id`. Both expirations are expressed in RFC 3339 with fractional seconds.

#### Validation Error (HTTP 400)

```json
{
  "error": "validation_error",
  "details": [
    { "field": "identifier", "message": "must be provided" },
    { "field": "secret", "message": "must be provided" }
  ]
}
```

#### Invalid Credentials (HTTP 401)

```json
{
  "error": "invalid_credentials"
}
```

#### Quick Test (curl)

```bash
curl -X POST http://127.0.0.1:8080/sessions/login \
  -H "content-type: application/json" \
  -H "x-forwarded-for: 203.0.113.42" \
  -d '{"identifier":"alice@example.org","secret":"supersecret","device":{"device_id":"alice-laptop","device_name":"Alice\'s Dev Laptop"}}'
```

> When `DATABASE_URL` (or `OPENGUILD_SERVER__DATABASE_URL`) is set, the server will upsert each access token into the `sessions` table and each refresh token into `refresh_sessions` alongside device metadata, last-seen timestamps, and inferred IP addresses. Subsequent logins with the same `device_id` replace the stored refresh token for that device while retaining audit history.

### `POST /users/register`

Registers a new user account. Requires a configured database; returns `503 Service Unavailable` when `database_url` is absent. Usernames are unique (case-sensitive) and passwords must be at least eight characters (Argon2id hashed).

- **Success**: returns HTTP 201 with the created user identifier and echoed username.
- **Validation error**: returns HTTP 400 with field errors for `username` and `password`.
- **Username conflict**: returns HTTP 409 with `{"error":"username_taken"}`.
- **Database unavailable**: returns HTTP 503 with `{"error":"database_unavailable"}`.

#### Request Body

```json
{
  "username": "alice",
  "password": "supersecret"
}
```

#### Successful Response

```json
{
  "user_id": "5f6171fb-4c76-43f7-9e2f-5b6f5fd278af",
  "username": "alice"
}
```

#### Username Conflict (HTTP 409)

```json
{
  "error": "username_taken"
}
```

### `POST /sessions/refresh`

Returns a fresh access token (and rotated refresh token) when provided with a valid, unexpired refresh token.

- **Success**: HTTP 200 with the same response schema as `POST /sessions/login`.
- **Invalid token / expired / revoked**: HTTP 401 with `{ "error": "invalid_refresh_token" }`.

#### Request Body

```json
{
  "refresh_token": "9cS8nB_zV7rVk7H4q4TRCQ"
}
```

#### Quick Test (curl)

```bash
curl -X POST http://127.0.0.1:8080/sessions/refresh \\
  -H "content-type: application/json" \\
  -d "{\"refresh_token\":\"9cS8nB_zV7rVk7H4q4TRCQ\"}"
```

### `POST /sessions/revoke`

Revokes a refresh token (e.g., on logout). Returns HTTP 204 even if the token is unknown to avoid leaking token state.

#### Request Body

```json
{
  "refresh_token": "9cS8nB_zV7rVk7H4q4TRCQ"
}
```

#### Quick Test (curl)

```bash
curl -X POST http://127.0.0.1:8080/sessions/revoke \\
  -H "content-type: application/json" \\
  -d "{\"refresh_token\":\"9cS8nB_zV7rVk7H4q4TRCQ\"}"
```

$null
#### Quick Test (curl)

```bash
curl -X POST http://127.0.0.1:8080/users/register \
  -H "content-type: application/json" \
  -d '{"username":"alice","password":"supersecret"}'
```

### CLI Seeding (`seed-user` subcommand)

For automated environments you can seed an account without hitting the HTTP API:

```bash
cargo run --bin openguild-server -- \
  --database-url postgres://app:secret@localhost/app \
  seed-user --username alice --password supersecret
```

- Respects the same configuration overrides as the server (`--host`, env vars, etc.).
- Exits successfully when the user already exists (logs a message and skips duplication).

## Guilds & Channels (Week 4 bootstrap)

### `POST /guilds`

Creates a new guild with a human-readable name (1–64 Unicode scalar values after trimming). Requires a valid bearer token. Returns HTTP 200 with the created guild record.

```json
{
  "guild_id": "f0b6ebd0-9e1b-4d67-8b66-08bf5b84b0e1",
  "name": "Example Guild",
  "created_at": "2025-10-12T23:59:04.315409Z"
}
```

- **Validation**: `name` must be non-empty and no longer than 64 characters (HTTP 400 on failure).
- **Auth failures**: missing or invalid access tokens return HTTP 401.

### `GET /guilds`

Lists all guilds known to the server, ordered by creation time. Requires a valid bearer token.

```json
[
  {
    "guild_id": "f0b6ebd0-9e1b-4d67-8b66-08bf5b84b0e1",
    "name": "Example Guild",
    "created_at": "2025-10-12T23:59:04.315409Z"
  }
]
```

### `POST /guilds/{guild_id}/channels`

Creates a channel within the specified guild (1–64 Unicode scalar values after trimming). Requires a valid bearer token.

```json
{
  "channel_id": "3b7f3e93-7c9c-47f5-91d3-cbf09dc5a8f6",
  "guild_id": "f0b6ebd0-9e1b-4d67-8b66-08bf5b84b0e1",
  "name": "general",
  "created_at": "2025-10-12T23:59:07.771025Z"
}
```

- **Validation**: `name` must be non-empty and no longer than 64 characters. Attempting to create a channel for a missing guild returns HTTP 404.
- **Auth failures**: missing or invalid access tokens return HTTP 401.

### `GET /guilds/{guild_id}/channels`

Lists channels for the guild (HTTP 200, empty array when none exist). Requires a valid bearer token.

- **Errors**: returns HTTP 404 if the guild ID is unknown.

### `POST /channels/{channel_id}/messages`

Appends an optimistic message event to the channel event log. Requires a valid bearer token. The route accepts a JSON payload with `sender` and `content` fields. Both fields must be non-empty; `content` is limited to 4,000 Unicode scalar values after trimming. The `sender` must match the authenticated user's identifier; mismatches return HTTP 403. Each authenticated user may submit up to 60 messages per rolling 60 seconds (3 per window in tests); excess requests return HTTP 429. Client IPs (derived from X-Forwarded-For, falling back to unknown) share a limit of 200 messages per 60 seconds (5 in tests); violating the shared window also returns HTTP 429.

```json
{
  "sender": "ec7c5138-ee1a-4112-95ef-e53514b670c2",
  "content": "Hello from OpenGuild!"
}
```

Successful requests respond with the sequence number generated by the storage layer:

```json
{
  "sequence": 1,
  "event_id": "$4nTFVwMCeV7zFJmioq5uPyrJ9Wscb1LZ6y3HfHE9mA1S",
  "created_at": "2025-10-12T23:59:12.412457Z"
}
```

Sequences increase monotonically per channel. When a Postgres pool is configured, events are persisted to `channel_events`; otherwise an in-memory journal is used for local development.

- **Errors**: validation failures return HTTP 400 (including length constraints); missing channels return HTTP 404; mismatched sender identities return HTTP 403; missing or invalid access tokens return HTTP 401; rate-limit violations return HTTP 429.

#### Quick Test (curl)

```bash
CHANNEL_ID=3b7f3e93-7c9c-47f5-91d3-cbf09dc5a8f6
ACCESS_TOKEN="eyJhbGciOiJFZERTQSIsInR5..."
curl -X POST http://127.0.0.1:8080/channels/$CHANNEL_ID/messages \
  -H "content-type: application/json" \
  -H "authorization: Bearer $ACCESS_TOKEN" \
  -d '{"sender":"ec7c5138-ee1a-4112-95ef-e53514b670c2","content":"hello world"}'
```

## WebSocket Streaming

### `GET /channels/{channel_id}/ws`

Upgrades to a WebSocket that streams channel events to connected clients. Clients must supply a valid bearer token using the `Authorization` header. Unauthorized requests receive HTTP 401 before the handshake.

- On connection the server replays the most recent 50 events (oldest to newest) so clients can warm their timeline.
- New events are broadcast fan-out style using a bounded queue (capacity 256 per channel). If a client falls behind, the server closes the socket with close code `POLICY` and a short reason.
- Idle connections must respond to ping/pong frames; writes are timed out after 10 seconds to avoid wedging the broadcast loop.
- A global semaphore caps concurrent connections (currently 256). Excess clients receive HTTP 429 prior to upgrade.
- Requests for unknown channels return HTTP 404 before the handshake completes.
- Missing or invalid bearer tokens return HTTP 401 before the handshake completes.

Each WebSocket message is a JSON object shaped as:

```json
{
  "sequence": 42,
  "channel_id": "3b7f3e93-7c9c-47f5-91d3-cbf09dc5a8f6",
  "event": {
    "event_id": "$6kXQ1dTgvYyTF8uZnD8QyUQ9oP3Gvc9nR7fN6vXqBZ3F",
    "event_type": "message",
    "room_id": "3b7f3e93-7c9c-47f5-91d3-cbf09dc5a8f6",
    "sender": "ec7c5138-ee1a-4112-95ef-e53514b670c2",
    "origin_server": "api.openguild.test",
    "origin_ts": 1749681600000,
    "content": { "content": "hello world" },
    "prev_events": [],
    "auth_events": [],
    "signatures": {}
  }
}
```

The `event` payload is a canonical OpenGuild event (see `openguild-core::event`) and can be fed directly into future federation workflows.

### `GET /channels/{channel_id}/events`

Returns a paginated slice of recent events for the channel. Requires a valid bearer token. Accepts optional query parameters:

- `limit` – maximum number of events to return (default `50`, max `200`).
- `since` – only return events with `sequence` greater than this value (handy for incremental sync).

Responses are ordered by sequence ascending:

```json
[
  {
    "sequence": 41,
    "channel_id": "3b7f3e93-7c9c-47f5-91d3-cbf09dc5a8f6",
    "event": {
      "schema_version": 1,
      "event_id": "$6kXQ1dTgvYyTF8uZnD8QyUQ9oP3Gvc9nR7fN6vXqBZ3F",
      "event_type": "message",
      "room_id": "3b7f3e93-7c9c-47f5-91d3-cbf09dc5a8f6",
      "sender": "ec7c5138-ee1a-4112-95ef-e53514b670c2",
      "origin_server": "api.openguild.test",
      "origin_ts": 1749681600000,
      "content": { "content": "hello world" },
      "prev_events": [],
      "auth_events": [],
      "signatures": {}
    }
  }
]
```

- **404** when the channel UUID is unknown.
- **401** when the bearer token is missing or invalid.
- **503** when messaging persistence is unavailable (in-memory service not initialized).
## Canonical Events

Every persisted or federated message is wrapped in a canonical envelope produced by `openguild-core::event`. Important fields:

- `schema_version`: unsigned integer describing the canonical JSON layout. Version `1` is the current default; bumps indicate hash/signature changes.
- `event_id`: `$`-prefixed base58 string derived from the BLAKE3 hash of the canonical event bytes (event ID changes if any canonical field mutates).
- `origin_server`: homeserver name from `ServerConfig::server_name` (e.g. `api.openguild.test`).
- `room_id`: UUID string pointing at the channel the event belongs to.
- `event_type`: `message` today, with future enum expansion.
- `origin_ts`: UTC milliseconds when the event was created.
- `sender`: authenticated user identifier (UUID string).
- `content`: domain payload. The MVP uses `{ "content": "<body>" }`.
- `prev_events` / `auth_events`: DAG metadata stubs reserved for future federation threading.
- `signatures`: map keyed by homeserver (outer) and `ed25519:<key_id>` (inner). The server fills this when signing outgoing PDUs.

Clients should treat unknown fields as opaque so schema migrations remain additive. The canonical hash excludes `event_id` and `signatures`, so verifiers can recompute the expected digest before comparing event IDs or validating signatures.

## Federation Transactions

The first federation endpoint accepts batches of PDUs from trusted homeservers. Transactions are only enabled when the server is configured with at least one `federation.trusted_servers` entry (see `docs/SETUP.md`).

### `POST /federation/transactions`

Processes one or more PDUs from a remote origin. The body is intentionally close to the Matrix `/send/{txnId}` shape so we can interop later.

#### Request Body

```json
{
  "origin": "remote.example.org",
  "pdus": [
    {
      "schema_version": 1,
      "event_id": "$z4Hh8y5MEt1X...",
      "origin_server": "remote.example.org",
      "room_id": "!room:remote.example.org",
      "event_type": "message",
      "sender": "@remote-user:remote.example.org",
      "origin_ts": 1761426000000,
      "content": { "content": "hello from remote" },
      "prev_events": [],
      "auth_events": [],
      "signatures": {
        "remote.example.org": {
          "ed25519:1": "3O1qC0..."
        }
      }
    }
  ]
}
```

- `origin` must match a trusted server entry (`server_name`).
- Each PDU must declare the same `origin_server` and include a signature for `ed25519:{key_id}` configured for that origin.

#### Responses

- **Federation disabled**: HTTP 501 with `{"origin":"<request origin>","accepted":[],"rejected":[],"disabled":true}` when no trusted servers are configured.
- **All events accepted**: HTTP 202 with `{ "accepted": ["$eventId"], "rejected": [], "disabled": false }`.
- **Partial success**: HTTP 207 Multi-Status with both `accepted` and `rejected` arrays populated.
- **All events rejected**: HTTP 400 with `accepted: []` plus `rejected` entries containing `{ "event_id": "...", "reason": "..." }`.

The server logs each rejection with `origin`, `event_id`, and a human-readable reason (`missing signature`, `origin mismatch`, `invalid signature`, etc.) so operators can debug remote failures.

