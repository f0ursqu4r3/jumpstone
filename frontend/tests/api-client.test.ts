import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useApiClient } from '../app/composables/useApiClient';

const ensureFreshAccessToken = vi.fn();
const sessionState = {
  isAuthenticated: true,
  accessToken: 'test-token',
  deviceId: 'device-123',
  ensureFreshAccessToken,
};

vi.mock('~/stores/session', () => ({
  useSessionStore: () => sessionState,
}));

const originalFetch = globalThis.fetch;

describe('useApiClient', () => {
  beforeEach(() => {
    sessionState.isAuthenticated = true;
    sessionState.accessToken = 'test-token';
    sessionState.deviceId = 'device-123';
    ensureFreshAccessToken.mockClear();

    // Nuxt runtime config shim.
    (globalThis as any).useRuntimeConfig = () => ({
      public: {
        apiBaseUrl: 'https://api.test.local',
      },
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    if (originalFetch) {
      globalThis.fetch = originalFetch;
    } else {
      delete (globalThis as any).fetch;
    }
    delete (globalThis as any).useRuntimeConfig;
  });

  it('adds auth headers and ensures fresh tokens for authenticated sessions', async () => {
    const fetchSpy = vi.fn(async (_input, init) => {
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    });

    globalThis.fetch = fetchSpy as typeof fetch;

    const client = useApiClient();
    await client('/ping');

    expect(ensureFreshAccessToken).toHaveBeenCalledTimes(1);
    expect(fetchSpy).toHaveBeenCalled();

    const [, init] = fetchSpy.mock.calls[0];
    const headers = new Headers(init?.headers as HeadersInit);

    expect(headers.get('authorization')).toBe('Bearer test-token');
    expect(headers.get('x-device-id')).toBe('device-123');
    expect(headers.get('x-request-id')).toBeTruthy();
    expect(headers.get('accept')).toBe('application/json');
  });

  it('omits auth headers when the session is not authenticated', async () => {
    sessionState.isAuthenticated = false;
    sessionState.accessToken = '';
    sessionState.deviceId = null as unknown as string;

    const fetchSpy = vi.fn(async (_input, init) => {
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    });

    globalThis.fetch = fetchSpy as typeof fetch;

    const client = useApiClient();
    await client('/public');

    expect(ensureFreshAccessToken).not.toHaveBeenCalled();

    const [, init] = fetchSpy.mock.calls[0];
    const headers = new Headers(init?.headers as HeadersInit);

    expect(headers.get('authorization')).toBeNull();
    expect(headers.get('x-device-id')).toBeNull();
  });

  it('logs response errors when the backend returns a failure', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});

    const fetchSpy = vi.fn(async () => {
      return new Response(JSON.stringify({ error: 'boom' }), {
        status: 500,
        headers: { 'content-type': 'application/json' },
      });
    });

    globalThis.fetch = fetchSpy as typeof fetch;

    const client = useApiClient();

    await expect(client('/error')).rejects.toBeDefined();

    expect(consoleError).toHaveBeenCalledWith(
      'API request failed',
      expect.objectContaining({
        status: 500,
        statusText: expect.any(String),
        body: { error: 'boom' },
      })
    );
  });
});
