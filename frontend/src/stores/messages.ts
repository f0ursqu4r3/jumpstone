import { defineStore, storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import { useConnectivityStore } from '@/stores/connectivity'
import { useSessionStore } from '@/stores/session'
import { useTimelineStore } from '@/stores/timeline'
import type {
  ChannelEventEnvelope,
  MessageComposeRequest,
  MessageCreateResponse,
} from '@/types/messaging'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

const MAX_CONTENT_LENGTH = 4_000

interface PendingMessage {
  id: string
  channelId: string
  content: string
  status: 'queued' | 'sending' | 'failed'
  error: string | null
  retries: number
  optimisticId: string
  createdAt: number
}

const createLocalEvent = (
  channelId: string,
  sender: string,
  content: string,
): Omit<ChannelEventEnvelope, 'sequence'> => ({
  channel_id: channelId,
  event: {
    schema_version: 1,
    event_id:
      typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
        ? crypto.randomUUID()
        : `pending-${Date.now()}`,
    event_type: 'message',
    room_id: channelId,
    sender,
    origin_server: null,
    origin_ts: Date.now(),
    content: { content },
    prev_events: [],
    auth_events: [],
    signatures: {},
  },
})

const createMessageId = (channelId: string) =>
  `${channelId}-${Date.now()}-${Math.random().toString(16).slice(2, 10)}`

const createRequestId = () =>
  typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
    ? crypto.randomUUID()
    : Math.random().toString(16).slice(2)

export const useMessageComposerStore = defineStore('message-composer', () => {
  const queue = ref<PendingMessage[]>([])
  const lastError = ref<string | null>(null)
  const flushing = ref(false)

  const connectivityStore = useConnectivityStore()
  const timelineStore = useTimelineStore()
  const sessionStore = useSessionStore()
  const { online } = storeToRefs(connectivityStore)
  const { profile, displayName, identifier, isAuthenticated } = storeToRefs(sessionStore)

  const queuedCount = computed(() =>
    queue.value.filter((entry) => entry.status === 'queued' || entry.status === 'failed').length,
  )
  const hasFailures = computed(() => queue.value.some((entry) => entry.status === 'failed'))

  const currentSenderId = computed(() => profile.value?.userId ?? '')
  const optimisticSenderLabel = computed(() => displayName.value || identifier.value || 'You')

  const api = () => useApiClient()

  const trimContent = (value: string) => value.replace(/\s+$/, '')

  const validateContent = (content: string) => {
    const trimmed = content.trim()
    if (!trimmed.length) {
      return 'Message cannot be empty.'
    }
    if (trimmed.length > MAX_CONTENT_LENGTH) {
      return `Messages are limited to ${MAX_CONTENT_LENGTH.toLocaleString()} characters.`
    }
    return null
  }

  const removeFromQueue = (id: string) => {
    queue.value = queue.value.filter((entry) => entry.id !== id)
  }

  const enqueue = (record: PendingMessage) => {
    queue.value = [...queue.value, record]
  }

  const markQueuedOptimistic = (channelId: string, localId: string) => {
    timelineStore.markOptimisticQueued(channelId, localId, 'Queued – will retry when online')
  }

  const applyFailure = (entry: PendingMessage, message: string) => {
    entry.status = 'failed'
    entry.error = message
    lastError.value = message
    timelineStore.markOptimisticFailed(entry.channelId, entry.optimisticId, message)
    connectivityStore.markDegraded(message)
  }

  const attemptSend = async (entry: PendingMessage) => {
    if (!isAuthenticated.value) {
      applyFailure(entry, 'Session not ready. Refresh and try again.')
      return false
    }

    entry.status = 'sending'
    entry.error = null
    entry.retries += 1
    timelineStore.markOptimisticPending(entry.channelId, entry.optimisticId, 'Sending…')

    const requestId = createRequestId()

    try {
      const payload: MessageComposeRequest = {
        sender: currentSenderId.value || '',
        content: entry.content,
      }

      const response = await api()<MessageCreateResponse>(
        `/channels/${entry.channelId}/messages`,
        {
          method: 'POST',
          body: JSON.stringify(payload),
          headers: {
            'content-type': 'application/json',
            'x-request-id': requestId,
          },
        },
      )

      timelineStore.markOptimisticSequence(entry.channelId, entry.optimisticId, response.sequence)
      removeFromQueue(entry.id)
      lastError.value = null
      connectivityStore.markDegraded(null)
      return true
    } catch (err) {
      const message =
        extractErrorMessage(err) ||
        'Message failed to send. Check your connection and try again.'
      applyFailure(entry, message)
      const hasResponse = Boolean((err as { response?: unknown }).response)
      recordNetworkBreadcrumb('api', {
        message: `POST /channels/${entry.channelId}/messages failed`,
        level: hasResponse ? 'warning' : 'error',
        data: {
          channelId: entry.channelId,
          requestId,
          retries: entry.retries,
          error: message,
        },
      })
      return false
    }
  }

  const flushQueue = async () => {
    if (flushing.value || !online.value) {
      return
    }
    flushing.value = true

    try {
      // Create a shallow copy because entries may be removed during iteration.
      const snapshot = [...queue.value]
      for (const entry of snapshot) {
        if (!online.value) {
          break
        }
        if (entry.status === 'queued' || entry.status === 'failed') {
          await attemptSend(entry)
        }
      }
    } finally {
      flushing.value = false
    }
  }

  const sendMessage = async (channelId: string, rawContent: string) => {
    const validationError = validateContent(rawContent)
    if (validationError) {
      return {
        ok: false,
        queued: false,
        error: validationError,
      }
    }

    if (!isAuthenticated.value) {
      return {
        ok: false,
        queued: false,
        error: 'Session not initialized. Authenticate again and retry.',
      }
    }

    const trimmedContent = trimContent(rawContent)
    const optimisticEnvelope = createLocalEvent(
      channelId,
      optimisticSenderLabel.value,
      trimmedContent,
    )
    const localId = timelineStore.addOptimisticEvent(channelId, optimisticEnvelope)

    const message: PendingMessage = {
      id: createMessageId(channelId),
      channelId,
      content: trimmedContent,
      status: online.value ? 'sending' : 'queued',
      error: null,
      retries: 0,
      optimisticId: localId,
      createdAt: Date.now(),
    }

    enqueue(message)

    if (!online.value) {
      markQueuedOptimistic(channelId, localId)
      return {
        ok: true,
        queued: true,
      }
    }

    const succeeded = await attemptSend(message)
    return {
      ok: succeeded,
      queued: false,
      error: succeeded ? null : message.error,
    }
  }

  const retryMessage = async (id: string) => {
    const entry = queue.value.find((message) => message.id === id)
    if (!entry) {
      return
    }
    await attemptSend(entry)
  }

  const retryOptimistic = async (localId: string) => {
    const entry = queue.value.find((message) => message.optimisticId === localId)
    if (!entry) {
      return
    }
    await attemptSend(entry)
  }

  watch(
    online,
    (onlineValue) => {
      if (onlineValue) {
        connectivityStore.markDegraded(null)
        flushQueue().catch((err) => {
          console.warn('Failed to flush queued messages', err)
        })
      } else if (queue.value.length) {
        connectivityStore.markDegraded('Offline – queued messages will resend automatically')
        queue.value.forEach((entry) => {
          entry.status = 'queued'
          entry.error = null
          timelineStore.markOptimisticQueued(entry.channelId, entry.optimisticId, 'Queued – offline')
        })
      }
    },
    { immediate: true },
  )

  return {
    queue,
    lastError,
    queuedCount,
    hasFailures,
    sendMessage,
    retryMessage,
    retryOptimistic,
    flushQueue,
  }
})
