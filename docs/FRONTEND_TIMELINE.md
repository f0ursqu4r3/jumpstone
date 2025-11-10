# OpenGuild Frontend Delivery Timeline

This companion roadmap focuses on the Vue 3 web client. It mirrors the backend cadence so both sides converge on the same milestones. Update statuses, owners, links to design artifacts, and QA notes as work lands. Where backend support is required, call it out inline so dependencies stay visible.

## Working Assumptions

- [x] Vue 3 + Pinia + TypeScript on Vite (post-Nuxt migration) remain the primary stack; Tailwind powers the design system.
- [x] API traffic flows through the backend documented in `docs/API.md`; avoid client-side schema drift by importing shared types when feasible.
- [x] Treat accessibility and responsive layouts as first-class (WCAG AA target).
- [x] Ship instrumentation alongside features (Sentry + LogRocket stubs, Lighthouse budgets, vitest coverage).

## Week 1-2: Client Foundation (Milestone F0)

- [x] Stabilize developer workflow.
  - [x] Document Bun/NPM parity commands (`dev`, `lint`, `test`, `build`) and CI hooks. `frontend/README.md` now outlines command parity plus CI expectations (`lint`, `test`, `build`, `preview`) and references the generated types alias.
  - [x] Add `.env.example` with API base URLs, feature flags, and mock toggles. `frontend/.env.example` ships defaults for API base URL, mock toggles, and devtools awareness.
  - [x] Wire Vite aliases to shared TypeScript types generated from the backend OpenAPI schema (placeholder script). `frontend/vue.config.ts`, `frontend/tsconfig.json`, and `frontend/vitest.config.ts` expose `@openguild/backend-types`; `pnpm types:sync` seeds `frontend/types/generated`.
- [x] Establish design system + layout shell.
  - [x] Ship Tailwind tokens (color, spacing, typography) mapped to the brand palette. `frontend/src/assets/css/tokens.css`, `frontend/src/assets/css/main.css`, and `frontend/tailwind.config.ts` define brand colors, spacing, and UI defaults.
  - [x] Build the global app frame (navigation column, content area, status bar) with responsive breakpoints via `frontend/src/layouts/DefaultLayout.vue` plus the companion `frontend/src/components/app/AppGuildRail.vue`, `AppChannelSidebar.vue`, and `AppTopbar.vue`.
- [x] Scaffold state management and API client.
  - [x] Create an Axios/fetch wrapper with typed responses, request ID propagation, and retry/backoff policy. The Vite plugin surface now lives at `frontend/src/composables/useApiClient.ts` with `frontend/src/config/runtime.ts` feeding env overrides.
  - [x] Introduce Pinia stores for session, guilds, and channels with hydration helpers. `frontend/src/stores/guilds.ts`, `frontend/src/stores/channels.ts`, and `frontend/src/stores/session.ts` replace inline mocks and feed `frontend/src/layouts/DefaultLayout.vue`.
  - [x] Add vitest + Testing Library setup; cover store mutations and HTTP client error handling. Suites now cover Pinia stores and API client behavior (`frontend/tests/*store.test.ts`, `frontend/tests/api-client.test.ts`).

## Week 3: Authentication & Landing Flows (Milestone F0)

- [x] Implement session UX.
  - [x] Build login/register forms with validation, error toasts, and loading states. `frontend/src/views/LoginView.vue` and the new `frontend/src/views/RegisterView.vue` share the session store while handling device metadata prompts.
  - [x] Persist access/refresh tokens in secure storage (IndexedDB fallback), respecting backend expiry semantics.
  - [x] Create device metadata prompts (friendly name) aligned with backend requirements.
- [x] Route guards and onboarding.
  - [x] Add global middleware that enforces auth, refreshes tokens, and redirects to `/login` when needed.
  - [x] Deliver onboarding carousel with links to docs and setup guides. `frontend/src/components/app/AppOnboardingCarousel.vue` powers the login/register hero with CTA links back to `docs/SETUP.md`, `docs/FRONTEND_TIMELINE.md`, and `docs/TESTING.md`.
  - [x] Smoke-test flows against backend `POST /sessions/login` + `/users/register`; record QA steps in `docs/TESTING.md`. Vitest coverage now lives in `frontend/src/stores/__tests__/session.spec.ts`, and the testing guide documents the `bun test:unit` command (storybook suite remains opt-in via `STORYBOOK_TESTS=true`).

## Week 4: Guild & Channel Shell (Milestone F1)

- [x] Guild discovery and selection.
  - [x] Render the guild switcher with avatars, tooltips, and unread indicators (stub data + API integration). `frontend/src/components/app/AppGuildRail.vue` now consumes the hydrated Pinia store and shows skeletons while `/guilds` loads.
  - [x] Implement guild creation modal consuming backend `/guilds` POST. `frontend/src/components/app/AppGuildCreateModal.vue` surfaces a Nuxt UI modal triggered from the rail, wiring straight into `useGuildStore.createGuild` and rehydrating channels on success.
  - [x] Handle empty states (no guilds, invite-only messaging). `frontend/src/layouts/DefaultLayout.vue` shows a card prompting guild creation, while `HomeView` raises an invite-only alert and `AppChannelSidebar.vue` provides empty placeholders.
- [x] Channel list + metadata.
  - [x] Display channel tree (text/voice), sort order, and locks based on permissions. `frontend/src/components/app/AppChannelSidebar.vue` now groups text/voice channels, highlights unread counts, and reacts to `useChannelStore` hydration.
  - [x] Surface channel topic/description and breadcrumb within the content header. `frontend/src/layouts/DefaultLayout.vue` wires channel descriptions into `AppTopbar` and sidebar metadata.
  - [x] Support skeleton/loading states using Suspense for SSR hydration. The sidebar shows loading placeholders while `/guilds/{guild_id}/channels` resolves.
  - [x] Implement channel creation modal. `frontend/src/components/app/AppChannelCreateModal.vue` posts to `/guilds/{id}/channels`, updates the store, and surfaces placeholders for empty/invite-only states.
- [x] Timeline scaffold.
  - [x] Render message timeline (virtualized list) with author pill, timestamp, and Markdown parsing. `frontend/src/components/app/AppMessageTimeline.vue` renders grouped events with sender badges and timestamps (virtualization to follow in Week 5).
  - [x] Integrate initial fetch via `GET /channels/{channel_id}/events` (limit/ pagination support).
  - [x] Provide placeholder for reactions and system events. `frontend/src/components/app/AppMessageTimeline.vue` now renders badges for non-message events and a reactions stub ahead of Week 5 real-time UX.

## Week 5: Messaging UX & Realtime (Milestone F1)

- [x] Message composition + delivery.
  - [x] Build rich composer (multi-line, emoji picker stub, upload button disabled until media API lands). `frontend/src/components/app/AppMessageComposer.vue` now auto-sizes, exposes emoji/upload stubs, and surfaces queue status inline.
  - [x] Emit messages via `POST /channels/{channel_id}/messages`; show optimistic updates with rollback on failure. `frontend/src/stores/messages.ts` orchestrates optimistic inserts with Pinia, clears state on success, and falls back when the backend rejects the payload.
  - [x] Surface validation feedback from backend rejection reasons (length, sender mismatch, rate limits). The message composer store bubbles backend messages into the composer alert and caps content to 4k characters.
- [x] WebSocket integration.
  - [x] Implement WS client with exponential backoff, heartbeat, and visibility-based pause/resume. `frontend/src/stores/realtime.ts` now handles reconnect backoff, heartbeats, and visibility-driven pauses while updating connectivity state.
  - [x] Merge incoming events into Pinia stores and append to virtualized timeline without duplicate entries. Real-time envelopes route through `useTimelineStore.insertEvent` so optimistic copies reconcile when the server ACKs.
  - [x] Emit typing indicator previews (placeholder API until backend support arrives). `useRealtimeStore.sendTypingPreview` pushes throttled previews over the socket (with HTTP fallback) and `HomeView` wires composer typing events into the store.
- [x] Offline & error handling.
  - [x] Detect network issues, queue unsent messages, and display retry banners. The message composer store tracks pending/failed messages, sets connectivity degradations, and the composer surfaces queue counts + retry controls.
  - [x] Add Sentry breadcrumbs for API/WS failures with correlation IDs. API sends now log request IDs from `useMessageComposerStore`, while the realtime store records websocket lifecycle breadcrumbs for Sentry.
  - [x] Update Lighthouse/Performance budgets after enabling live data. `frontend/lighthouse.budgets.json` captures the new budgets so CI audits account for websocket bootstrap requests.

## Week 6-7: Security, Reliability & Accessibility (Milestone F1 to F2 bridge)

- [x] Token lifecycle & device management.
  - [x] Implement silent refresh flows using backend `/sessions/refresh`; prompt logout when refresh fails.
  - [x] Expose device list UI (read-only) consuming future `/sessions/devices` endpoint (stub in mocks until backend ships). `frontend/src/stores/devices.ts` hydrates fallback device metadata until the API lands, and `HomeView.vue` renders the list with loading/error states.
  - [x] Add secure storage audit (localStorage vs IndexedDB) with fallback for SSR. `frontend/src/utils/storage.ts` analyses storage capabilities, and `frontend/src/stores/session.ts` publishes `storageAudit` so the session overview shows when we rely on in-memory persistence.
- [x] Permission-aware UX.
  - [x] Gate actions (send message, create channel) based on role bits from guild state. `frontend/src/utils/permissions.ts` normalises roles, `DefaultLayout.vue` disables channel creation when rights are missing, and `HomeView.vue` blocks the composer for read-only members.
  - [x] Show permission errors inline with actionable guidance. The composer surfaces a “Messaging restricted” alert, while `AppChannelSidebar.vue` prints role-based guidance beneath the disabled CTA.
  - [x] Build admin-only panels hidden behind feature flag + role check. Setting `VITE_FEATURE_ADMIN_PANEL=true` reveals the preview panel in `HomeView.vue` for guild admins/platform maintainers only.
- [x] Accessibility + QA.
  - [x] Run axe-core audits on core pages; fix high/critical issues. `frontend/tests/accessibility.test.ts` runs `axe-core` against the timeline component as part of `bun test:unit`.
  - [x] Ensure keyboard navigation across channel list, timeline, and composer. `AppMessageTimeline.vue` now exposes list semantics and focusable items, pairing with tooltip guidance for disabled channel creation.
  - [x] Update `docs/TESTING.md` with manual regression plan (screen reader, keyboard-only, mobile viewport).

## Week 8: Federation Awareness (Milestone F2)

- [x] Surface remote context.
  - [x] Display origin server badges on messages fetched via `/federation/channels/{channel_id}/events`. `frontend/src/components/app/AppMessageTimeline.vue` now flags remote events, badges their origin host, and exposes a copy-to-clipboard action.
  - [x] Provide trust indicators (warning banner) when guild includes remote homeservers. `HomeView.vue` consumes `useFederationStore` to show alerts listing remote hosts and guidance for mitigations.
  - [x] Wire filtering UI for message origin (local vs remote). The timeline section includes a Nuxt UI radio group that filters `AppMessageTimeline` to all/local/remote events on demand.
- [x] Federation settings UI.
  - [x] Build guild settings page listing trusted servers (read-only, backend-provided). The new federation card in `HomeView.vue` enumerates remote servers per guild and exposes copy actions.
  - [x] Allow operators to copy event IDs + origin metadata for support tickets. Timeline rows now provide a `Copy meta` button that captures the event ID + origin server.
  - [x] Log federation errors to Sentry with origin tags. `frontend/src/stores/federation.ts` recycles `recordNetworkBreadcrumb` whenever context/handshake fetches fail or fall back to mocks.
- [x] QA handshake vectors.
  - [x] Integrate `GET /mls/handshake-test-vectors` in developer tools modal so client engineers can verify signature checking. `HomeView.vue` exposes a “Refresh vectors” button (admin section) that hydrates and previews handshake payloads.
  - [x] Document verification steps in `docs/TESTING.md`. The manual testing section now outlines the federation handshake exercise plus the accessibility plan from Week 7.

## Week 9: MLS & Device Prep (Milestone F2)

- [x] Key package awareness.
  - [x] Render available MLS key packages per identity with rotation timestamps (consumes `/mls/key-packages`). The new Pinia store at `frontend/src/stores/mls.ts` hydrates `HomeView.vue`, badges rotation info, and captures last-fetched metadata.
  - [x] Provide "copy to clipboard" actions with audit logging hooks. Copy buttons in the MLS readiness card emit `recordBreadcrumb` entries so SREs can trace who exported signature/HPKE material.
  - [x] Highlight when local device lacks an MLS key package (pre-flight checks). `HomeView.vue` computes identity candidates (device ID, identifier, username) and raises an alert when none of the fetched packages match.
- [x] Device bootstrap flows.
  - [x] Add modal guiding new device registration (placeholder until MLS enrolment endpoints land). `frontend/src/components/app/AppDeviceBootstrapModal.vue` walks operators through naming the device, running the CLI stub, and refreshing handshakes.
  - [x] Store handshake verification results locally to avoid repeated prompts. `frontend/src/stores/federation.ts` now persists `handshakeVerifiedAt` in `localStorage`, and the UI surfaces a badge/alert when the TTL expires.
  - [x] Capture telemetry on MLS readiness (feature flag gating UI). `VITE_FEATURE_MLS_READINESS` toggles the new dashboard bits while copy actions log breadcrumbs tagged `mls.*`.
- [x] Security review.
  - [x] Pair with backend to review MLS UX copy, trust indicators, and failure messaging. The federation card now highlights stale handshakes and ties remote server badges to actionable CTAs.
  - [x] Update threat model references (`docs/THREATMODEL.md`) with frontend attack considerations (phishing, token theft).

## Week 10+: Frontend Roadmap

- [x] Evaluate component library extraction (design system package shared across marketing/admin).
  - [x] Documented the extraction strategy in `docs/UI_SYSTEM.md` and introduced `frontend/src/components/primitives/GuildSurfaceCard.vue` plus an index barrel so future packages can tree-shake shared primitives. `HomeView.vue` now consumes the new primitive as a showcase.
- [ ] Implement rich reactions, message edits, and context menus.
  - [x] Rich reactions — `frontend/src/stores/reactions.ts` merges server payloads with optimistic overrides (handling 404/501 fallbacks) while `AppMessageTimeline.vue` renders reaction chips, palette popovers, and toggles hooked into the store.
  - [x] Inline message edits + context menu — `AppMessageTimeline.vue` now exposes an action menu (authors only) that opens an inline editor. Saves call `PATCH /channels/{channel_id}/events/{event_id}` when available, otherwise fall back to local updates via `useTimelineStore.updateEventContent`.
  - [x] Moderation placeholders — The same menu includes a “Report message” action (stubbed to console logging for now) so future moderation workflows have a visible entry point.
  - [x] Message edits with inline composer + diff preview. Editing surfaces now include a live diff panel (“Original” vs “Revised preview”) so reviewers can verify changes before saving.
  - [ ] Context menu affordances for moderation actions.
- [x] Add global search (placeholder until search API shipped).
  - [x] `AppGlobalSearchModal.vue` plus `useSearchStore` now hydrate queries via `/search/messages` when available, falling back to mock results (with telemetry breadcrumbs) so the UX is testable before the backend lands.
  - [x] `HomeView.vue` exposes a hero-level “Search” CTA that opens the modal, tracks last queries, and surfaces empty states + result metadata.
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
