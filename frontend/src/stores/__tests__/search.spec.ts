import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { useSearchStore } from '../search'

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    ...init,
  })

describe('search store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('falls back to mock results when search API disabled', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () =>
        createJsonResponse(
          {
            error: 'Not implemented',
          },
          { status: 501 },
        ),
      ),
    )

    const store = useSearchStore()
    await store.performSearch('test query')

    expect(store.results.length).toBeGreaterThan(0)
    expect(store.error).toBeNull()
  })

  it('normalizes API payload', async () => {
    const payload = [
      {
        id: 'msg-1',
        type: 'message',
        title: 'Test',
        subtitle: '#general',
        snippet: 'body',
      },
    ]

    vi.stubGlobal(
      'fetch',
      vi.fn(async () => createJsonResponse(payload)),
    )

    const store = useSearchStore()
    await store.performSearch('test')

    expect(store.results).toHaveLength(1)
    expect(store.results[0]?.title).toBe('Test')
    expect(store.error).toBeNull()
  })
})
