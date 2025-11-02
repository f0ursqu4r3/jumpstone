# OpenGuild Nuxt Client

This package contains the Nuxt 3 frontend for OpenGuild. It ships with ESLint, Pinia stores, and a custom API client aligned with the Rust backend.

## Prerequisites

- Node.js 18+ (LTS recommended)
- Your preferred package manager (we use `pnpm` in CI, but `npm` and `bun` are supported)
- Copy environment defaults: `cp .env.example .env`

## Environment

Runtime configuration is read from `.env`, `.env.development`, or your shell. The defaults in `.env.example` include:

- `NUXT_PUBLIC_API_BASE_URL` – REST API origin (defaults to local backend)
- `NUXT_PUBLIC_ENABLE_MOCKS` – toggles mock data providers
- `NUXT_PUBLIC_ENABLE_DEVTOOLS` – allows enabling experimental UI

## Install dependencies

```bash
pnpm install
# or
npm install
# or
bun install
```

## Common commands

| Task           | pnpm             | npm                | bun                  |
| -------------- | ---------------- | ------------------ | -------------------- |
| Dev server     | `pnpm dev`       | `npm run dev`      | `bun run dev`        |
| Type-safe lint | `pnpm lint`      | `npm run lint`     | `bun run lint`       |
| Fix lint       | `pnpm lint:fix`  | `npm run lint -- --fix` | `bun run lint -- --fix` |
| Unit tests     | `pnpm test`      | `npm run test`     | `bun run test`       |
| Watch tests    | `pnpm test:watch`| `npm run test -- --watch` | `bun run test -- --watch` |
| Production build | `pnpm build`   | `npm run build`    | `bun run build`      |
| Preview build  | `pnpm preview`   | `npm run preview`  | `bun run preview`    |

## CI Hooks

- `pnpm lint` – enforced pre-merge to keep ESLint clean (`--max-warnings=0`)
- `pnpm test` – Vitest suite (jsdom environment) for stores/composables
- `pnpm build` – Nuxt production build (SSR + client bundles)

## Additional resources

- [Nuxt documentation](https://nuxt.com/docs/getting-started/introduction)
- Backend contracts live in `docs/API.md`; generated types sync via the `@openguild/backend-types` alias (see `nuxt.config.ts`)
