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

## Authentication & Session Lifecycle

### `POST /sessions/login`

Initiates a session for the supplied identifier/secret pair. The current prototype authenticates against an in-memory store (suitable for local development); persistence will move to Postgres once the user schema is finalized.

- **Success**: returns HTTP 200 with a JSON payload containing a signed token and expiration timestamp.
- **Validation error**: returns HTTP 400 with a list of field errors.
- **Invalid credentials**: returns HTTP 401 with `{"error":"invalid_credentials"}`.

#### Request Body

```json
{
  "identifier": "alice@example.org",
  "secret": "supersecret"
}
```

#### Successful Response

```json
{
  "token": "eyJzZXNzaW9uX2lkIjoiYmI4NzJiZjAt...snip...SzM1MjQifQ.7QAuPNJxjZO2q6WmyRjGy_qKSLqoTj_xdG9aQa2bjRw",
  "expires_at": "2025-10-12T21:34:26.123456Z"
}
```

> Token and timestamp above are illustrative. The actual value encodes the session payload (base64url) followed by an ed25519 signature.

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
  -d '{"identifier":"alice@example.org","secret":"supersecret"}'
```
