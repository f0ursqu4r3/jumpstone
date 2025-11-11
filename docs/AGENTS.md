# OpenGuild · Agent Guide

This repo is actively developed by multiple AI/RA helpers. Use this guide to get oriented quickly before diving into new work.

## Current State (2025‑11‑04)

- **Frontend:** Vue 3 + Pinia + Vite with Nuxt UI. Auth, guild/channel shell, messaging, realtime, security/a11y, and federation awareness (Weeks 0‑8) are complete. Week 9 (MLS prep + device bootstrap) is next.
- **Backend:** Rust (Axum) APIs for auth, messaging, federation. WebSocket fan‑out requires bearer tokens or `?access_token=`.
- **Docs:** `docs/FRONTEND_TIMELINE.md` is the source of truth for milestone status. `docs/TESTING.md` maps feature areas to automated + manual checks. `BRAIN.txt` mirrors these highlights for quick reference.

## Workflows

| Area    | Commands / Notes |
| ------- | ---------------- |
| Backend | `cargo xtask test`, `cargo xtask ci`, `cargo xtask ci-metrics-smoke` |
| Frontend | `cd frontend && bun install`, `bun test:unit`, `bun run type-check`, `bun dev` |
| Testing extras | `STORYBOOK_TESTS=true bun test:unit` to include Storybook/Playwright. |

- Prefer Bun but npm/pnpm also work (see `frontend/README.md`).
- Vite aliases: `@/` → `frontend/src`, `~/` → root.

## Recent Highlights

- **Security & Accessibility (Weeks 6‑7):** Role‑aware gating (`frontend/src/utils/permissions.ts`), device inventory (`frontend/src/stores/devices.ts`), storage audit (`frontend/src/utils/storage.ts`), axe regression test (`frontend/tests/accessibility.test.ts`).
- **Federation Awareness (Week 8):** `useFederationStore` hydrates remote context + MLS handshake vectors; `DashboardView.vue` shows trust alerts, remote server lists, origin filters, and admin-only tooling; `AppMessageTimeline.vue` badges remote events and exposes “Copy meta”.

## Next Up (Week 9 Roadmap)

1. **MLS Key Packages**
   - Render `/mls/key-packages` per identity with rotation timestamps.
   - Add copy-to-clipboard + “missing package” warnings.
2. **Device Bootstrap**
   - Modal guiding new device registration (placeholder until backend lands).
   - Persist handshake verification results client-side.
   - Gate UI with feature flag/telemetry.
3. **Security Review**
   - Update `docs/THREATMODEL.md` with phishing/token theft notes.
   - Align UX copy with backend MLS requirements.

See the Week 9 section in `docs/FRONTEND_TIMELINE.md` for granular acceptance criteria.

## Tips for Future Agents

- **Reference hierarchy:** Timeline → BRAIN → README/TESTING; keep them in sync when you land work.
- **Telemetry:** Use `recordNetworkBreadcrumb` (see `frontend/src/utils/telemetry.ts`) for meaningful logging, especially around federation / MLS flows.
- **Clipboard helpers:** Reuse the guards in `DashboardView.vue` / `AppMessageTimeline.vue` when adding new copy-to-clipboard surfaces.
- **Feature flags:** Add new flags via `frontend/src/config/features.ts` and document them in `docs/FRONTEND_TIMELINE.md`.

Happy shipping! Keep milestone status boxes checked/unchecked as you progress so the next agent knows exactly where to pick up.
