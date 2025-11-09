export interface FeatureFlags {
  adminPanel: boolean
  mlsReadiness: boolean
}

let cached: FeatureFlags | null = null

const parseBooleanFlag = (value: unknown, fallback: boolean): boolean => {
  if (typeof value === 'boolean') {
    return value
  }

  if (value == null) {
    return fallback
  }

  const normalized = value.toString().trim().toLowerCase()
  if (['1', 'true', 'yes', 'on'].includes(normalized)) {
    return true
  }
  if (['0', 'false', 'no', 'off'].includes(normalized)) {
    return false
  }
  return fallback
}

export const getFeatureFlags = (): FeatureFlags => {
  if (cached) {
    return cached
  }

  const adminPanel = parseBooleanFlag(import.meta.env.VITE_FEATURE_ADMIN_PANEL, false)
  const mlsReadiness = parseBooleanFlag(import.meta.env.VITE_FEATURE_MLS_READINESS ?? 'true', true)

  cached = {
    adminPanel,
    mlsReadiness,
  }

  return cached
}
