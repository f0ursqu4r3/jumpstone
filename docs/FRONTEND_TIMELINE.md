# OpenGuild Frontend Delivery Timeline

This companion roadmap focuses on the Nuxt 3 web client. It mirrors the backend cadence so both sides converge on the same milestones. Update statuses, owners, links to design artifacts, and QA notes as work lands. Where backend support is required, call it out inline so dependencies stay visible.

## Working Assumptions

- [ ] Nuxt 3 + Pinia + TypeScript remain the primary stack; Tailwind powers the design system.
- [ ] API traffic flows through the backend documented in `docs/API.md`; avoid client-side schema drift by importing shared types when feasible.
- [ ] Treat accessibility and responsive layouts as first-class (WCAG AA target).
- [ ] Ship instrumentation alongside features (Sentry + LogRocket stubs, Lighthouse budgets, vitest coverage).

## Week 1-2: Client Foundation (Milestone F0)

- [x] Stabilize developer workflow.
  - [x] Document Bun/NPM parity commands (`dev`, `lint`, `test`, `build`) and CI hooks. `frontend/README.md` now outlines command parity plus CI expectations (`lint`, `test`, `build`, `preview`) and references the generated types alias.
  - [x] Add `.env.example` with API base URLs, feature flags, and mock toggles. `frontend/.env.example` ships defaults for API base URL, mock toggles, and devtools awareness.
  - [x] Wire Vite aliases to shared TypeScript types generated from the backend OpenAPI schema (placeholder script). `frontend/nuxt.config.ts`, `frontend/tsconfig.json`, and `frontend/vitest.config.ts` expose `@openguild/backend-types`; `pnpm types:sync` seeds `frontend/types/generated`.
- [ ] Establish design system + layout shell.
  - [x] Ship Tailwind tokens (color, spacing, typography) mapped to the brand palette. `frontend/app/assets/css/tokens.css`, `frontend/tailwind.config.ts`, and the updated `frontend/app/app.config.ts` define brand colors, spacing, and UI defaults.
  - [ ] Implement core components (Button, Input, Badge, Avatar, Tooltip) with Storybook stories and Chromatic snapshots. Base wrappers live in `frontend/app/components/ui/*`, now adopted across the shell, but Storybook scaffolding and visual regression remain TBD.
  - [x] Build the global app frame (navigation column, content area, status bar) with responsive breakpoints via `frontend/app/layouts/default.vue` and companion shell components.
- [ ] Scaffold state management and API client.
  - [x] Create an Axios/fetch wrapper with typed responses, request ID propagation, and retry/backoff policy. Nuxt plugin `frontend/app/plugins/api-client.ts` exposes `$api` leveraging `frontend/app/composables/useApiClient.ts` for auth headers and request IDs.
  - [x] Introduce Pinia stores for session, guilds, and channels with hydration helpers. `frontend/app/stores/guilds.ts` + `frontend/app/stores/channels.ts` replace inline mocks and feed `frontend/app/layouts/default.vue`.
  - [x] Add vitest + Testing Library setup; cover store mutations and HTTP client error handling. Suites now cover Pinia stores and API client behavior (`frontend/tests/*store.test.ts`, `frontend/tests/api-client.test.ts`).

## Week 3: Authentication & Landing Flows (Milestone F0)

- [ ] Implement session UX.
  - [x] Build login/register forms with validation, error toasts, and loading states.
  - [x] Persist access/refresh tokens in secure storage (IndexedDB fallback), respecting backend expiry semantics.
  - [x] Create device metadata prompts (friendly name) aligned with backend requirements.
- [ ] Route guards and onboarding.
  - [x] Add global middleware that enforces auth, refreshes tokens, and redirects to `/login` when needed.
  - [ ] Deliver onboarding carousel with links to docs and setup guides.
  - [ ] Smoke-test flows against backend `POST /sessions/login` + `/users/register`; record QA steps in `docs/TESTING.md`.

## Week 4: Guild & Channel Shell (Milestone F1)

- [ ] Guild discovery and selection.
  - [ ] Render the guild switcher with avatars, tooltips, and unread indicators (stub data + API integration).
  - [ ] Implement guild creation modal consuming backend `/guilds` POST.
  - [ ] Handle empty states (no guilds, invite-only messaging).
- [ ] Channel list + metadata.
  - [ ] Display channel tree (text/voice), sort order, and locks based on permissions.
  - [ ] Surface channel topic/description and breadcrumb within the content header.
  - [ ] Support skeleton/loading states using Suspense for SSR hydration.
- [ ] Timeline scaffold.
  - [ ] Render message timeline (virtualized list) with author pill, timestamp, and Markdown parsing.
  - [ ] Integrate initial fetch via `GET /channels/{channel_id}/events` (limit/ pagination support).
  - [ ] Provide placeholder for reactions and system events.

## Week 5: Messaging UX & Realtime (Milestone F1)

- [ ] Message composition + delivery.
  - [ ] Build rich composer (multi-line, emoji picker stub, upload button disabled until media API lands).
  - [ ] Emit messages via `POST /channels/{channel_id}/messages`; show optimistic updates with rollback on failure.
  - [ ] Surface validation feedback from backend rejection reasons (length, sender mismatch, rate limits).
- [ ] WebSocket integration.
  - [ ] Implement WS client with exponential backoff, heartbeat, and visibility-based pause/resume.
  - [ ] Merge incoming events into Pinia stores and append to virtualized timeline without duplicate entries.
  - [ ] Emit typing indicator previews (placeholder API until backend support arrives).
- [ ] Offline & error handling.
  - [ ] Detect network issues, queue unsent messages, and display retry banners.
  - [ ] Add Sentry breadcrumbs for API/WS failures with correlation IDs.
  - [ ] Update Lighthouse/Performance budgets after enabling live data.

## Week 6-7: Security, Reliability & Accessibility (Milestone F1 to F2 bridge)

- [ ] Token lifecycle & device management.
  - [x] Implement silent refresh flows using backend `/sessions/refresh`; prompt logout when refresh fails.
  - [ ] Expose device list UI (read-only) consuming future `/sessions/devices` endpoint (stub in mocks until backend ships).
  - [ ] Add secure storage audit (localStorage vs IndexedDB) with fallback for SSR.
- [ ] Permission-aware UX.
  - [ ] Gate actions (send message, create channel) based on role bits from guild state.
  - [ ] Show permission errors inline with actionable guidance.
  - [ ] Build admin-only panels hidden behind feature flag + role check.
- [ ] Accessibility + QA.
  - [ ] Run axe-core audits on core pages; fix high/critical issues.
  - [ ] Ensure keyboard navigation across channel list, timeline, and composer.
  - [ ] Update `docs/TESTING.md` with manual regression plan (screen reader, keyboard-only, mobile viewport).

## Week 8: Federation Awareness (Milestone F2)

- [ ] Surface remote context.
  - [ ] Display origin server badges on messages fetched via `/federation/channels/{channel_id}/events`.
  - [ ] Provide trust indicators (warning banner) when guild includes remote homeservers.
  - [ ] Wire filtering UI for message origin (local vs remote).
- [ ] Federation settings UI.
  - [ ] Build guild settings page listing trusted servers (read-only, backend-provided).
  - [ ] Allow operators to copy event IDs + origin metadata for support tickets.
  - [ ] Log federation errors to Sentry with origin tags.
- [ ] QA handshake vectors.
  - [ ] Integrate `GET /mls/handshake-test-vectors` in developer tools modal so client engineers can verify signature checking.
  - [ ] Document verification steps in `docs/DEV_NOTES.md` (new section).

## Week 9: MLS & Device Prep (Milestone F2)

- [ ] Key package awareness.
  - [ ] Render available MLS key packages per identity with rotation timestamps (consumes `/mls/key-packages`).
  - [ ] Provide "copy to clipboard" actions with audit logging hooks.
  - [ ] Highlight when local device lacks an MLS key package (pre-flight checks).
- [ ] Device bootstrap flows.
  - [ ] Add modal guiding new device registration (placeholder until MLS enrolment endpoints land).
  - [ ] Store handshake verification results locally to avoid repeated prompts.
  - [ ] Capture telemetry on MLS readiness (feature flag gating UI).
- [ ] Security review.
  - [ ] Pair with backend to review MLS UX copy, trust indicators, and failure messaging.
  - [ ] Update threat model references (`docs/THREATMODEL.md`) with frontend attack considerations (phishing, token theft).

## Week 10+: Frontend Roadmap

- [ ] Evaluate component library extraction (design system package shared across marketing/admin).
- [ ] Implement rich reactions, message edits, and context menus.
- [ ] Add global search (placeholder until search API shipped).
- [ ] Mobile app shell (Capacitor/React Native evaluation) leveraging existing stores.
- [ ] Telemetry dashboards (PostHog or Amplitude) for engagement metrics.

## Ongoing Backlog (Parallel Streams)

- [ ] Storybook visual regression coverage (Chromatic/GitHub action).
- [ ] Add Playwright E2E suite for smoke tests (login, send message, channel create).
- [ ] Localization groundwork (i18n routing, translation catalogs, fallback strings).
- [ ] Performance budgets (bundle analysis, code splitting, prefetch heuristics).
- [ ] Moderation tooling (report message modal, admin dashboards).
- [ ] Document frontend-only feature flags and runtime configuration matrix.

Keep this document evolving—sync with the backend timeline during weekly check-ins, annotate blockers, and link to issues/designs to preserve context.
