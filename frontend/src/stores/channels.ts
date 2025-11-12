import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import type { ChannelRecord, ChannelUnreadState } from '~/types/messaging'

export type ChannelKind = 'text' | 'voice'

export interface ChannelSummary {
  id: string
  guildId: string
  label: string
  kind: ChannelKind
  icon?: string
  unread?: number
  description?: string | null
  createdAt?: string
}

const FETCH_TTL_MS = 30_000

const inferChannelKind = (name: string): ChannelKind =>
  /^voice-|^call-|^meeting-|^standup-/.test(name) ? 'voice' : 'text'

const mapChannelRecord = (record: ChannelRecord): ChannelSummary => {
  const label = record.name.trim()
  const kind = inferChannelKind(label.toLowerCase())
  return {
    id: record.channel_id,
    guildId: record.guild_id,
    label,
    kind,
    icon: kind === 'voice' ? 'i-heroicons-speaker-wave' : undefined,
    description: null,
    createdAt: record.created_at,
  }
}

export const useChannelStore = defineStore('channels', () => {
  const channelsByGuild = ref<Record<string, ChannelSummary[]>>({})
  const activeGuildId = ref<string | null>(null)
  const activeChannelId = ref<string | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const hydrated = ref(false)
  const lastFetchedAt = ref<number | null>(null)
  const inFlightFetches = new Map<string, Promise<void>>()
  const guildFetchTimestamps = ref<Record<string, number>>({})
  const latestSequenceByChannel = ref<Record<string, number>>({})
  const lastReadSequenceByChannel = ref<Record<string, number>>({})

  const channelsForGuild = computed(() => {
    if (!activeGuildId.value) {
      return []
    }
    return channelsByGuild.value[activeGuildId.value] ?? []
  })

  const activeChannel = computed(() => {
    if (!activeGuildId.value || !activeChannelId.value) {
      return null
    }

    const scoped = channelsByGuild.value[activeGuildId.value] ?? []
    return scoped.find((channel) => channel.id === activeChannelId.value) ?? null
  })

  const shouldSkipFetch = (guildId: string, force: boolean) => {
    if (force) {
      return false
    }

    const lastFetched = guildFetchTimestamps.value[guildId]
    if (!lastFetched) {
      return false
    }

    return Date.now() - lastFetched < FETCH_TTL_MS
  }

  async function fetchChannelsForGuild(guildId: string, force = false) {
    if (!guildId) {
      return
    }

    if (shouldSkipFetch(guildId, force)) {
      return
    }

    if (inFlightFetches.has(guildId)) {
      return inFlightFetches.get(guildId)
    }

    const api = useApiClient()
    loading.value = true
    error.value = null

    const request = (async () => {
      try {
        const payload = await api<ChannelRecord[]>(
          `/guilds/${guildId}/channels`,
        )
        const mapped = payload.map(mapChannelRecord)
        channelsByGuild.value = {
          ...channelsByGuild.value,
          [guildId]: mapped,
        }
        guildFetchTimestamps.value[guildId] = Date.now()
        hydrated.value = true
        lastFetchedAt.value = Date.now()

        if (activeGuildId.value === guildId) {
          ensureActiveChannelForGuild(guildId)
        }
      } catch (err) {
        error.value = extractErrorMessage(err) || 'Failed to load channels'
        throw err
      } finally {
        loading.value = false
        inFlightFetches.delete(guildId)
      }
    })()

    inFlightFetches.set(guildId, request)
    return request
  }

  async function hydrate(force = false) {
    if (!activeGuildId.value) {
      hydrated.value = true
      return
    }

    await fetchChannelsForGuild(activeGuildId.value, force)
  }

  async function setActiveGuild(guildId: string | null) {
    if (!guildId) {
      activeGuildId.value = null
      activeChannelId.value = null
      return
    }

    activeGuildId.value = guildId

    await fetchChannelsForGuild(guildId).catch(() => undefined)
    ensureActiveChannelForGuild(guildId)
  }

  async function setActiveChannel(channelId: string) {
    if (!activeGuildId.value) {
      error.value = 'No active guild selected'
      return
    }

    let scoped = channelsByGuild.value[activeGuildId.value] ?? []
    if (!scoped.length || !scoped.some((channel) => channel.id === channelId)) {
      await fetchChannelsForGuild(activeGuildId.value).catch(() => undefined)
      scoped = channelsByGuild.value[activeGuildId.value] ?? []
    }

    if (!scoped.some((channel) => channel.id === channelId)) {
      error.value = `Unknown channel: ${channelId}`
      return
    }

    activeChannelId.value = channelId
    error.value = null
    markChannelRead(channelId)
  }

  function ensureActiveChannelForGuild(guildId: string) {
    const scoped = channelsByGuild.value[guildId] ?? []
    if (!scoped.length) {
      activeChannelId.value =
        activeGuildId.value === guildId ? null : activeChannelId.value
      return
    }

    if (
      !activeChannelId.value ||
      !scoped.some((channel) => channel.id === activeChannelId.value)
    ) {
      const first = scoped[0]
      activeChannelId.value = first ? first.id : null
    }
  }

  function upsertChannelRecord(record: ChannelRecord) {
    const summary = mapChannelRecord(record)
    const guildChannels = channelsByGuild.value[summary.guildId] ?? []
    const index = guildChannels.findIndex(
      (channel) => channel.id === summary.id,
    )

    if (index >= 0) {
      guildChannels.splice(index, 1, summary)
    } else {
      guildChannels.push(summary)
    }

    channelsByGuild.value = {
      ...channelsByGuild.value,
      [summary.guildId]: guildChannels,
    }

    if (summary.guildId === activeGuildId.value) {
      ensureActiveChannelForGuild(summary.guildId)
    }
  }

  function updateLatestSequence(channelId: string, sequence: number) {
    const current = latestSequenceByChannel.value[channelId] ?? 0
    if (sequence <= current) {
      return
    }
    latestSequenceByChannel.value = {
      ...latestSequenceByChannel.value,
      [channelId]: sequence,
    }
  }

  function markChannelRead(channelId: string, sequence?: number | null) {
    const targetSequence =
      typeof sequence === 'number'
        ? sequence
        : latestSequenceByChannel.value[channelId] ?? 0
    const current = lastReadSequenceByChannel.value[channelId] ?? 0
    if (targetSequence <= current) {
      return
    }
    lastReadSequenceByChannel.value = {
      ...lastReadSequenceByChannel.value,
      [channelId]: targetSequence,
    }
  }

  function unreadCount(channelId: string) {
    const latest = latestSequenceByChannel.value[channelId] ?? 0
    if (!latest) {
      return 0
    }
    const read = lastReadSequenceByChannel.value[channelId] ?? 0
    return Math.max(0, latest - read)
  }

  async function syncUnreadState() {
    const api = useApiClient()
    try {
      const payload = await api<ChannelUnreadState[]>('/channels/unread')
      payload.forEach((entry) => {
        latestSequenceByChannel.value = {
          ...latestSequenceByChannel.value,
          [entry.channel_id]: entry.latest_sequence,
        }
        lastReadSequenceByChannel.value = {
          ...lastReadSequenceByChannel.value,
          [entry.channel_id]: entry.last_read_sequence,
        }
      })
    } catch (err) {
      console.warn('Failed to sync unread state', err)
    }
  }

  async function markChannelReadRemote(channelId: string, sequence?: number | null) {
    const targetSequence =
      typeof sequence === 'number'
        ? sequence
        : latestSequenceByChannel.value[channelId] ?? 0
    const current = lastReadSequenceByChannel.value[channelId] ?? 0
    if (targetSequence <= current) {
      return
    }

    markChannelRead(channelId, targetSequence)

    const api = useApiClient()
    try {
      await api(`/channels/${channelId}/read`, {
        method: 'POST',
        body: JSON.stringify({ sequence: targetSequence }),
        headers: {
          'content-type': 'application/json',
        },
      })
    } catch (err) {
      console.warn('Failed to persist read state', err)
    }
  }

  async function createChannel(guildId: string, name: string) {
    const api = useApiClient()
    try {
      const payload = await api<ChannelRecord>(
        `/guilds/${guildId}/channels`,
        {
          method: 'POST',
          body: JSON.stringify({ name }),
          headers: {
            'content-type': 'application/json',
          },
        },
      )

      upsertChannelRecord(payload)
      guildFetchTimestamps.value[guildId] = Date.now()

      return payload
    } catch (err) {
      error.value = extractErrorMessage(err) || 'Unable to create channel'
      throw err
    }
  }

  function channelById(channelId: string) {
    for (const channels of Object.values(channelsByGuild.value)) {
      const match = channels.find((channel) => channel.id === channelId)
      if (match) {
        return match
      }
    }
    return null
  }

  return {
    channelsByGuild,
    activeGuildId,
    activeChannelId,
    loading,
    error,
    hydrated,
    lastFetchedAt,
    guildFetchTimestamps,
    channelsForGuild,
    activeChannel,
    hydrate,
    setActiveGuild,
    setActiveChannel,
    fetchChannelsForGuild,
    createChannel,
    channelById,
    upsertChannelRecord,
    updateLatestSequence,
    markChannelRead,
    unreadCount,
    syncUnreadState,
    markChannelReadRemote,
  }
})
