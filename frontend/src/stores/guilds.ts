import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import type { GuildRecord } from '~/types/messaging'

export interface GuildSummary {
  id: string
  name: string
  initials: string
  notificationCount?: number
}

const FETCH_TTL_MS = 30_000

const deriveInitials = (name: string): string => {
  const trimmed = name.trim()
  if (!trimmed) {
    return '??'
  }

  const parts = trimmed.split(/\s+/).filter(Boolean)
  if (!parts.length) {
    return trimmed.slice(0, 2).toUpperCase()
  }

  const initials = parts.slice(0, 2).map((segment) => segment[0] ?? '').join('')
  return initials.toUpperCase()
}

const mapGuildRecord = (record: GuildRecord): GuildSummary => ({
  id: record.guild_id,
  name: record.name,
  initials: deriveInitials(record.name),
})

export const useGuildStore = defineStore('guilds', () => {
  const guilds = ref<GuildSummary[]>([])
  const activeGuildId = ref<string | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const hydrated = ref(false)
  const lastFetchedAt = ref<number | null>(null)
  let inflight: Promise<void> | null = null

  const activeGuild = computed(() => {
    if (!activeGuildId.value) {
      return null
    }

    return guilds.value.find((guild) => guild.id === activeGuildId.value) ?? null
  })

  const shouldSkipFetch = (force: boolean) => {
    if (force) {
      return false
    }
    if (!hydrated.value) {
      return false
    }
    if (!lastFetchedAt.value) {
      return false
    }
    return Date.now() - lastFetchedAt.value < FETCH_TTL_MS
  }

  async function fetchGuilds(force = false) {
    if (shouldSkipFetch(force)) {
      return
    }

    if (loading.value && inflight) {
      return inflight
    }

    const api = useApiClient()
    loading.value = true
    error.value = null

    inflight = (async () => {
      try {
        const response = await api<GuildRecord[]>('/guilds')
        const mapped = response.map(mapGuildRecord)

        guilds.value = mapped
        hydrated.value = true
        lastFetchedAt.value = Date.now()

        const firstGuild = mapped[0]

        if (!activeGuildId.value && firstGuild) {
          activeGuildId.value = firstGuild.id
        } else if (
          activeGuildId.value &&
          !mapped.some((guild) => guild.id === activeGuildId.value)
        ) {
          activeGuildId.value = firstGuild?.id ?? null
        }
      } catch (err) {
        error.value = extractErrorMessage(err) || 'Failed to load guilds'
        throw err
      } finally {
        loading.value = false
        inflight = null
      }
    })()

    return inflight
  }

  async function hydrate(force = false) {
    await fetchGuilds(force)
  }

  async function setActiveGuild(guildId: string) {
    if (!guilds.value.some((guild) => guild.id === guildId)) {
      await fetchGuilds(true).catch(() => undefined)
    }

    if (!guilds.value.some((guild) => guild.id === guildId)) {
      error.value = `Unknown guild: ${guildId}`
      return
    }

    activeGuildId.value = guildId
    error.value = null
  }

  function addOrUpdateGuild(record: GuildRecord) {
    const summary = mapGuildRecord(record)
    const existingIndex = guilds.value.findIndex(
      (guild) => guild.id === summary.id,
    )

    if (existingIndex >= 0) {
      guilds.value.splice(existingIndex, 1, summary)
    } else {
      guilds.value.unshift(summary)
    }
  }

  async function createGuild(name: string) {
    const api = useApiClient()
    try {
      const payload = await api<GuildRecord>('/guilds', {
        method: 'POST',
        body: JSON.stringify({ name }),
        headers: {
          'content-type': 'application/json',
        },
      })

      addOrUpdateGuild(payload)
      if (!activeGuildId.value) {
        activeGuildId.value = payload.guild_id
      }

      return payload
    } catch (err) {
      error.value = extractErrorMessage(err) || 'Unable to create guild'
      throw err
    }
  }

  return {
    guilds,
    activeGuildId,
    loading,
    error,
    hydrated,
    lastFetchedAt,
    activeGuild,
    hydrate,
    fetchGuilds,
    setActiveGuild,
    createGuild,
    addOrUpdateGuild,
  }
})
