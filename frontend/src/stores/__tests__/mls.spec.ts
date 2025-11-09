import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { useMlsStore } from '../mls'

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    ...init,
  })

describe('mls store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('fetches key packages and records last refresh time', async () => {
    const payload = [
      {
        identity: 'device-1',
        ciphersuite: 'MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519',
        signature_key: 'sig',
        hpke_public_key: 'hpke',
      },
    ]

    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        return createJsonResponse(payload)
      }),
    )

    const store = useMlsStore()
    await store.fetchKeyPackages()

    expect(store.keyPackages).toHaveLength(1)
    expect(store.keyPackages[0]?.identity).toBe('device-1')
    expect(store.lastFetchedAt).not.toBeNull()
    expect(store.error).toBeNull()
  })
})
