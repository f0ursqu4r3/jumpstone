export interface GuildRecord {
  guild_id: string
  name: string
  created_at: string
}

export interface ChannelRecord {
  channel_id: string
  guild_id: string
  name: string
  created_at: string
}

export interface TimelineEventPayload {
  schema_version: number
  event_id: string
  event_type: string
  room_id: string
  sender: string
  origin_server: string | null
  origin_ts: number
  content: Record<string, unknown>
  prev_events?: unknown[]
  auth_events?: unknown[]
  signatures?: Record<string, unknown>
}

export interface ChannelEventEnvelope {
  sequence: number
  channel_id: string
  event: TimelineEventPayload
}
