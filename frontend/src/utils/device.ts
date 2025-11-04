export const createDefaultDeviceId = () => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `browser-${crypto.randomUUID().slice(0, 8)}`
  }

  return `browser-${Math.random().toString(36).slice(2, 8)}`
}
