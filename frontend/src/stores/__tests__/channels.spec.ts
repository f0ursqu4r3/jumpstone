import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { useChannelStore } from '../channels'

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    ...init,
  })

describe('channel store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('fetches channels for the active guild', async () => {
    const responses: Record<string, Response> = {
      '/guilds': createJsonResponse([]),
      '/guilds/guild-1/channels': createJsonResponse([
        {
          channel_id: 'channel-1',
          guild_id: 'guild-1',
          name: 'general',
          created_at: '2025-01-01T00:00:00Z',
        },
        {
          channel_id: 'channel-2',
          guild_id: 'guild-1',
          name: 'voice-sync',
          created_at: '2025-01-01T00:05:00Z',
        },
      ]),
    }

    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === 'string' ? input : input.toString()
      const key = url.replace('http://127.0.0.1:8080', '')
      return responses[key] ?? createJsonResponse([])
    })

    vi.stubGlobal('fetch', fetchMock)

    const store = useChannelStore()
    await store.setActiveGuild('guild-1')

    const channels = store.channelsByGuild['guild-1'] ?? []
    expect(channels).toHaveLength(2)
    expect(channels[0]).toMatchObject({ id: 'channel-1', kind: 'text' })
    expect(channels[1]).toMatchObject({ id: 'channel-2', kind: 'voice' })
    expect(store.activeChannelId).toBe('channel-1')
  })

  it('creates a channel and updates local cache', async () => {
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
        const url = typeof input === 'string' ? input : input.toString()
        if (url.endsWith('/guilds/guild-1/channels') && init?.method === 'POST') {
          return createJsonResponse({
            channel_id: 'channel-3',
            guild_id: 'guild-1',
            name: 'alerts',
            created_at: '2025-01-02T00:00:00Z',
          })
        }

        if (url.endsWith('/guilds/guild-1/channels')) {
          return createJsonResponse([
            {
              channel_id: 'channel-1',
              guild_id: 'guild-1',
              name: 'general',
              created_at: '2025-01-01T00:00:00Z',
            },
          ])
        }

        return createJsonResponse([])
      },
    )

    vi.stubGlobal('fetch', fetchMock)

    const store = useChannelStore()
    await store.setActiveGuild('guild-1')
    await store.createChannel('guild-1', 'alerts')

    const channels = store.channelsByGuild['guild-1'] ?? []
    expect(channels.some((channel) => channel.label === 'alerts')).toBe(true)
  })
})
