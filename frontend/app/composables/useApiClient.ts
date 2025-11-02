import { useSessionStore } from '~/stores/session';

const normalizeHeaders = (headers?: HeadersInit): Record<string, string> => {
  if (!headers) {
    return {};
  }

  if (headers instanceof Headers) {
    return Object.fromEntries(headers.entries());
  }

  if (Array.isArray(headers)) {
    return Object.fromEntries(headers);
  }

  return { ...headers };
};

const createRequestId = () => {
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.randomUUID === 'function'
  ) {
    return crypto.randomUUID();
  }

  return Math.random().toString(16).slice(2);
};

export const useApiClient = () => {
  const runtimeConfig = useRuntimeConfig();
  const baseURL = runtimeConfig.public.apiBaseUrl;

  const client = $fetch.create({
    baseURL,
    retry: 0,
    async onRequest({ options }) {
      const headers = normalizeHeaders(options.headers);
      const session = useSessionStore();

      if (session.isAuthenticated) {
        await session.ensureFreshAccessToken();
      }

      const token = session.accessToken;

      if (token && !headers.authorization) {
        headers.authorization = `Bearer ${token}`;
      }

      if (session.deviceId && !headers['x-device-id']) {
        headers['x-device-id'] = session.deviceId;
      }

      if (!headers['x-request-id']) {
        headers['x-request-id'] = createRequestId();
      }

      if (!headers.accept) {
        headers.accept = 'application/json';
      }

      options.headers = new Headers(headers);
    },
    onResponseError({ response }) {
      console.error('API request failed', {
        status: response.status,
        statusText: response.statusText,
        body: response._data,
      });
    },
  });

  return client;
};
