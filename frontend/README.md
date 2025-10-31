# OpenGuild Frontend

Nuxt 3 web application for the OpenGuild federated chat platform. The app leans on Pinia for state management and Tailwind for the design system.

## Prerequisites

- Node.js 20.x (or Bun 1.1+)
- Package manager of choice: pnpm 9.x, npm 10.x, or Bun 1.1+

Install dependencies with your preferred runtime:

```bash
pnpm install        # or: npm install / bun install
```

## Environment

Copy `.env.example` to `.env` (or `.env.local`) and adjust values for your environment:

```bash
cp .env.example .env
```

Nuxt exposes any keys prefixed with `NUXT_PUBLIC_` to the client; keep secrets on the backend.

## Common Commands

| Goal        | pnpm              | npm                  | bun                    | Notes                            |
| ----------- | ----------------- | -------------------- | ---------------------- | -------------------------------- |
| Develop     | `pnpm dev`        | `npm run dev`        | `bun run dev`          | Starts Nuxt with hot reload      |
| Lint        | `pnpm lint`       | `npm run lint`       | `bun run lint`         | ESLint over the entire repo      |
| Test*       | `pnpm test`       | `npm test`           | `bun test`             | Vitest + Vue Testing Library     |
| Build       | `pnpm build`      | `npm run build`      | `bun run build`        | Production build (SSR ready)     |
| Preview     | `pnpm preview`    | `npm run preview`    | `bun run preview`      | Serves the built app locally     |

\*`test` wiring lands alongside the Vitest setup (tracked in Week 1-2 milestone).

CI mirrors the `lint`, `test`, and `build` steps; keep feature branches passing these commands before opening a PR.

## Project Structure

- `app/` & `pages/`: Nuxt routing and top-level layout shells.
- `components/`: Shared UI components (design system work begins in Week 1-2).
- `stores/`: Pinia stores for session, guild, and channel state.
- `assets/css/`: Tailwind entry point and design tokens.

The development server runs on `http://localhost:3000` by default.
