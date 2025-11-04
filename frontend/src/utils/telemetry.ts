type BreadcrumbLevel = 'info' | 'warning' | 'error'

interface BreadcrumbPayload {
  message: string
  category?: string
  level?: BreadcrumbLevel
  data?: Record<string, unknown>
}

const getSentry = () => (globalThis as unknown as { Sentry?: { addBreadcrumb?: Function; captureException?: Function } }).Sentry

export const recordBreadcrumb = (payload: BreadcrumbPayload) => {
  const sentry = getSentry()
  if (sentry?.addBreadcrumb) {
    sentry.addBreadcrumb({
      message: payload.message,
      category: payload.category,
      level: payload.level,
      data: payload.data,
      timestamp: Date.now() / 1000,
    })
  } else if (import.meta.env.DEV) {
    console.debug('[telemetry]', payload)
  }
}

export const recordNetworkBreadcrumb = (
  channel: 'api' | 'ws',
  payload: BreadcrumbPayload & { data?: Record<string, unknown> },
) => {
  recordBreadcrumb({
    ...payload,
    category: payload.category ?? `network.${channel}`,
  })
}

export const recordException = (error: unknown, context?: Record<string, unknown>) => {
  const sentry = getSentry()
  if (sentry?.captureException) {
    sentry.captureException(error, { extra: context })
  } else if (import.meta.env.DEV) {
    console.error('[telemetry:error]', error, context)
  }
}
