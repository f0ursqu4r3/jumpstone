export interface RuntimeConfig {
  public: {
    apiBaseUrl: string
  }
}

export const getRuntimeConfig = (): RuntimeConfig => {
  const apiBaseUrl =
    import.meta.env.VITE_API_BASE_URL?.toString() ?? 'http://127.0.0.1:8080'

  return {
    public: {
      apiBaseUrl,
    },
  }
}
