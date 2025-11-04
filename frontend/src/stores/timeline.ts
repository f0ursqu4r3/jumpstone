import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import type { ChannelEventEnvelope } from '~/types/messaging'

interface LoadOptions {
  force?: boolean
  refresh?: boolean
  limit?: number
}

const DEFAULT_LIMIT = 50
const FETCH_TTL_MS = 15_000

export const useTimelineStore = defineStore('timeline', () => {
  const eventsByChannel = ref<Record<string, ChannelEventEnvelope[]>>({})
  const loadingByChannel = ref<Record<string, boolean>>({})
  const errorByChannel = ref<Record<string, string | null>>({})
  const lastSequenceByChannel = ref<Record<string, number>>({})
  const lastFetchedAt = ref<Record<string, number>>({})
  const inFlight = new Map<string, Promise<void>>()

  const isLoading = (channelId: string) => Boolean(loadingByChannel.value[channelId])
  const errorForChannel = (channelId: string) => errorByChannel.value[channelId] ?? null

  const shouldSkip = (channelId: string, options: LoadOptions) => {
    if (options.force || options.refresh) {
      return false
    }

    const lastFetched = lastFetchedAt.value[channelId]
    if (!lastFetched) {
      return false
    }

    const elapsed = Date.now() - lastFetched
    if (elapsed > FETCH_TTL_MS) {
      return false
    }

    return (eventsByChannel.value[channelId] ?? []).length > 0
  }

  const setLoading = (channelId: string, value: boolean) => {
    loadingByChannel.value = {
      ...loadingByChannel.value,
      [channelId]: value,
    }
  }

  const setError = (channelId: string, value: string | null) => {
    errorByChannel.value = {
      ...errorByChannel.value,
      [channelId]: value,
    }
  }

  const upsertEvents = (
    channelId: string,
    incoming: ChannelEventEnvelope[],
    { append }: { append: boolean },
  ) => {
    if (!append) {
      eventsByChannel.value = {
        ...eventsByChannel.value,
        [channelId]: incoming,
      }
      const lastIncoming = incoming[incoming.length - 1]
      if (lastIncoming) {
        lastSequenceByChannel.value[channelId] = lastIncoming.sequence
      }
      return
    }

    const existing = eventsByChannel.value[channelId] ?? []
    const sequenceSet = new Set(existing.map((event) => event.sequence))
    const merged = [...existing]

    incoming.forEach((event) => {
      if (!sequenceSet.has(event.sequence)) {
        merged.push(event)
        sequenceSet.add(event.sequence)
      }
    })

    merged.sort((a, b) => a.sequence - b.sequence)

    eventsByChannel.value = {
      ...eventsByChannel.value,
      [channelId]: merged,
    }

    const lastMerged = merged[merged.length - 1]
    if (lastMerged) {
      lastSequenceByChannel.value[channelId] = lastMerged.sequence
    }
  }

  async function loadChannel(channelId: string, options: LoadOptions = {}): Promise<void> {
    if (!channelId) {
      return
    }

    const mergedOptions: Required<LoadOptions> = {
      force: Boolean(options.force),
      refresh: Boolean(options.refresh),
      limit: options.limit ?? DEFAULT_LIMIT,
    }

    if (shouldSkip(channelId, mergedOptions)) {
      return
    }

    if (inFlight.has(channelId)) {
      return inFlight.get(channelId)
    }

    const api = useApiClient()
    setLoading(channelId, true)
    setError(channelId, null)

    const request = (async () => {
      try {
        const params = new URLSearchParams()
        if (mergedOptions.limit) {
          params.set('limit', String(mergedOptions.limit))
        }

        const lastSequence = lastSequenceByChannel.value[channelId]
        if (mergedOptions.refresh && lastSequence) {
          params.set('since', String(lastSequence))
        }

        const suffix = params.toString() ? `?${params.toString()}` : ''
        const payload = await api<ChannelEventEnvelope[]>(`/channels/${channelId}/events${suffix}`)

        upsertEvents(channelId, payload, {
          append: mergedOptions.refresh && Boolean(lastSequence),
        })

        lastFetchedAt.value[channelId] = Date.now()
      } catch (err) {
        setError(channelId, extractErrorMessage(err) || 'Failed to load channel timeline')
        throw err
      } finally {
        setLoading(channelId, false)
        inFlight.delete(channelId)
      }
    })()

    inFlight.set(channelId, request)
    return request
  }

  function clearChannel(channelId: string) {
    delete eventsByChannel.value[channelId]
    delete lastSequenceByChannel.value[channelId]
    delete lastFetchedAt.value[channelId]
    delete loadingByChannel.value[channelId]
    delete errorByChannel.value[channelId]
  }

  function insertEvent(channelId: string, event: ChannelEventEnvelope) {
    upsertEvents(channelId, [event], { append: true })
  }

  const timelineFor = (channelId: string) => computed(() => eventsByChannel.value[channelId] ?? [])

  return {
    eventsByChannel,
    loadingByChannel,
    errorByChannel,
    lastSequenceByChannel,
    lastFetchedAt,
    loadChannel,
    clearChannel,
    insertEvent,
    timelineFor,
    isLoading,
    errorForChannel,
  }
})
