import { defineStore } from 'pinia'
import { ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

export type SearchResultType = 'message' | 'channel' | 'user'

export interface SearchResult {
  id: string
  type: SearchResultType
  title: string
  subtitle?: string | null
  snippet?: string | null
  channelId?: string | null
  eventId?: string | null
}

interface SearchResponseItem {
  id: string
  type: SearchResultType
  title: string
  subtitle?: string | null
  snippet?: string | null
  channel_id?: string | null
  event_id?: string | null
}

const mockResults = (query: string): SearchResult[] => {
  const sanitized = query.trim()
  if (!sanitized) {
    return []
  }
  return [
    {
      id: `mock-message-${sanitized}`,
      type: 'message',
      title: `Mock result for “${sanitized}”`,
      subtitle: '#general · Maya',
      snippet: `This is a placeholder search hit for ${sanitized}.`,
      channelId: 'general',
      eventId: '$mock1',
    },
    {
      id: `mock-channel-${sanitized}`,
      type: 'channel',
      title: `#${sanitized.replace(/\s+/g, '-').toLowerCase()}-ops`,
      subtitle: 'Channel suggestion',
      snippet: 'Channel level search result placeholder.',
      channelId: `${sanitized}-ops`,
    },
  ]
}

export const useSearchStore = defineStore('search', () => {
  const results = ref<SearchResult[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const lastQuery = ref('')
  const lastFetchedAt = ref<string | null>(null)

  const api = () => useApiClient()

  const transformPayload = (payload: SearchResponseItem[]): SearchResult[] =>
    payload.map((item) => ({
      id: item.id,
      type: item.type,
      title: item.title,
      subtitle: item.subtitle ?? null,
      snippet: item.snippet ?? null,
      channelId: item.channel_id ?? null,
      eventId: item.event_id ?? null,
    }))

  const performSearch = async (query: string) => {
    const trimmed = query.trim()
    lastQuery.value = trimmed
    error.value = null

    if (!trimmed.length) {
      results.value = []
      return
    }

    loading.value = true

    try {
      const payload = await api()<SearchResponseItem[]>(
        `/search/messages?q=${encodeURIComponent(trimmed)}`,
      )

      results.value = transformPayload(payload)
      lastFetchedAt.value = new Date().toISOString()

      recordNetworkBreadcrumb('api', {
        message: 'Global search succeeded',
        level: 'info',
        data: { query: trimmed, count: payload.length },
      })
    } catch (err) {
      const status = (err as { response?: { status?: number } }).response?.status

      if (status === 404 || status === 501) {
        results.value = mockResults(trimmed)
        lastFetchedAt.value = new Date().toISOString()
        error.value = null
        recordNetworkBreadcrumb('api', {
          message: 'Search fallback (mock results)',
          level: 'warning',
          data: { query: trimmed },
        })
        return
      }

      const message = extractErrorMessage(err) || 'Search request failed'
      error.value = message
      recordNetworkBreadcrumb('api', {
        message: 'Global search failed',
        level: 'error',
        data: { query: trimmed, error: message },
      })
      throw err
    } finally {
      loading.value = false
    }
  }

  const reset = () => {
    results.value = []
    error.value = null
    lastQuery.value = ''
  }

  return {
    results,
    loading,
    error,
    lastQuery,
    lastFetchedAt,
    performSearch,
    reset,
  }
})
