# OpenGuild UI System — Extraction Plan

This document captures the Week 10 evaluation work for extracting our shared component library so the marketing site, admin portal, and core client can reuse the same primitives.

## Goals

1. **Single source of truth** for tokens (colors, typography, spacing) and primitives (buttons, cards, alerts).
2. **Portable Vue 3/Nuxt UI preset** that teams can import without dragging the entire app shell.
3. **Theming hooks** so product surfaces can opt into light/dark/brand overrides.

## Evaluation Summary

| Option                                           | Pros                                                                             | Cons                                                                     | Decision        |
| ------------------------------------------------ | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | --------------- |
| Keep everything inside the app (`UButton`, etc.) | Zero extra tooling, mirrors Nuxt UI defaults.                                    | Marketing/admin surfaces must depend on the whole client; no versioning. | Rejected.       |
| Build from scratch (headless + Tailwind)         | Total control, framework agnostic.                                               | Large maintenance tail; duplicates Nuxt UI work.                         | Deferred.       |
| **Wrap Nuxt UI as a preset** (`@openguild/ui`)   | Leverages existing components, aligns with design tokens, supports tree-shaking. | Requires thin wrappers + docs.                                           | ✅ Move forward. |

## Extraction Strategy

1. **Create a `frontend/src/components/primitives` directory** where we wrap Nuxt UI components with brand defaults (spacing, icon slots, focus states).
2. **Export the primitives through `frontend/src/components/primitives/index.ts`** so future packages can tree-shake (`import { GuildButton } from '@openguild/ui'`).
3. **Author story-driven docs** (Storybook or Nuxt Content) to showcase the primitives and document props.
4. **Package plan:** once stable, move the primitives + tokens into `packages/ui/` and publish via npm (internal registry to start).

## Immediate Deliverables (Week 10)

- `frontend/src/components/primitives/GuildSurfaceCard.vue`: baseline card wrapper with consistent padding/typography.
- `frontend/src/components/primitives/index.ts`: exports `GuildSurfaceCard` plus re-exports of Nuxt UI utilities.
- `DashboardView.vue`: showcase usage by swapping one of the existing cards to use `GuildSurfaceCard`.

## Next Steps

- Extract remaining primitives (Button, Badge, Input) with brand-specific defaults.
- Author design tokens README clarifying how Tailwind + CSS variables map to primitives.
- Evaluate bundling strategy (Vite library mode vs Rollup) once the primitives list stabilizes.
- Sync with marketing/admin teams to ensure the extracted package meets their needs (SSG-friendly, themable).
