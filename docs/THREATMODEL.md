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

- Client API (REST/WS)
- Federation API endpoints
- Media retrieval paths
- Voice SFU signaling and transport

## Mitigations

Document rate limiting, signature verification, TLS usage, MLS and SFrame key management, audit logging, and moderation tooling as they are implemented.
