import { defineStore } from 'pinia'
import { ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import type { CurrentUserDevice } from '~/types/session'

const mockDevices: CurrentUserDevice[] = [
  {
    device_id: 'relay-devkit',
    device_name: 'DevKit (mock)',
    last_seen_at: new Date(Date.now() - 60 * 60 * 1000).toISOString(),
    ip_address: '10.0.0.42',
    user_agent: 'Rusty-Link/0.1 (mock)',
  },
]

export const useSessionDevicesStore = defineStore('session-devices', () => {
  const devices = ref<CurrentUserDevice[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const hydrated = ref(false)

  const api = () => useApiClient()

  const applyFallback = () => {
    devices.value = mockDevices
    error.value = null
    hydrated.value = true
  }

  const fetchDevices = async () => {
    if (loading.value) {
      return
    }

    loading.value = true
    error.value = null

    try {
      const payload = await api()<CurrentUserDevice[]>('/sessions/devices')
      devices.value = payload
      hydrated.value = true
    } catch (err) {
      const status = (err as { response?: { status?: number } }).response?.status
      if (status === 404 || status === 501) {
        applyFallback()
        return
      }
      error.value = extractErrorMessage(err) || 'Unable to load devices'
      throw err
    } finally {
      loading.value = false
      hydrated.value = true
    }
  }

  return {
    devices,
    loading,
    error,
    hydrated,
    fetchDevices,
  }
})
