import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = dirname(fileURLToPath(new URL('.', import.meta.url)));

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-10-07',
  devtools: { enabled: true },
  alias: {
    '#shared-types': resolve(projectRoot, 'types', 'generated'),
  },
  runtimeConfig: {
    public: {
      apiBaseUrl:
        process.env.NUXT_PUBLIC_API_BASE_URL ?? 'http://localhost:4000',
      mediaBaseUrl:
        process.env.NUXT_PUBLIC_MEDIA_BASE_URL ?? 'http://localhost:4001',
      enableMockApi: process.env.NUXT_PUBLIC_ENABLE_MOCK_API === 'true',
      enableRequestLogger:
        process.env.NUXT_PUBLIC_ENABLE_REQUEST_LOGGER === 'true',
      featureFlags: (process.env.NUXT_PUBLIC_FEATURE_FLAGS ?? '')
        .split(',')
        .map((flag) => flag.trim())
        .filter(Boolean),
    },
  },
  modules: ['@nuxtjs/tailwindcss', '@nuxthq/ui'],
  ui: {
    global: true,
    icons: ['heroicons'],
  },
  css: ['@/assets/css/tailwind.css'],
  app: {
    head: {
      title: 'OpenGuild',
      meta: [
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
        {
          name: 'description',
          content: 'Federated Discord-like client for OpenGuild',
        },
      ],
    },
  },
});
