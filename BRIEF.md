# BRIEF.md — Federated Discord (codename: **OpenGuild**)

> **Elevator pitch:** A Discord-like platform where anyone can run a server, and servers interoperate. Keep the familiar UX (guilds, channels, roles, voice) while using a Matrix-inspired, signed-event federation layer and modern E2EE (MLS/SFrame) for privacy.

---

## 1) Goals & Non-Goals

### Goals

* Discord-style UX: guilds (servers), text channels, voice channels, roles/permissions, DMs, reactions.
* First-class **federation**: multi-server rooms/guilds with conflict-free state convergence.
* **Private by default**: E2EE for DMs/private channels; transport security for all traffic.
* **Composable**: clean Client API, Bot API, and S2S Federation API.
* **Practical deployment**: single binary + Postgres + S3-compatible media store; runs on a $10 VPS; scales out.

### Non-Goals (initially)

* Perfect compatibility with Discord clients, APIs, or bots.
* Full feature parity (threads, forum channels, stages, video streaming) before federation & voice are solid.
* Public/global directory by default (opt-in discoverability later).

---

## 2) Architecture (High Level)

### Components

* **Homeserver** (Rust; actix-web/axum): Hosts users, guilds, channels it “owns,” and participates in federated rooms.
* **Federation Layer**: Signed events in a DAG (Matrix-inspired); HTTP(S)/QUIC S2S with HTTP Signatures (and/or mTLS).
* **Client Gateway**: JSON over HTTP/WebSocket for low-latency events, long-poll `/sync` or SSE.
* **Media Repo**: S3-compatible object store; `mxc://server/id` addressing with caching/AV hooks.
* **Voice SFU**: Single-server first (mediasoup/LiveKit/Pion); later SFU↔SFU peering for cross-server voice.

### Data Stores

* **Postgres**: events (append-only), state snapshots, membership, roles, rate limits, audit.
* **Object storage**: attachments, avatars, voice recordings (optional).
* **Search**: SQLite FTS or Meilisearch for message search.

### Internal Bus

* NATS or Redis Streams for fan-out to gateway workers & background jobs (search indexing, media, moderation).

---

## 3) Federation Model

### Event Envelope (canonical JSON; deterministic field order for hashing)

```json
{
  "event_id": "$<hash>",                 // hash of canonical JSON + origin signature
  "room_id": "!<id>:example.com",        // guild or channel/room scope
  "origin_server": "example.com",
  "type": "m.room.message",
  "sender": "@alice:example.com",
  "origin_ts": 1733359200123,
  "content": { "...": "..." },
  "prev_events": ["$hashA","$hashB"],    // DAG edges
  "auth_events": ["$hashX","$hashY"],    // power/perm context
  "signatures": { "example.com": { "ed25519:abc": "<base64sig>" } }
}
```

### State & Conflict Resolution

* State is the union of **auth events** (power levels, membership, roles, channel defs).
* Initially: linear history per room (simplify).
* Then: power-aware merge (highest authorized chain wins; timestamp + lexicographic tiebreakers).
* Long-term: Matrix-style resolver for edge cases.

### Auth/Trust

* User IDs: `@name@homeserver`.
* Server trust: HTTP Signatures + rotating server keys; optional mTLS; publish keys to a transparency log.
* Server policy: allowlists/blocklists, rate limits, join throttles, spam defences.

---

## 4) Identity, Auth, and Permissions

* **Client auth**: Home server OIDC access tokens (short-lived), refresh tokens; scopes per guild/bot.
* **S2S auth**: HTTP Signatures (or mTLS) on every transaction; per-event Ed25519 signature by origin.
* **Permissions**: Discord-style bitflags + `m.power_levels`; roles are state events `m.guild.role` with bitmasks.
* **Join flows**: invite-only by default; public discovery opt-in.

---

## 5) Core Entities

* **Guild**: container for channels/roles; state events: `m.guild.create`, `m.guild.role`, `m.guild.meta`.
* **Channel (Text/Voice)**: `m.channel.create` (`kind:text|voice`), topic, parent (for category/space).
* **Member**: `m.room.member` (join|leave|ban|invite); membership drives access and voice authorization.
* **Message**: `m.room.message` (`msgtype:text|file|embed`), `m.reaction`, `m.redaction`.
* **Typing/Presence/Reads**: ephemeral EDUs (`m.typing`, `m.presence`, `m.read`).

---

## 6) Encryption

* **DMs/Private channels**: **MLS** (OpenMLS) preferred; alternative Olm/MEGOLM.

  * Keys distributed via MLS KeyPackages; rekey on membership changes.
* **Voice**: **SFrame** end-to-end keys per voice channel; server is SFU (no decrypt). Rekey on joins/leaves.
* **Key backup**: optional encrypted backup on home server; device verification & cross-signing.

---

## 7) Client API (Sketch)

### Transport

* REST for CRUD; WebSocket or SSE `/sync` for events.

### Selected Endpoints

* `POST /client/v1/login` → tokens
* `GET /client/v1/sync?since=...` → incremental timeline/state
* `POST /client/v1/guilds` → create guild
* `GET /client/v1/guilds/:id` / `PATCH` / `DELETE`
* `POST /client/v1/channels` (guild_id, kind)
* `GET /client/v1/rooms/:room_id/messages?from=...&limit=...`
* `POST /client/v1/rooms/:room_id/send` (message)
* `PUT /client/v1/media/upload` → `mxc://...`
* `GET /client/v1/media/:server/:media_id`
* `POST /client/v1/invite` (room_id, user_id)
* `POST /client/v1/roles` (guild_id, bitmask, name)
* `POST /client/v1/voice/:channel_id/join` → SDP/ICE exchange bootstrap
* `POST /client/v1/reactions` / `POST /client/v1/redact`

### Bot/OAuth

* `POST /oauth2/token` (client_credentials)
* Webhooks: signed POSTs for selected events; slash commands via signed callbacks.

---

## 8) Federation API (Sketch)

* `POST /_federation/send/{txn_id}` → { origin, pdus[], edus[] } → `{received: [event_ids...]}`
* `GET  /_federation/backfill?room_id=...&from=...` → historical PDUs
* `GET  /_federation/state?room_id=...` → current auth/state snapshot
* `POST /_federation/invite` → signed membership event proposal
* `GET  /_federation/media/:server/:media_id` → media fetch (with policy checks)
* **Voice peering (later)**: `/ _federation/voice/subscribe`, `/ice`, `/tracks` to signal SFU↔SFU links.

---

## 9) Voice Architecture

* **Phase 1 (single-server)**: WebRTC SFU hosted by the channel’s server; publish/subscribe tracks by membership.
* **Phase 2 (federated)**: SFU↔SFU peering; forward SRTP selectively; membership & perms validated via federation state.
* **Security**: DTLS-SRTP with **SFrame**; keys rotated on membership change; no server decrypt.

---

## 10) Moderation & Safety

* **Server tools**: join rate limits, invite expiry, keyword/URL filters, media scanning hooks, IP reputation.
* **Guild tools**: shadow-mute, slowmode, quarantine channels, report API, audit log.
* **Federated safety**: signed blocklists/allowlists, shareable between admins; appeal flows with signed decisions.
* **Abuse resistance**: proof-of-work or stamp-based joins for open guilds; per-origin rate limits; anti-raid throttles.

---

## 11) Discovery

* Default **invite-only**.
* Optional index: servers publish signed guild descriptors to a directory (ActivityPub-like collection or DHT).
* Search respects visibility flags; do not leak membership or message metadata for private rooms.

---

## 12) Performance & Scaling

* **Write path**: append-only events, dedup by `event_id`; batch fan-out; backpressure on hot rooms.
* **Hot shards**: partition by `room_id` hash; gateway shards subscribe per room.
* **Caches**: state snapshot cache per room; media CDN or reverse proxies.
* **Latency targets**: p50 < 200 ms event delivery within a server, < 500 ms cross-server.

---

## 13) Observability & Ops

* **Tracing**: OpenTelemetry (distributed traces: client → homeserver → federation).
* **Metrics**: Prometheus (event ingress/egress, state res time, WS connections, SFU stats).
* **Logs**: structured JSON with request IDs; audit events are signed and retained.
* **Migrations**: SQLx/Alembic-style migrations; blue/green deploys; snapshot/restore scripts.

---

## 14) Security & Threat Model (Summary)

* **Network**: TLS 1.3 everywhere; CSP on client; token binding; secure cookies where relevant.
* **Identity**: rotate server keys; publish to transparency log; device verification for user keys.
* **DoS/Spam**: global and per-origin rate limits; circuit breakers; greylisting unknown servers.
* **Privacy**: E2EE for DMs/private channels; minimize metadata leaks in federation APIs.

---

## 15) Tech Choices

* **Backend**: Rust (axum or actix-web), Tokio, SQLx (Postgres).
* **Message Bus**: NATS (preferred) or Redis Streams.
* **Media**: S3-compatible (MinIO for dev).
* **Search**: SQLite FTS (MVP) → Meilisearch (scale).
* **Voice**: Pion (Go) or mediasoup (C++/Node) or LiveKit (Go). Start with one.
* **Client**: Nuxt 3 + Pinia + Tailwind; Tauri/Electron wrapper optional.

---

## 16) Project Milestones & Deliverables

### M0 — Single-Server Text MVP (2–4 wks)

* Users, guilds, text channels, roles, messages, reactions.
* REST + WS gateway; Postgres + S3; basic moderation; DMs (non-E2EE).
* Deliverables: runnable docker-compose; minimal Nuxt client.

### M1 — E2EE for DMs/Private (2–3 wks)

* MLS integration; device management; key backup (optional).

### M2 — Federation (Text) (3–6 wks)

* S2S `/send`, backfill, state snapshots; event signatures; basic resolver; media fetch/caching.
* Fed policies: allow/block lists; rate limiting.

### M3 — Bots & Webhooks (2–3 wks)

* OAuth2 client creds; slash commands; outbound webhooks; intent filters.

### M4 — Voice (Single-Server) (3–5 wks)

* SFU; signaling over client API; SFrame optional; mute/deafen; basic UI.

### M5 — Federated Voice (4–8 wks)

* SFU↔SFU peering; membership-based auth; SFrame keying + rekey.

### M6 — Polish & Extras (ongoing)

* Threads, pins, edits, presence, typing, read receipts; directory opt-in; improved moderation.

---

## 17) Directory Layout (Proposed)

```text
/backend
  /crates
    core/           # events, state, signatures, canonical JSON
    server/         # HTTP/WS, federation routes
    storage/        # Postgres models, migrations
    media/          # S3 adapters, AV hooks
    crypto/         # ed25519, MLS glue
    sfu-client/     # (if external SFU) signaling lib
  /migrations
  Dockerfile
/frontend
  app/              # Nuxt 3
  components/, pages/, stores/
  Dockerfile
/deploy
  docker-compose.yml
  k8s/              # manifests (optional)
/docs
  BRIEF.md
  PROTOCOL.md
  API.md
  THREATMODEL.md
```

---

## 18) Success Metrics

* **Reliability**: >99.9% monthly gateway uptime on a single node; message loss = 0 (once acked).
* **Latency**: p50 event delivery <200 ms intra-server; <500 ms inter-server.
* **Security**: no plaintext DMs/private messages; weekly key rotation.
* **Adoption**: N independent servers can join the same guild and exchange messages/voice seamlessly.

---

## 19) Open Questions / Next Decisions

* Choose **MLS** library (OpenMLS) vs. short-term Olm/MEGOLM rails.
* SFU choice (Pion vs mediasoup vs LiveKit) and peering strategy.
* Directory mechanism: ActivityPub collection vs. lightweight DHT.
* Canonical JSON format/versioning strategy (`room_version`, `api_version`).
* Governance of shared safety lists (format, signatures, distribution cadence).

---

## 20) Risks & Mitigations

* **Federated spam/raids** → default invite-only, server reputation, proof-of-work joins, shared blocklists.
* **State divergence bugs** → start linear, extensive property tests, replay logs, determinism checks.
* **Voice complexity** → ship single-server first; defer peering until text federation is stable.
* **Crypto UX** → device bootstrap & recovery flows; cross-signing to reduce key prompts.

---

## 21) Minimal Canonicalization & Signature (Example)

```text
1) Serialize event content with canonical JSON (sorted keys, no whitespace variance).
2) event_hash = BLAKE3(canonical_json_bytes)
3) signature = Ed25519.sign(server_private_key, event_hash)
4) event_id = "$" + base58(event_hash)
5) Publish with signatures[origin_server]["ed25519:keyid"] = base64(signature)
```

---

## 22) Quick Start (Dev)

* `docker compose up` (Postgres, MinIO, NATS)
* `cargo run -p server` → launches client + federation endpoints
* `pnpm -C frontend dev` → Nuxt client (localhost)
* Create a guild, a text channel, post a message; verify events in DB.
* Spin a second server on a different port with its own signing key; join the same room; send messages both ways.

---

## 23) License & Governance (TBD)

* Likely **AGPLv3** (server) + **Apache-2.0** (client SDKs) to keep federation open while encouraging ecosystem growth.

---

**Owner:** Kyle (architecture, BE)
**Contributors:** TBD (client, SFU, crypto)
**Tracking:** `/docs/PROTOCOL.md`, `/docs/API.md`, issues labeled `M0…M5`.

> Next action: scaffold repo (backend crates + Nuxt app) and implement **M0** endpoints and storage.
