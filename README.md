# OpenGuild

Federated Discord-like platform inspired by Matrix federation and modern end-to-end encryption. This repo tracks the implementation milestones described in `BRIEF.md`.

## Repository Layout

- `backend/` — Rust workspace powering the homeserver, federation endpoints, media services, and shared libraries.
- `frontend/` — Nuxt 3 application for the web client.
- `deploy/` — Infrastructure-as-code assets (Docker Compose, Kubernetes manifests).
- `docs/` — Extended architecture, protocol, and API specifications.

See `docs/ROADMAP.md` for milestone tracking and `docs/SETUP.md` for environment setup (to be authored).
