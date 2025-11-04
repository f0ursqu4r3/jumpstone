import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { useSessionStore } from '../session'

const futureIso = (offsetMs: number) =>
  new Date(Date.now() + offsetMs).toISOString()

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    ...init,
  })

describe('session store auth flows', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('registers a new user and hydrates the profile', async () => {
    const registerResponse = { user_id: 'a-user-id', username: 'guildmaster' }
    const accessExpiresAt = futureIso(60 * 60 * 1000)
    const refreshExpiresAt = futureIso(7 * 24 * 60 * 60 * 1000)

    const fetchSpy = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === 'string' ? input : input.toString()

      if (url.endsWith('/users/register')) {
        expect(init?.method).toBe('POST')
        const payload = JSON.parse(String(init?.body ?? '{}'))
        expect(payload).toEqual({
          username: 'guildmaster',
          password: 'Supersafe123',
        })
        return createJsonResponse(registerResponse, { status: 201 })
      }

      if (url.endsWith('/sessions/login')) {
        const payload = JSON.parse(String(init?.body ?? '{}'))
        expect(payload).toEqual({
          identifier: 'guildmaster',
          secret: 'Supersafe123',
          device: {
            device_id: 'browser-abc',
            device_name: 'Onboarding rig',
          },
        })
        return createJsonResponse({
          access_token: 'access-token',
          access_expires_at: accessExpiresAt,
          refresh_token: 'refresh-token',
          refresh_expires_at: refreshExpiresAt,
        })
      }

      if (url.endsWith('/client/v1/users/me')) {
        return createJsonResponse({
          user_id: 'a-user-id',
          username: 'guildmaster',
          display_name: 'Guildmaster',
        })
      }

      return new Response(null, { status: 404 })
    })

    vi.stubGlobal('fetch', fetchSpy)

    const store = useSessionStore()

    await store.register({
      username: 'guildmaster',
      password: 'Supersafe123',
      deviceId: 'browser-abc',
      deviceName: 'Onboarding rig',
    })

    expect(fetchSpy).toHaveBeenCalledTimes(3)
    expect(store.tokens?.accessToken).toBe('access-token')
    expect(store.tokens?.refreshToken).toBe('refresh-token')
    expect(store.isAuthenticated).toBe(true)
    expect(store.identifier).toBe('guildmaster')
    expect(store.profile?.displayName).toBe('Guildmaster')
  })

  it('surfaces register validation errors', async () => {
    const fetchSpy = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === 'string' ? input : input.toString()
      if (url.endsWith('/users/register')) {
        return createJsonResponse(
          {
            error: 'username_taken',
          },
          { status: 409 },
        )
      }

      return new Response(null, { status: 404 })
    })

    vi.stubGlobal('fetch', fetchSpy)

    const store = useSessionStore()

    await expect(
      store.register({
        username: 'guildmaster',
        password: 'Supersafe123',
        deviceId: 'browser-abc',
      }),
    ).rejects.toThrow('That username is already in use. Choose a different one.')

    expect(fetchSpy).toHaveBeenCalledTimes(1)
    expect(store.fieldErrors.username).toBe('This username is already in use.')
    expect(store.isAuthenticated).toBe(false)
  })

  it('handles invalid login credentials', async () => {
    const fetchSpy = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === 'string' ? input : input.toString()
      if (url.endsWith('/sessions/login')) {
        return createJsonResponse(
          { error: 'invalid_credentials' },
          { status: 401 },
        )
      }

      return new Response(null, { status: 404 })
    })

    vi.stubGlobal('fetch', fetchSpy)

    const store = useSessionStore()

    await expect(
      store.login({
        identifier: 'guildmaster',
        secret: 'bad-pass',
        deviceId: 'browser-abc',
      }),
    ).rejects.toThrow(
      'Invalid credentials. Check your identifier and secret, then try again.',
    )

    expect(fetchSpy).toHaveBeenCalledTimes(1)
    expect(store.error).toBe(
      'Invalid credentials. Check your identifier and secret, then try again.',
    )
    expect(store.tokens).toBeNull()
  })
})
