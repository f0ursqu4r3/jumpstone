export interface RuntimeConfig {
  public: {
    apiBaseUrl: string
  }
}

const sanitizeExplicitBase = (raw?: string | null): string | null => {
  if (!raw) {
    return null
  }
  const trimmed = raw.trim().replace(/\/+$/, '')
  if (!trimmed.length) {
    return null
  }
  if (/^https?:\/\//i.test(trimmed)) {
    return trimmed
  }
  if (trimmed.startsWith('//')) {
    return `https:${trimmed}`
  }
  if (trimmed.startsWith('/')) {
    return trimmed
  }

  // Treat bare hostnames like "api.example.com" as HTTPS URLs.
  return `https://${trimmed}`
}

export const getRuntimeConfig = (): RuntimeConfig => {
  const explicitBase = sanitizeExplicitBase(import.meta.env.VITE_API_BASE_URL)

  let apiBaseUrl: string

  if (explicitBase && explicitBase.length) {
    apiBaseUrl = explicitBase
  } else if (import.meta.env.DEV) {
    apiBaseUrl = '/api'
  } else {
    apiBaseUrl = 'http://127.0.0.1:8080/api'
  }

  return {
    public: {
      apiBaseUrl,
    },
  }
}
