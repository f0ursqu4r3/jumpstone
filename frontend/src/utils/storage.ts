interface StorageAdapter {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

export type StorageAuditType = 'memory' | 'local-storage' | 'indexeddb+local-storage'

export interface StorageAuditSnapshot {
  type: StorageAuditType
  available: boolean
  reason?: string
}

const createMemoryStorage = (): StorageAdapter => {
  const store = new Map<string, string>()
  return {
    getItem: (key) => store.get(key) ?? null,
    setItem: (key, value) => {
      store.set(key, value)
    },
    removeItem: (key) => {
      store.delete(key)
    },
  }
}

interface StorageResolution {
  adapter: StorageAdapter
  snapshot: StorageAuditSnapshot
}

const resolveStorage = (): StorageResolution => {
  if (typeof window === 'undefined') {
    return {
      adapter: createMemoryStorage(),
      snapshot: {
        type: 'memory',
        available: false,
        reason: 'SSR fallback (window unavailable)',
      },
    }
  }

  const testKey = '__openguild_storage_audit__'
  let localAvailable = false
  let failureReason: string | undefined

  try {
    window.localStorage.setItem(testKey, '1')
    window.localStorage.removeItem(testKey)
    localAvailable = true
  } catch (err) {
    if (err instanceof Error) {
      failureReason = err.message
    } else {
      failureReason = 'Unable to access localStorage'
    }
  }

  const hasIndexedDb = typeof window.indexedDB !== 'undefined' && window.indexedDB !== null

  if (localAvailable) {
    return {
      adapter: window.localStorage,
      snapshot: {
        type: hasIndexedDb ? 'indexeddb+local-storage' : 'local-storage',
        available: true,
      },
    }
  }

  return {
    adapter: createMemoryStorage(),
    snapshot: {
      type: 'memory',
      available: false,
      reason: failureReason ?? 'localStorage unavailable (likely disabled or sandboxed)',
    },
  }
}

let cachedResolution: StorageResolution | null = null

export const getStorageResolution = (): StorageResolution => {
  if (!cachedResolution) {
    cachedResolution = resolveStorage()
  }
  return cachedResolution
}
