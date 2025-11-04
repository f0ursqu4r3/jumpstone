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

describe('channel create flow', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('posts to the backend and updates the store', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === 'string' ? input : input.toString()
      if (url.endsWith('/guilds/guild-1/channels') && init?.method === 'POST') {
        return createJsonResponse({
          channel_id: 'channel-2',
          guild_id: 'guild-1',
          name: 'updates',
          created_at: '2025-11-03T16:00:00Z',
        })
      }

      if (url.endsWith('/guilds/guild-1/channels')) {
        return createJsonResponse([
          {
            channel_id: 'channel-1',
            guild_id: 'guild-1',
            name: 'general',
            created_at: '2025-11-03T15:00:00Z',
          },
        ])
      }

      return createJsonResponse([])
    })

    vi.stubGlobal('fetch', fetchMock)

    const store = useChannelStore()
    await store.setActiveGuild('guild-1')
    const record = await store.createChannel('guild-1', 'updates')

    expect(record).toMatchObject({ channel_id: 'channel-2', name: 'updates' })
    expect(store.channelsByGuild['guild-1']).toHaveLength(2)
  })
})
