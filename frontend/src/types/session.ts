export interface LoginParameters {
  identifier: string
  secret: string
  deviceId: string
  deviceName?: string
}

export interface LoginRequestBody {
  identifier: string
  secret: string
  device: {
    device_id: string
    device_name?: string
  }
}

export interface LoginResponse {
  access_token: string
  access_expires_at: string
  refresh_token: string
  refresh_expires_at: string
}

export interface RefreshRequestBody {
  refresh_token: string
}

export interface ApiErrorDetail {
  field: string
  message: string
}

export interface ApiErrorResponse {
  error?: string
  message?: string
  details?: ApiErrorDetail[]
}

export interface CurrentUserDevice {
  device_id: string
  device_name?: string | null
  last_seen_at?: string | null
  ip_address?: string | null
  user_agent?: string | null
}

export interface CurrentUserGuild {
  guild_id: string
  name?: string | null
  role?: string | null
}

export interface CurrentUser {
  user_id: string
  username: string
  display_name?: string | null
  avatar_url?: string | null
  email?: string | null
  server_name?: string | null
  default_guild_id?: string | null
  timezone?: string | null
  locale?: string | null
  created_at?: string | null
  updated_at?: string | null
  devices?: CurrentUserDevice[]
  guilds?: CurrentUserGuild[]
  roles?: string[]
  metadata?: Record<string, unknown>
}
