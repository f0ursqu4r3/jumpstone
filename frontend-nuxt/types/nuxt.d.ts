import type { useApiClient } from '../app/composables/useApiClient';

declare module '#app' {
  interface NuxtApp {
    $api: ReturnType<typeof useApiClient>;
  }
}

declare module '@vue/runtime-core' {
  interface ComponentCustomProperties {
    $api: ReturnType<typeof useApiClient>;
  }
}
