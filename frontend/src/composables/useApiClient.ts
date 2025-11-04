import { ofetch } from 'ofetch'
import { useSessionStore } from '~/stores/session'
import { getRuntimeConfig } from '@/config/runtime'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

const normalizeHeaders = (headers?: HeadersInit): Record<string, string> => {
  if (!headers) {
    return {}
  }

  if (headers instanceof Headers) {
    return Object.fromEntries(headers.entries())
  }

  if (Array.isArray(headers)) {
    return Object.fromEntries(headers)
  }

  return { ...headers }
}

const createRequestId = () => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }

  return Math.random().toString(16).slice(2)
}

type SessionStore = ReturnType<typeof useSessionStore>
type ApiClient = ReturnType<typeof ofetch.create>

export const createApiClient = (
  sessionAccessor: () => SessionStore,
): ApiClient => {
  const runtimeConfig = getRuntimeConfig()
  const baseURL = runtimeConfig.public.apiBaseUrl

  const client = ofetch.create({
    baseURL,
    retry: 0,
    async onRequest({ options }) {
      const headers = normalizeHeaders(options.headers)
      const session = sessionAccessor()

      if (session.isAuthenticated) {
        await session.ensureFreshAccessToken()
      }

      const token = session.accessToken

      if (token && !headers.authorization) {
        headers.authorization = `Bearer ${token}`
      }

      if (session.deviceId && !headers['x-device-id']) {
        headers['x-device-id'] = session.deviceId
      }

      if (!headers['x-request-id']) {
        headers['x-request-id'] = createRequestId()
      }

      if (!headers.accept) {
        headers.accept = 'application/json'
      }

      options.headers = new Headers(headers)
    },
    onResponseError({ response, options }) {
      const requestHeaders =
        options.headers instanceof Headers ? options.headers : new Headers(options.headers ?? {})
      const requestId =
        requestHeaders.get('x-request-id') ?? response.headers.get('x-request-id') ?? undefined
      const method = (options.method ?? 'GET').toUpperCase()
      const url = response.url

      console.error('API request failed', {
        status: response.status,
        statusText: response.statusText,
        body: response._data,
        requestId,
      })

      recordNetworkBreadcrumb('api', {
        message: `${method} ${url} failed`,
        level: 'error',
        data: {
          status: response.status,
          statusText: response.statusText,
          requestId,
          method,
          url,
        },
      })
    },
  })

  return client
}

export const useApiClient = () =>
  createApiClient(() => useSessionStore())
