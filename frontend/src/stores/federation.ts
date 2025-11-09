import { defineStore } from 'pinia'
import { ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

export interface GuildFederationContext {
  guildId: string
  remoteServers: string[]
  trustLevel: 'trusted' | 'limited' | 'untrusted'
  updatedAt: string
}

export interface HandshakeVector {
  vector_id: string
  origin: string
  payload: Record<string, unknown>
}

const mockContext = (guildId: string): GuildFederationContext => ({
  guildId,
  remoteServers: ['relay.dev.openguild.net'],
  trustLevel: 'limited',
  updatedAt: new Date().toISOString(),
})

export const useFederationStore = defineStore('federation', () => {
  const contexts = ref<Record<string, GuildFederationContext>>({})
  const loading = ref(false)
  const error = ref<string | null>(null)
  const handshakeVectors = ref<HandshakeVector[] | null>(null)
  const handshakeLoading = ref(false)
  const handshakeError = ref<string | null>(null)

  const api = () => useApiClient()

  const setContext = (ctx: GuildFederationContext) => {
    contexts.value = {
      ...contexts.value,
      [ctx.guildId]: ctx,
    }
  }

  const fetchContext = async (guildId: string | null | undefined) => {
    if (!guildId) {
      return
    }

    loading.value = true
    error.value = null

    try {
      const payload = await api()<GuildFederationContext>(`/federation/guilds/${guildId}/context`)
      setContext(payload)
    } catch (err) {
      const status = (err as { response?: { status?: number } }).response?.status
      if (status === 404 || status === 501) {
        const fallback = mockContext(guildId)
        setContext(fallback)
        recordNetworkBreadcrumb('api', {
          message: 'Federation context fallback (mock)',
          level: 'warning',
          data: { guildId },
        })
        return
      }
      const message = extractErrorMessage(err) || 'Unable to load federation context'
      error.value = message
      recordNetworkBreadcrumb('api', {
        message: 'Federation context fetch failed',
        level: 'error',
        data: { guildId, error: message },
      })
      throw err
    } finally {
      loading.value = false
    }
  }

  const fetchHandshakeVectors = async () => {
    handshakeLoading.value = true
    handshakeError.value = null

    try {
      const payload = await api()<HandshakeVector[]>('/mls/handshake-test-vectors')
      handshakeVectors.value = payload
      recordNetworkBreadcrumb('api', {
        message: 'Fetched handshake test vectors',
        level: 'info',
        data: { count: payload.length },
      })
    } catch (err) {
      const message = extractErrorMessage(err) || 'Unable to load handshake vectors'
      handshakeError.value = message
      recordNetworkBreadcrumb('api', {
        message: 'Handshake vector fetch failed',
        level: 'warning',
        data: { error: message },
      })
      throw err
    } finally {
      handshakeLoading.value = false
    }
  }

  const contextForGuild = (guildId: string | null | undefined) => {
    if (!guildId) {
      return null
    }
    return contexts.value[guildId] ?? null
  }

  return {
    contexts,
    loading,
    error,
    handshakeVectors,
    handshakeLoading,
    handshakeError,
    fetchContext,
    fetchHandshakeVectors,
    contextForGuild,
  }
})
