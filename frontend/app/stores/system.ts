import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

import type {
  ComponentStatus,
  ReadinessResponse,
  VersionResponse,
} from '~/types/api';
import { extractErrorMessage } from '~/utils/errors';

export const useSystemStore = defineStore('system', () => {
  const readiness = ref<ReadinessResponse | null>(null);
  const version = ref<string | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const lastFetchedAt = ref<number | null>(null);

  const status = computed(() => readiness.value?.status ?? 'unknown');
  const components = computed((): ComponentStatus[] => {
    return readiness.value?.components ?? [];
  });
  const uptimeSeconds = computed(() => readiness.value?.uptime_seconds ?? null);
  const hasError = computed(() => Boolean(error.value));

  async function fetchBackendStatus(force = false) {
    if (loading.value) {
      return;
    }

    if (
      !force &&
      lastFetchedAt.value &&
      Date.now() - lastFetchedAt.value < 15_000
    ) {
      return;
    }

    const nuxtApp = useNuxtApp();
    const api = nuxtApp.$api;

    loading.value = true;
    error.value = null;

    try {
      const [readinessResponse, versionResponse] = await Promise.all([
        api<ReadinessResponse>('/ready'),
        api<VersionResponse>('/version'),
      ]);

      readiness.value = readinessResponse;
      version.value = versionResponse.version;
      lastFetchedAt.value = Date.now();
    } catch (err) {
      error.value = extractErrorMessage(err);
    } finally {
      loading.value = false;
    }
  }

  return {
    // state
    readiness,
    version,
    loading,
    error,
    lastFetchedAt,

    // getters
    status,
    components,
    uptimeSeconds,
    hasError,

    // actions
    fetchBackendStatus,
  };
});
