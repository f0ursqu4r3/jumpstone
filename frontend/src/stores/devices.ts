import { defineStore } from 'pinia'
import { ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import type { CurrentUserDevice } from '~/types/session'

export interface SessionDeviceRecord {
  deviceId: string
  deviceName?: string | null
  lastSeenAt?: string | null
  ipAddress?: string | null
  userAgent?: string | null
}

const mockDevices: SessionDeviceRecord[] = [
  {
    deviceId: 'relay-devkit',
    deviceName: 'DevKit (mock)',
    lastSeenAt: new Date(Date.now() - 60 * 60 * 1000).toISOString(),
    ipAddress: '10.0.0.42',
    userAgent: 'Rusty-Link/0.1 (mock)',
  },
]

const normalizeDevice = (device: CurrentUserDevice): SessionDeviceRecord | null => {
  if (!device || typeof device.device_id !== 'string' || !device.device_id.trim().length) {
    return null
  }
  return {
    deviceId: device.device_id,
    deviceName: typeof device.device_name === 'string' ? device.device_name : null,
    lastSeenAt: typeof device.last_seen_at === 'string' ? device.last_seen_at : null,
    ipAddress: typeof device.ip_address === 'string' ? device.ip_address : null,
    userAgent: typeof device.user_agent === 'string' ? device.user_agent : null,
  }
}

export const useSessionDevicesStore = defineStore('session-devices', () => {
  const devices = ref<SessionDeviceRecord[]>([])
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
        .map((entry) => normalizeDevice(entry))
        .filter((entry): entry is SessionDeviceRecord => Boolean(entry))
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
