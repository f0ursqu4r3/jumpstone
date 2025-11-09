import { defineStore } from 'pinia'
import { ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

export interface KeyPackage {
  identity: string
  ciphersuite: string
  signature_key: string
  hpke_public_key: string
  rotated_at?: string | null
}

export const useMlsStore = defineStore('mls', () => {
  const keyPackages = ref<KeyPackage[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const lastFetchedAt = ref<string | null>(null)

  const api = () => useApiClient()

  const fetchKeyPackages = async () => {
    if (loading.value) {
      return
    }

    loading.value = true
    error.value = null

    try {
      const payload = await api()<KeyPackage[]>('/mls/key-packages')
      keyPackages.value = payload
      lastFetchedAt.value = new Date().toISOString()

      recordNetworkBreadcrumb('api', {
        message: 'Fetched MLS key packages',
        level: 'info',
        data: { count: payload.length },
      })
    } catch (err) {
      const status = (err as { response?: { status?: number } }).response?.status
      if (status === 501) {
        error.value = 'MLS is not enabled on this homeserver.'
        keyPackages.value = []
        return
      }

      const message = extractErrorMessage(err) || 'Unable to load MLS key packages'
      error.value = message

      recordNetworkBreadcrumb('api', {
        message: 'MLS key package fetch failed',
        level: 'warning',
        data: { error: message },
      })

      throw err
    } finally {
      loading.value = false
    }
  }

  return {
    keyPackages,
    loading,
    error,
    lastFetchedAt,
    fetchKeyPackages,
  }
})
