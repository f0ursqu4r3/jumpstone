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
const OPTIMISTIC_BASE_SEQUENCE = Number.MAX_SAFE_INTEGER - 10_000

export type TimelineStatus = 'pending' | 'queued' | 'failed' | 'sent'

export interface TimelineEntry extends ChannelEventEnvelope {
  optimistic?: boolean
  localId?: string
  pendingSequence?: number | null
  createdAt?: number
  status?: TimelineStatus
  statusMessage?: string | null
  ackedAt?: number | null
}

const isSameContent = (a: ChannelEventEnvelope, b: ChannelEventEnvelope) => {
  if (a.event.event_type !== b.event.event_type) {
    return false
  }
  if (a.event.sender !== b.event.sender) {
    return false
  }

  try {
    return JSON.stringify(a.event.content) === JSON.stringify(b.event.content)
  } catch {
    return false
  }
}

export const useTimelineStore = defineStore('timeline', () => {
  const eventsByChannel = ref<Record<string, TimelineEntry[]>>({})
  const loadingByChannel = ref<Record<string, boolean>>({})
  const errorByChannel = ref<Record<string, string | null>>({})
  const lastSequenceByChannel = ref<Record<string, number>>({})
  const lastFetchedAt = ref<Record<string, number>>({})
  const inFlight = new Map<string, Promise<void>>()
  const optimisticCounter = ref(0)

  const isLoading = (channelId: string) => Boolean(loadingByChannel.value[channelId])
  const errorForChannel = (channelId: string) => errorByChannel.value[channelId] ?? null

  const committedSequenceForChannel = (channelId: string) => {
    const entries = eventsByChannel.value[channelId] ?? []
    const committed = entries.filter((entry) => !entry.optimistic)
    if (!committed.length) {
      return null
    }
    return committed[committed.length - 1]?.sequence ?? null
  }

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

  const sortTimeline = (entries: TimelineEntry[]) =>
    entries.sort((a, b) => {
      const aOpt = Boolean(a.optimistic)
      const bOpt = Boolean(b.optimistic)

      if (aOpt && bOpt) {
        return (a.createdAt ?? 0) - (b.createdAt ?? 0)
      }
      if (aOpt) {
        return 1
      }
      if (bOpt) {
        return -1
      }

      return a.sequence - b.sequence
    })

  const normalizeEvents = (incoming: ChannelEventEnvelope[]): TimelineEntry[] =>
    incoming.map((entry) => ({
      ...entry,
      optimistic: false,
      localId: undefined,
      pendingSequence: null,
      createdAt:
        typeof entry.event.origin_ts === 'number' && Number.isFinite(entry.event.origin_ts)
          ? entry.event.origin_ts
          : Date.now(),
      status: undefined,
      statusMessage: null,
      ackedAt: null,
    }))

  const replaceChannelEvents = (channelId: string, events: TimelineEntry[]) => {
    eventsByChannel.value = {
      ...eventsByChannel.value,
      [channelId]: sortTimeline(events),
    }

    const latestSequence = committedSequenceForChannel(channelId)
    if (typeof latestSequence === 'number') {
      lastSequenceByChannel.value[channelId] = latestSequence
    } else {
      delete lastSequenceByChannel.value[channelId]
    }
  }

  const removeOptimisticBy = (channelId: string, predicate: (entry: TimelineEntry) => boolean) => {
    const existing = eventsByChannel.value[channelId]
    if (!existing || !existing.length) {
      return
    }

    const next = existing.filter((entry) => !(entry.optimistic && predicate(entry)))
    if (next.length === existing.length) {
      return
    }

    replaceChannelEvents(channelId, next)
  }

  const upsertEvents = (
    channelId: string,
    incoming: ChannelEventEnvelope[],
    { append }: { append: boolean },
  ) => {
    const normalized = normalizeEvents(incoming)
    const existing = eventsByChannel.value[channelId] ?? []
    const optimistic = existing.filter((entry) => entry.optimistic)
    const committed = existing.filter((entry) => !entry.optimistic)

    if (!append) {
      replaceChannelEvents(channelId, [...normalized, ...optimistic])
      return
    }

    const sequenceSet = new Set(committed.map((event) => event.sequence))
    const appended: TimelineEntry[] = []

    normalized.forEach((event) => {
      if (!sequenceSet.has(event.sequence)) {
        appended.push(event)
        sequenceSet.add(event.sequence)
      }
    })

    replaceChannelEvents(channelId, [...committed, ...appended, ...optimistic])
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
    removeOptimisticBy(channelId, (entry) => {
      if (typeof entry.pendingSequence === 'number') {
        return entry.pendingSequence === event.sequence
      }
      return isSameContent(entry, event)
    })
    upsertEvents(channelId, [event], { append: true })
  }

  function addOptimisticEvent(channelId: string, event: Omit<ChannelEventEnvelope, 'sequence'>) {
    const localId = `${channelId}-${Date.now()}-${optimisticCounter.value++}`
    const optimisticEntry: TimelineEntry = {
      ...event,
      sequence: OPTIMISTIC_BASE_SEQUENCE - optimisticCounter.value,
      optimistic: true,
      localId,
      pendingSequence: null,
      createdAt: Date.now(),
      status: 'pending',
      statusMessage: null,
      ackedAt: null,
    }

    const existing = eventsByChannel.value[channelId] ?? []
    replaceChannelEvents(channelId, [...existing, optimisticEntry])

    return localId
  }

  function markOptimisticSequence(channelId: string, localId: string, sequence: number) {
    const existing = eventsByChannel.value[channelId]
    if (!existing) {
      return
    }

    const idx = existing.findIndex((entry) => entry.optimistic && entry.localId === localId)
    if (idx < 0) {
      return
    }

    const target = existing[idx]
    if (!target) {
      return
    }

    const updated: TimelineEntry = {
      ...target,
      sequence,
      optimistic: false,
      localId: undefined,
      pendingSequence: null,
      ackedAt: Date.now(),
      status: undefined,
      statusMessage: null,
    }

    const next = [...existing]
    next.splice(idx, 1, updated)

    replaceChannelEvents(channelId, next)
  }

  function markOptimisticFailed(channelId: string, localId: string, errorMessage: string) {
    const existing = eventsByChannel.value[channelId]
    if (!existing) {
      return
    }

    const idx = existing.findIndex((entry) => entry.optimistic && entry.localId === localId)
    if (idx < 0) {
      return
    }

    const target = existing[idx]
    if (!target) {
      return
    }

    const updated: TimelineEntry = {
      ...target,
      status: 'failed',
      statusMessage: errorMessage,
    }

    const next = [...existing]
    next.splice(idx, 1, updated)

    replaceChannelEvents(channelId, next)
  }

  function markOptimisticPending(channelId: string, localId: string, message: string | null = null) {
    const existing = eventsByChannel.value[channelId]
    if (!existing) {
      return
    }

    const idx = existing.findIndex((entry) => entry.optimistic && entry.localId === localId)
    if (idx < 0) {
      return
    }

    const target = existing[idx]
    if (!target) {
      return
    }

    const updated: TimelineEntry = {
      ...target,
      status: 'pending',
      statusMessage: message,
    }

    const next = [...existing]
    next.splice(idx, 1, updated)

    replaceChannelEvents(channelId, next)
  }

  function markOptimisticQueued(channelId: string, localId: string, message: string | null) {
    const existing = eventsByChannel.value[channelId]
    if (!existing) {
      return
    }

    const idx = existing.findIndex((entry) => entry.optimistic && entry.localId === localId)
    if (idx < 0) {
      return
    }

    const target = existing[idx]
    if (!target) {
      return
    }

    const updated: TimelineEntry = {
      ...target,
      status: 'queued',
      statusMessage: message,
    }

    const next = [...existing]
    next.splice(idx, 1, updated)

    replaceChannelEvents(channelId, next)
  }

  function removeOptimistic(channelId: string, localId: string) {
    removeOptimisticBy(channelId, (entry) => entry.localId === localId)
  }

  function updateEventContent(
    channelId: string,
    eventId: string | null | undefined,
    content: string,
  ) {
    if (!eventId) {
      return
    }
    const existing = eventsByChannel.value[channelId]
    if (!existing) {
      return
    }
    const idx = existing.findIndex(
      (entry) => entry.event.event_id && entry.event.event_id === eventId,
    )
    if (idx < 0) {
      return
    }
    const target = existing[idx]
    if (!target) {
      return
    }
    const nextContent = {
      ...(target.event.content ?? {}),
      content,
    }
    const updated: TimelineEntry = {
      ...target,
      event: {
        ...target.event,
        content: nextContent,
      },
      optimistic: target.optimistic,
    }
    const next = [...existing]
    next.splice(idx, 1, updated)
    replaceChannelEvents(channelId, next)
  }

  const timelineFor = (channelId: string) =>
    computed<TimelineEntry[]>(() => eventsByChannel.value[channelId] ?? [])

  const getCommittedSequence = (channelId: string) => committedSequenceForChannel(channelId)

  return {
    eventsByChannel,
    loadingByChannel,
    errorByChannel,
    lastSequenceByChannel,
    lastFetchedAt,
    loadChannel,
    clearChannel,
    insertEvent,
    addOptimisticEvent,
    markOptimisticSequence,
    markOptimisticPending,
    markOptimisticFailed,
    markOptimisticQueued,
    removeOptimistic,
    updateEventContent,
    timelineFor,
    isLoading,
    errorForChannel,
    getCommittedSequence,
  }
})
