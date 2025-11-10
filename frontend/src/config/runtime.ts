export interface RuntimeConfig {
  public: {
    apiBaseUrl: string
  }
}

export const getRuntimeConfig = (): RuntimeConfig => {
  const explicitBase = import.meta.env.VITE_API_BASE_URL?.toString().trim()

  let apiBaseUrl: string

  if (explicitBase && explicitBase.length) {
    apiBaseUrl = explicitBase.replace(/\/+$/, '')
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
