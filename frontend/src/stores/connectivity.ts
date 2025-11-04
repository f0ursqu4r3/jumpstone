import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

let listenersRegistered = false

export const useConnectivityStore = defineStore('connectivity', () => {
  const initialOnline =
    typeof navigator === 'undefined' || typeof navigator.onLine !== 'boolean'
      ? true
      : navigator.onLine

  const online = ref(initialOnline)
  const degradedMessage = ref<string | null>(null)
  const lastOfflineAt = ref<number | null>(initialOnline ? null : Date.now())
  const lastOnlineAt = ref<number | null>(initialOnline ? Date.now() : null)
  const visibility = ref<string>(
    typeof document === 'undefined' || !document.visibilityState
      ? 'visible'
      : document.visibilityState,
  )

  const isDegraded = computed(() => !online.value || Boolean(degradedMessage.value))
  const status = computed<'online' | 'offline' | 'degraded'>(() => {
    if (!online.value) {
      return 'offline'
    }
    if (degradedMessage.value) {
      return 'degraded'
    }
    return 'online'
  })

  const lastChangeAt = computed(() => {
    if (!online.value) {
      return lastOfflineAt.value
    }
    return lastOnlineAt.value
  })

  const setVisibility = (state: DocumentVisibilityState | string) => {
    visibility.value = state
  }

  const updateOnline = (nextOnline: boolean) => {
    if (online.value === nextOnline) {
      return
    }

    online.value = nextOnline

    if (nextOnline) {
      degradedMessage.value = null
      lastOnlineAt.value = Date.now()
    } else {
      lastOfflineAt.value = Date.now()
    }
  }

  const markDegraded = (message: string | null) => {
    degradedMessage.value = message
  }

  if (typeof window !== 'undefined' && !listenersRegistered) {
    const handleOnline = () => updateOnline(true)
    const handleOffline = () => updateOnline(false)
    window.addEventListener('online', handleOnline, { passive: true })
    window.addEventListener('offline', handleOffline, { passive: true })

    if (typeof document !== 'undefined') {
      const handleVisibility = () => setVisibility(document.visibilityState)
      document.addEventListener('visibilitychange', handleVisibility, { passive: true })
    }

    listenersRegistered = true
  }

  return {
    online,
    degradedMessage,
    visibility,
    status,
    isDegraded,
    lastChangeAt,
    updateOnline,
    markDegraded,
    setVisibility,
  }
})
