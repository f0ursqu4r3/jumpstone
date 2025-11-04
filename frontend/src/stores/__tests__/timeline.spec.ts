import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { useTimelineStore } from '../timeline'

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    ...init,
  })

describe('timeline store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('loads channel events and appends refreshes', async () => {
    const firstPayload = [
      {
        sequence: 1,
        channel_id: 'channel-1',
        event: {
          schema_version: 1,
          event_id: '$event1',
          event_type: 'message',
          room_id: 'channel-1',
          sender: 'user-1',
          origin_server: 'dev.openguild.local',
          origin_ts: 1_725_000_001_000,
          content: { content: 'hello world' },
        },
      },
    ]

    const secondPayload = [
      {
        sequence: 2,
        channel_id: 'channel-1',
        event: {
          schema_version: 1,
          event_id: '$event2',
          event_type: 'message',
          room_id: 'channel-1',
          sender: 'user-1',
          origin_server: 'dev.openguild.local',
          origin_ts: 1_725_000_002_000,
          content: { content: 'second message' },
        },
      },
    ]

    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const raw = typeof input === 'string' ? input : input.toString()
      const url = new URL(raw)

      if (url.pathname.endsWith('/channels/channel-1/events')) {
        if (url.searchParams.get('since') === '1') {
          return createJsonResponse(secondPayload)
        }
        return createJsonResponse(firstPayload)
      }

      return createJsonResponse([])
    })

    vi.stubGlobal('fetch', fetchMock)

    const store = useTimelineStore()
    await store.loadChannel('channel-1', { force: true })
    expect(store.eventsByChannel['channel-1']).toHaveLength(1)

    await store.loadChannel('channel-1', { refresh: true })
    expect(store.eventsByChannel['channel-1']).toHaveLength(2)
    expect(store.eventsByChannel['channel-1']?.[1]?.event.content).toMatchObject({
      content: 'second message',
    })
  })
})
