import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const currentDir = dirname(fileURLToPath(import.meta.url));
const backendTypesDir = resolve(currentDir, 'types/generated');

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-07-15',
  devtools: { enabled: true },
  modules: ['@nuxt/eslint', '@nuxt/ui', '@pinia/nuxt'],
  css: ['~/assets/css/main.css'],
  alias: {
    '@openguild/backend-types': backendTypesDir,
  },
  vite: {
    resolve: {
      alias: {
        '@openguild/backend-types': backendTypesDir,
      },
    },
  },
  runtimeConfig: {
    public: {
      apiBaseUrl:
        process.env.NUXT_PUBLIC_API_BASE_URL ?? 'http://127.0.0.1:8080',
    },
  },
});
