export interface FeatureFlags {
  adminPanel: boolean
}

let cached: FeatureFlags | null = null

export const getFeatureFlags = (): FeatureFlags => {
  if (cached) {
    return cached
  }

  const adminPanelEnv = (import.meta.env.VITE_FEATURE_ADMIN_PANEL ?? '').toString().toLowerCase()
  const adminPanel = adminPanelEnv === 'true'

  cached = {
    adminPanel,
  }

  return cached
}
