import { defineStore } from 'pinia';
import type {
  ComponentStatus,
  ReadinessResponse,
  VersionResponse,
} from '~/types/api';

interface SystemState {
  readiness: ReadinessResponse | null;
  version: string | null;
  loading: boolean;
  error: string | null;
  lastFetchedAt: number | null;
}

const formatError = (err: unknown): string => {
  if (!err) {
    return '';
  }
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === 'string') {
    return err;
  }
  try {
    return JSON.stringify(err);
  } catch {
    return 'Unexpected error';
  }
};

export const useSystemStore = defineStore('system', {
  state: (): SystemState => ({
    readiness: null,
    version: null,
    loading: false,
    error: null,
    lastFetchedAt: null,
  }),

  getters: {
    status: (state) => state.readiness?.status ?? 'unknown',
    components: (state): ComponentStatus[] => state.readiness?.components ?? [],
    uptimeSeconds: (state) => state.readiness?.uptime_seconds ?? null,
    hasError: (state) => Boolean(state.error),
  },

  actions: {
    async fetchBackendStatus(force = false) {
      if (this.loading) {
        return;
      }

      if (
        !force &&
        this.lastFetchedAt &&
        Date.now() - this.lastFetchedAt < 15_000
      ) {
        return;
      }

      const nuxtApp = useNuxtApp();
      const api = nuxtApp.$api;

      this.loading = true;
      this.error = null;

      try {
        const [readiness, version] = await Promise.all([
          api<ReadinessResponse>('/ready'),
          api<VersionResponse>('/version'),
        ]);

        this.readiness = readiness;
        this.version = version.version;
        this.lastFetchedAt = Date.now();
      } catch (err) {
        this.error = formatError(err);
      } finally {
        this.loading = false;
      }
    },
  },
});
