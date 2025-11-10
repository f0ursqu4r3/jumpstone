import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { useReactionStore, type ServerReaction } from '../reactions'

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    ...init,
  })

describe('reaction store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('merges overrides with server reactions', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => createJsonResponse({ ok: true })),
    )

    const store = useReactionStore()
    const server: ServerReaction[] = [
      { emoji: '👍', count: 2, reactors: ['alice'] },
      { emoji: '🔥', count: 1, reactors: [] },
    ]

    let merged = store.resolveReactions('channel-1', 'event-1', server, 'bob')
    expect(merged).toEqual([
      { emoji: '👍', count: 2, reacted: false },
      { emoji: '🔥', count: 1, reacted: false },
    ])

    await store.toggleReaction({
      channelId: 'channel-1',
      eventId: 'event-1',
      emoji: '👍',
      currentlyReacted: false,
    })

    merged = store.resolveReactions('channel-1', 'event-1', server, 'bob')
    expect(merged.find((entry) => entry.emoji === '👍')?.count).toBe(3)
    expect(merged.find((entry) => entry.emoji === '👍')?.reacted).toBe(true)
  })

  it('falls back when reactions API not implemented', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () =>
        createJsonResponse({ error: 'not implemented' }, { status: 501 }),
      ),
    )

    const store = useReactionStore()

    await expect(
      store.toggleReaction({
        channelId: 'channel-1',
        eventId: 'event-1',
        emoji: '🎉',
        currentlyReacted: false,
      }),
    ).resolves.not.toThrow()

    const merged = store.resolveReactions('channel-1', 'event-1', [], 'user-1')
    expect(merged[0]).toMatchObject({ emoji: '🎉', count: 1, reacted: true })
  })
})
