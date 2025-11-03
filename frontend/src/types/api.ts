export interface ComponentStatus {
  name: string
  status: string
  details?: string | null
}

export interface ReadinessResponse {
  status: string
  uptime_seconds: number
  components: ComponentStatus[]
}

export interface VersionResponse {
  version: string
}

export interface BackendStatusPayload {
  ready: ReadinessResponse
  version: string
}
