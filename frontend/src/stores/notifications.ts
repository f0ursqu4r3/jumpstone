import { defineStore, storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'

import { getRuntimeConfig } from '@/config/runtime'
import { useConnectivityStore } from '@/stores/connectivity'
import { useChannelStore } from '@/stores/channels'
import { useSessionStore } from '@/stores/session'
import type { ChannelRecord, NotificationEventEnvelope } from '@/types/messaging'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

type NotificationsStatus = 'idle' | 'connecting' | 'connected' | 'paused' | 'error'

const MAX_BACKOFF_MS = 30_000
const BASE_BACKOFF_MS = 500

const resolveBaseUrl = () => {
  const runtimeConfig = getRuntimeConfig()
  const configured = (runtimeConfig.public.apiBaseUrl || '/api').trim()
  if (/^https?:/i.test(configured)) {
    return configured
  }
  if (typeof window !== 'undefined' && window.location) {
    return new URL(configured, window.location.origin).toString()
  }
  const fallbackHost = 'http://127.0.0.1:8080'
  return new URL(configured, fallbackHost).toString()
}

const buildSocketUrl = (token: string) => {
  const base = new URL(resolveBaseUrl())
  const normalizedPath = base.pathname.replace(/\/$/, '')
  base.pathname = `${normalizedPath}/notifications/ws`
  base.protocol = base.protocol === 'https:' ? 'wss:' : 'ws:'
  base.searchParams.set('access_token', token)
  return base
}

const parsePayload = (data: string | ArrayBuffer | Blob): NotificationEventEnvelope | null => {
  if (typeof data !== 'string') {
    return null
  }
  try {
    const parsed = JSON.parse(data) as NotificationEventEnvelope
    if (parsed && typeof parsed.kind === 'string' && parsed.event) {
      return parsed
    }
  } catch {
    return null
  }
  return null
}

export const useNotificationStore = defineStore('notifications', () => {
  const connectivityStore = useConnectivityStore()
  const sessionStore = useSessionStore()
  const channelStore = useChannelStore()
  const { online } = storeToRefs(connectivityStore)

  const state = ref<{
    status: NotificationsStatus
    attempt: number
    lastError: string | null
  }>({
    status: 'idle',
    attempt: 0,
    lastError: null,
  })

  const latestSequenceByChannel = ref<Record<string, number>>({})
  const socketRef = ref<WebSocket | null>(null)
  const reconnectTimer = ref<ReturnType<typeof setTimeout> | null>(null)
  const shouldConnect = ref(false)
  const closeIntent = ref<'manual' | 'pause' | null>(null)

  const status = computed(() => state.value.status)

  const scheduleReconnect = () => {
    if (!shouldConnect.value || reconnectTimer.value) {
      return
    }
    const delay = Math.min(MAX_BACKOFF_MS, BASE_BACKOFF_MS * 2 ** state.value.attempt)
    reconnectTimer.value = setTimeout(() => {
      reconnectTimer.value = null
      if (shouldConnect.value) {
        openSocket()
      }
    }, delay)
  }

  const clearReconnectTimer = () => {
    if (reconnectTimer.value) {
      clearTimeout(reconnectTimer.value)
      reconnectTimer.value = null
    }
  }

  const updateState = (next: Partial<(typeof state)['value']>) => {
    state.value = {
      ...state.value,
      ...next,
    }
  }

  const recordChannelMessage = (event: NotificationEventEnvelope) => {
    if (!event.channel_id || typeof event.sequence !== 'number') {
      return
    }
    const current = latestSequenceByChannel.value[event.channel_id] ?? 0
    if (event.sequence <= current) {
      return
    }
    latestSequenceByChannel.value = {
      ...latestSequenceByChannel.value,
      [event.channel_id]: event.sequence,
    }
    channelStore.updateLatestSequence(event.channel_id, event.sequence)
  }

  const normalizeChannelRecord = (value: Record<string, unknown>): ChannelRecord | null => {
    const channelId = typeof value.channel_id === 'string' ? value.channel_id : null
    const guildId = typeof value.guild_id === 'string' ? value.guild_id : null
    const name = typeof value.name === 'string' ? value.name : null
    const createdAt =
      typeof value.created_at === 'string' ? value.created_at : new Date().toISOString()
    if (!channelId || !guildId || !name) {
      return null
    }
    return {
      channel_id: channelId,
      guild_id: guildId,
      name,
      created_at: createdAt,
    }
  }

  const recordChannelCreated = (event: NotificationEventEnvelope) => {
    if (!event.event || typeof event.event !== 'object') {
      return
    }
    const record = normalizeChannelRecord(event.event as Record<string, unknown>)
    if (!record) {
      return
    }
    channelStore.upsertChannelRecord(record)
  }

  const handleNotificationEvent = (event: NotificationEventEnvelope) => {
    switch (event.kind) {
      case 'channel_message':
        recordChannelMessage(event)
        break
      case 'channel_created':
        recordChannelCreated(event)
        break
      default:
        break
    }
  }

  const handleMessage = (payload: string | ArrayBuffer | Blob) => {
    const parsed = parsePayload(payload)
    if (!parsed) {
      return
    }
    handleNotificationEvent(parsed)
  }

  const teardownSocket = (intent: 'manual' | 'pause' | null = 'manual') => {
    if (!socketRef.value) {
      return
    }
    closeIntent.value = intent
    try {
      socketRef.value.close(1000, intent === 'pause' ? 'Paused' : 'Client closed connection')
    } catch {
      // noop
    }
    socketRef.value = null
  }

  const openSocket = () => {
    if (!shouldConnect.value || !online.value) {
      return
    }

    const token = sessionStore.accessToken
    if (!token) {
      return
    }

    const existing = socketRef.value
    if (
      existing &&
      (existing.readyState === WebSocket.OPEN || existing.readyState === WebSocket.CONNECTING)
    ) {
      return
    }

    const url = buildSocketUrl(token)
    let socket: WebSocket | null = null

    try {
      socket = new WebSocket(url)
    } catch (err) {
      updateState({
        status: 'error',
        lastError: err instanceof Error ? err.message : 'Failed to open socket',
      })
      recordNetworkBreadcrumb('ws', {
        message: 'Notification socket failed to open',
        level: 'error',
        data: {
          url: url.toString(),
        },
      })
      scheduleReconnect()
      return
    }

    socketRef.value = socket
    updateState({
      status: 'connecting',
    })

    recordNetworkBreadcrumb('ws', {
      message: 'Notification socket connecting',
      level: 'info',
      data: {
        url: url.toString(),
      },
    })

    socket.addEventListener('open', () => {
      updateState({
        status: 'connected',
        attempt: 0,
        lastError: null,
      })
      clearReconnectTimer()
      recordNetworkBreadcrumb('ws', {
        message: 'Notification socket connected',
        level: 'info',
        data: {
          url: url.toString(),
        },
      })
    })

    socket.addEventListener('message', (event) => {
      handleMessage(event.data)
    })

    socket.addEventListener('close', (event) => {
      socketRef.value = null
      const intent = closeIntent.value
      closeIntent.value = null

      if (intent === 'manual') {
        updateState({
          status: 'idle',
        })
        clearReconnectTimer()
        return
      }

      if (intent === 'pause' || !online.value) {
        updateState({
          status: 'paused',
          lastError: null,
        })
        clearReconnectTimer()
        return
      }

      updateState({
        status: 'error',
        lastError: event.reason || 'Notification socket closed',
        attempt: state.value.attempt + 1,
      })
      recordNetworkBreadcrumb('ws', {
        message: 'Notification socket closed unexpectedly',
        level: 'warning',
        data: {
          code: event.code,
          reason: event.reason,
        },
      })
      scheduleReconnect()
    })

    socket.addEventListener('error', () => {
      updateState({
        status: 'error',
        lastError: 'Notification socket error',
        attempt: state.value.attempt + 1,
      })
      recordNetworkBreadcrumb('ws', {
        message: 'Notification socket error',
        level: 'error',
      })
      scheduleReconnect()
    })
  }

  const stop = () => {
    shouldConnect.value = false
    clearReconnectTimer()
    teardownSocket('manual')
    updateState({
      status: 'idle',
      attempt: 0,
      lastError: null,
    })
    latestSequenceByChannel.value = {}
  }

  return {
    status,
    latestSequenceByChannel,
    connect: () => {
      shouldConnect.value = true
      if (online.value) {
        openSocket()
      }
    },
    pause: () => {
      if (!shouldConnect.value) {
        return
      }
      teardownSocket('pause')
      updateState({
        status: 'paused',
        lastError: null,
      })
    },
    disconnect: stop,
  }
})
