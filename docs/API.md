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

- `GET /health` — liveness probe, returns `ok`.
- `GET /version` — returns `{ "version": "<semver>" }` from package metadata.
