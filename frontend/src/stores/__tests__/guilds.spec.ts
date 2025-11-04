import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { useGuildStore } from '../guilds'

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    ...init,
  })

describe('guild store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('hydrates guilds from the backend', async () => {
    const payload = [
      {
        guild_id: 'guild-1',
        name: 'Frontend Guild',
        created_at: '2025-01-01T00:00:00Z',
      },
      {
        guild_id: 'guild-2',
        name: 'Infra Ops',
        created_at: '2025-01-02T00:00:00Z',
      },
    ]

    const fetchMock = vi.fn(async () => createJsonResponse(payload))
    vi.stubGlobal('fetch', fetchMock)

    const store = useGuildStore()
    await store.hydrate(true)

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(store.guilds).toHaveLength(2)
    expect(store.guilds[0]).toMatchObject({ id: 'guild-1', initials: 'FG' })
    expect(store.activeGuildId).toBe('guild-1')
  })

  it('creates a guild via POST', async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
        const url = typeof input === 'string' ? input : input.toString()
        if (url.endsWith('/guilds') && init?.method === 'POST') {
          return createJsonResponse({
            guild_id: 'guild-3',
            name: 'New Guild',
            created_at: '2025-01-03T00:00:00Z',
          })
        }

        return createJsonResponse([])
      },
    )

    vi.stubGlobal('fetch', fetchMock)

    const store = useGuildStore()
    await store.hydrate(true)
    await store.createGuild('New Guild')

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining('/guilds'),
      expect.objectContaining({ method: 'POST' }),
    )
    expect(store.guilds.some((guild) => guild.name === 'New Guild')).toBe(true)
  })
})
