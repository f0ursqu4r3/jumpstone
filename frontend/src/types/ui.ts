import type { RouteLocationRaw } from 'vue-router'

export interface OnboardingSlide {
  id: string
  title: string
  description: string
  ctaLabel: string
  href?: string
  to?: RouteLocationRaw
  eyebrow?: string
  icon?: string
}

export interface ChannelEntry {
  id: string
  label: string
  kind: 'text' | 'voice'
  icon?: string
  unread?: number
  description?: string
  active?: boolean
}

export interface GuildSummary {
  id: string
  name: string
  initials: string
  active?: boolean
  notificationCount?: number
}
