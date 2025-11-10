import { defineStore, storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'

import { getRuntimeConfig } from '@/config/runtime'
import { useApiClient } from '@/composables/useApiClient'
import { useConnectivityStore } from '@/stores/connectivity'
import { useSessionStore } from '@/stores/session'
import { useTimelineStore } from '@/stores/timeline'
import type { ChannelEventEnvelope } from '@/types/messaging'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

export type RealtimeStatus = 'idle' | 'connecting' | 'connected' | 'paused' | 'error'

interface RealtimeState {
  channelId: string | null
  status: RealtimeStatus
  attempt: number
  connectionId: string | null
  lastError: string | null
  lastEventAt: number | null
}

const HEARTBEAT_INTERVAL_MS = 20_000
const VISIBILITY_PAUSE_DELAY_MS = 1_000
const MAX_BACKOFF_MS = 30_000
const TYPING_PREVIEW_THROTTLE_MS = 1_500

const createConnectionId = () => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }
  return `ws-${Date.now()}-${Math.random().toString(16).slice(2, 10)}`
}

const serializeData = (data: string | ArrayBuffer | Blob): string | null => {
  if (typeof data === 'string') {
    return data
  }
  if (data instanceof ArrayBuffer) {
    try {
      return new TextDecoder().decode(data)
    } catch {
      return null
    }
  }
  if (typeof Blob !== 'undefined' && data instanceof Blob) {
    return null
  }
  return null
}

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

const buildSocketUrl = (channelId: string, token: string) => {
  const base = resolveBaseUrl()
  const url = new URL(`/channels/${channelId}/ws`, base)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  url.searchParams.set('access_token', token)
  return url
}

export const useRealtimeStore = defineStore('realtime', () => {
  const state = ref<RealtimeState>({
    channelId: null,
    status: 'idle',
    attempt: 0,
    connectionId: null,
    lastError: null,
    lastEventAt: null,
  })

  const socketRef = ref<WebSocket | null>(null)
  const reconnectTimer = ref<ReturnType<typeof setTimeout> | null>(null)
  const heartbeatTimer = ref<ReturnType<typeof setInterval> | null>(null)
  const visibilityPauseTimer = ref<ReturnType<typeof setTimeout> | null>(null)
  const pausedReason = ref<'visibility' | 'network' | null>(null)
  const typingPreviewTimestamps = new Map<string, number>()

  const connectivityStore = useConnectivityStore()
  const { online, visibility } = storeToRefs(connectivityStore)
  const sessionStore = useSessionStore()
  const { lastSequenceByChannel } = storeToRefs(useTimelineStore())
  const timelineStore = useTimelineStore()

  const status = computed(() => state.value.status)
  const activeChannelId = computed(() => state.value.channelId)
  const attemptingReconnect = computed(
    () => state.value.status === 'error' && Boolean(reconnectTimer.value),
  )

  const clearReconnectTimer = () => {
    if (reconnectTimer.value !== null) {
      clearTimeout(reconnectTimer.value)
      reconnectTimer.value = null
    }
  }

  const clearHeartbeat = () => {
    if (heartbeatTimer.value !== null) {
      clearInterval(heartbeatTimer.value)
      heartbeatTimer.value = null
    }
  }

  const cleanupSocket = () => {
    const socket = socketRef.value
    if (socket && socket.readyState === WebSocket.OPEN) {
      try {
        socket.close(1000, 'client navigation')
      } catch (err) {
        console.warn('Failed to close websocket cleanly', err)
      }
    } else if (socket && socket.readyState === WebSocket.CONNECTING) {
      try {
        socket.close()
      } catch {
        /* noop */
      }
    }
    socketRef.value = null
    clearHeartbeat()
  }

  const updateState = (patch: Partial<RealtimeState>) => {
    state.value = {
      ...state.value,
      ...patch,
    }
  }

  const scheduleReconnect = () => {
    clearReconnectTimer()
    if (!state.value.channelId || pausedReason.value === 'visibility' || !online.value) {
      return
    }
    const attempt = state.value.attempt + 1
    const delay = Math.min(2 ** attempt * 1000, MAX_BACKOFF_MS)
    recordNetworkBreadcrumb('ws', {
      message: 'Realtime reconnect scheduled',
      level: 'warning',
      data: {
        channelId: state.value.channelId,
        connectionId: state.value.connectionId,
        attempt,
        delay,
      },
    })
    reconnectTimer.value = setTimeout(() => {
      openSocket(state.value.channelId!)
    }, delay)
    updateState({
      attempt,
      status: 'error',
    })
    connectivityStore.markDegraded('Realtime connection lost — retrying…')
  }

  const handleMessage = (channelId: string, raw: MessageEvent['data']) => {
    const serialized = serializeData(raw)
    if (!serialized) {
      return
    }

    try {
      const payload = JSON.parse(serialized) as ChannelEventEnvelope
      timelineStore.insertEvent(channelId, payload)
      updateState({
        lastEventAt: Date.now(),
        lastError: null,
      })
      connectivityStore.markDegraded(null)
    } catch (err) {
      console.warn('Failed to parse realtime payload', err)
      recordNetworkBreadcrumb('ws', {
        message: 'Failed to parse realtime payload',
        level: 'warning',
        data: {
          channelId,
          connectionId: state.value.connectionId,
        },
      })
    }
  }

  const startHeartbeat = () => {
    clearHeartbeat()
    heartbeatTimer.value = setInterval(() => {
      const socket = socketRef.value
      if (!socket || socket.readyState !== WebSocket.OPEN) {
        return
      }
      try {
        socket.send(JSON.stringify({ type: 'ping', ts: Date.now() }))
      } catch (err) {
        console.warn('Failed to send heartbeat', err)
      }
    }, HEARTBEAT_INTERVAL_MS)
  }

  const openSocket = (channelId: string) => {
    const token = sessionStore.accessToken
    if (!token) {
      updateState({
        status: 'error',
        lastError: 'Missing access token for realtime connection',
      })
      connectivityStore.markDegraded('Realtime unavailable — missing access token')
      recordNetworkBreadcrumb('ws', {
        message: 'Realtime connection blocked (missing access token)',
        level: 'error',
        data: {
          channelId,
        },
      })
      return
    }

    if (!online.value) {
      pausedReason.value = 'network'
      updateState({ status: 'paused' })
      connectivityStore.markDegraded('Offline — realtime updates paused')
      return
    }

    cleanupSocket()
    clearReconnectTimer()

    const connectionId = createConnectionId()
    const url = buildSocketUrl(channelId, token)
    const lastSequence = lastSequenceByChannel.value[channelId]
    if (typeof lastSequence === 'number') {
      url.searchParams.set('since', String(lastSequence))
    }
    url.searchParams.set('client_id', connectionId)

    let socket: WebSocket | null = null

    try {
      socket = new WebSocket(url)
    } catch (err) {
      updateState({
        status: 'error',
        lastError: extractError(err),
      })
      scheduleReconnect()
      recordNetworkBreadcrumb('ws', {
        message: 'Realtime connect failed',
        level: 'error',
        data: {
          channelId,
          connectionId,
          error: extractError(err),
        },
      })
      connectivityStore.markDegraded('Realtime connection failed — retrying shortly')
      return
    }

    socketRef.value = socket
    pausedReason.value = null
    updateState({
      channelId,
      status: 'connecting',
      connectionId,
    })
    recordNetworkBreadcrumb('ws', {
      message: 'Realtime connecting',
      level: 'info',
      data: {
        channelId,
        connectionId,
        since: lastSequenceByChannel.value[channelId] ?? null,
      },
    })

    socket.addEventListener('open', () => {
      updateState({
        status: 'connected',
        attempt: 0,
        lastError: null,
      })
      startHeartbeat()
      connectivityStore.markDegraded(null)
      recordNetworkBreadcrumb('ws', {
        message: 'Realtime connected',
        level: 'info',
        data: {
          channelId,
          connectionId,
        },
      })
    })

    socket.addEventListener('message', (event) => {
      handleMessage(channelId, event.data)
    })

    socket.addEventListener('close', (event) => {
      clearHeartbeat()
      socketRef.value = null
      if (pausedReason.value === 'visibility' || !online.value) {
        updateState({
          status: 'paused',
          lastError: null,
        })
        return
      }

      const reason =
        event.reason ||
        (event.code === 1000 ? 'Server closed connection' : `Socket closed (${event.code})`)

      updateState({
        status: 'error',
        lastError: reason,
      })

      recordNetworkBreadcrumb('ws', {
        message: 'Realtime disconnected',
        level: event.code === 1000 ? 'info' : 'warning',
        data: {
          channelId,
          connectionId,
          code: event.code,
          reason,
        },
      })

      connectivityStore.markDegraded(
        event.code === 1000 ? 'Realtime connection closed' : 'Realtime connection lost — retrying…',
      )

      scheduleReconnect()
    })

    socket.addEventListener('error', (event) => {
      console.warn('Realtime socket error', event)
      updateState({
        status: 'error',
        lastError: 'Realtime connection error',
      })
      recordNetworkBreadcrumb('ws', {
        message: 'Realtime socket error',
        level: 'error',
        data: {
          channelId,
          connectionId,
        },
      })
      connectivityStore.markDegraded('Realtime connection error — attempting to recover')
    })
  }

  const connect = (channelId: string | null) => {
    if (!channelId) {
      disconnect()
      return
    }

    if (state.value.channelId === channelId && state.value.status === 'connected') {
      return
    }

    updateState({
      channelId,
    })

    openSocket(channelId)
  }

  const disconnect = () => {
    clearReconnectTimer()
    cleanupSocket()
    updateState({
      channelId: null,
      status: 'idle',
      attempt: 0,
      lastError: null,
      connectionId: null,
    })
  }

  const pause = (reason: 'visibility' | 'network') => {
    if (state.value.status === 'idle') {
      return
    }
    pausedReason.value = reason
    clearReconnectTimer()
    cleanupSocket()
    updateState({
      status: 'paused',
    })
  }

  const resume = () => {
    if (!state.value.channelId) {
      return
    }
    const channelId = state.value.channelId
    pausedReason.value = null
    openSocket(channelId)
  }

  const sendTypingPreview = async (channelId: string | null, preview: string) => {
    if (!channelId) {
      return
    }

    const trimmed = preview.trim()
    if (!trimmed.length) {
      return
    }

    const now = Date.now()
    const lastSent = typingPreviewTimestamps.get(channelId) ?? 0
    if (now - lastSent < TYPING_PREVIEW_THROTTLE_MS) {
      return
    }
    typingPreviewTimestamps.set(channelId, now)

    const socket = socketRef.value
    const payload = {
      type: 'typing_preview',
      channel_id: channelId,
      preview: trimmed.slice(0, 120),
    }

    if (socket && socket.readyState === WebSocket.OPEN) {
      try {
        socket.send(JSON.stringify(payload))
        recordNetworkBreadcrumb('ws', {
          message: 'Typing preview sent via websocket',
          level: 'info',
          data: {
            channelId,
            connectionId: state.value.connectionId,
            length: payload.preview.length,
          },
        })
        return
      } catch (err) {
        console.warn('Failed to send typing preview via websocket', err)
        recordNetworkBreadcrumb('ws', {
          message: 'Typing preview websocket send failed',
          level: 'warning',
          data: {
            channelId,
            connectionId: state.value.connectionId,
            error: extractError(err),
          },
        })
      }
    }

    try {
      const api = useApiClient()
      await api(`/channels/${channelId}/typing-preview`, {
        method: 'POST',
        body: JSON.stringify({ preview: payload.preview }),
        headers: {
          'content-type': 'application/json',
        },
      })
      recordNetworkBreadcrumb('ws', {
        message: 'Typing preview sent via HTTP fallback',
        level: 'info',
        data: {
          channelId,
          length: payload.preview.length,
        },
      })
    } catch (err) {
      recordNetworkBreadcrumb('ws', {
        message: 'Typing preview HTTP fallback failed',
        level: 'warning',
        data: {
          channelId,
          error: extractError(err),
        },
      })
    }
  }

  const extractError = (err: unknown) => {
    if (err instanceof Error) {
      return err.message
    }
    if (typeof err === 'string') {
      return err
    }
    try {
      return JSON.stringify(err)
    } catch {
      return 'Unknown realtime error'
    }
  }

  watch(
    online,
    (onlineValue) => {
      if (onlineValue) {
        if (pausedReason.value === 'network' && state.value.channelId) {
          resume()
        }
      } else {
        pause('network')
      }
    },
    { immediate: true },
  )

  watch(
    visibility,
    (nextVisibility) => {
      if (nextVisibility === 'hidden') {
        if (visibilityPauseTimer.value !== null) {
          return
        }
        visibilityPauseTimer.value = setTimeout(() => {
          pause('visibility')
          if (visibilityPauseTimer.value !== null) {
            clearTimeout(visibilityPauseTimer.value)
            visibilityPauseTimer.value = null
          }
        }, VISIBILITY_PAUSE_DELAY_MS)
      } else if (nextVisibility === 'visible') {
        if (visibilityPauseTimer.value !== null) {
          clearTimeout(visibilityPauseTimer.value)
          visibilityPauseTimer.value = null
        }
        if (pausedReason.value === 'visibility') {
          resume()
        }
      }
    },
    { immediate: true },
  )

  return {
    state,
    status,
    activeChannelId,
    attemptingReconnect,
    connect,
    disconnect,
    pause,
    resume,
    sendTypingPreview,
  }
})
