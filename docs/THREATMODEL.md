# OpenGuild Threat Model (Initial Draft)

This living document enumerates assets, adversaries, attack surfaces, and mitigations.

## Assets

- End-to-end encrypted DM payloads
- Server signing keys & transparency log
- User access tokens and refresh tokens
- Media stored in object storage

## Adversaries

- Malicious federated servers
- Compromised clients or bots
- Network-level attackers (passive and active)
- Insider threats within a home server

## Attack Surfaces

- **Client API (REST/WS)** – includes authenticated messaging CRUD, WebSocket fan-out, session lifecycle.
- **Metrics Endpoint** – Prometheus scrape surface when enabled.
- Federation API endpoints *(future work)*.
- Media retrieval paths.
- Voice SFU signaling and transport.
- **MLS readiness UI** – key package exports, bootstrap modal copy actions, and local handshake verification state accessible via the dashboard.

## Mitigations (Current Status)

| Vector | Mitigation | Gaps / Follow-ups |
|--------|------------|-------------------|
| Unauthorized messaging access | Bearer token required on all messaging CRUD + WS routes; sender must match access token subject. | Expand RBAC once guild membership model lands. |
| Excessive payload sizes | Guild/channel names capped at 64 chars, message content at 4,000 chars, with rejection metrics logged (`openguild_messaging_rejections_total`). | Add configurable limits + client feedback docs. |
| Messaging spam / abuse | Server enforces per-user and per-IP sliding window limits (`openguild_messaging_rejections_total{reason in ["message_rate_limit","ip_rate_limit"]}`) with metric visibility. | Tune limits per environment and add adaptive backoff/ban lists. |
| WebSocket resource exhaustion | Global semaphore caps concurrent channel connections; attempts over capacity receive HTTP 429 and increment rejection metrics. | Evaluate shard-aware scaling once multi-node gateway lands. |
| Clickjacking, response sniffing | Global CSP (`default-src 'none'`), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer` applied at the gateway. | Review compatibility once UI embeds server responses. |
| Metrics exposure | Metrics listener can bind to separate interface; documentation instructs operators to ensure restricted network access. | Add auth/ACL story for multi-tenant deployments. |
| Session token theft | Access tokens signed with ed25519; refresh tokens bound to device metadata. | Enforce token rotation telemetry + anomaly detection. |
| MLS bootstrap phishing / token theft | `HomeView.vue` displays explicit device/server metadata, MLS copy actions log breadcrumbs via `recordBreadcrumb`, and `VITE_FEATURE_MLS_READINESS` gates the UI until operators opt in. Handshake verification timestamps persist locally so stale prompts stand out. | Add signed bootstrap instructions + clipboard hardening (auto-clear, warning modals) once MLS enrolment APIs land. |

### Outstanding Work

- Extend documentation for abusive pattern detection leveraging rejection metrics.
- Model exposure from future federation endpoints; define signature verification and rate limiting per peer.
- Capture audit logging plan (current middleware placeholder).
- Revisit threat posture once MLS integration progresses.
- Harden MLS clipboard exports (clear-after-copy, integrate signed device provisioning flows).
