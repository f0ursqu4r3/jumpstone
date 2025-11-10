import { defineStore } from 'pinia'
import { ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { extractErrorMessage } from '@/utils/errors'
import { recordNetworkBreadcrumb } from '@/utils/telemetry'

export interface ServerReaction {
  emoji: string
  count: number
  reactors?: string[]
}

export interface ReactionSummary {
  emoji: string
  count: number
  reacted: boolean
}

interface ToggleReactionOptions {
  channelId: string
  eventId: string
  emoji: string
  currentlyReacted: boolean
}

interface LocalOverride {
  reacted: boolean
}

type ReactionOverrides = Record<string, Record<string, Record<string, LocalOverride>>>

const COMMON_REACTIONS = ['👍', '🎉', '🔥', '❤️', '🚀', '👀'] as const

const sanitizeEmoji = (value: string) => value.trim() || '👍'

const resolveBaseReactions = (
  serverReactions: ServerReaction[],
  currentUserId: string | null | undefined,
): { order: string[]; map: Map<string, ReactionSummary> } => {
  const order: string[] = []
  const map = new Map<string, ReactionSummary>()
  serverReactions.forEach((reaction) => {
    if (!reaction || typeof reaction.emoji !== 'string') {
      return
    }
    const emoji = reaction.emoji
    const count = Number(reaction.count ?? 0)
    const reactors = Array.isArray(reaction.reactors) ? reaction.reactors : []
    const reacted =
      typeof currentUserId === 'string' && currentUserId.length
        ? reactors.includes(currentUserId)
        : false
    map.set(emoji, {
      emoji,
      count: Number.isFinite(count) && count > 0 ? count : 0,
      reacted,
    })
    order.push(emoji)
  })
  return { order, map }
}

export const useReactionStore = defineStore('reactions', () => {
  const overrides = ref<ReactionOverrides>({})
  const api = () => useApiClient()

  const applyOverride = (channelId: string, eventId: string, emoji: string, reacted: boolean) => {
    const existingChannel = overrides.value[channelId] ?? {}
    const existingEvent = existingChannel[eventId] ?? {}
    overrides.value = {
      ...overrides.value,
      [channelId]: {
        ...existingChannel,
        [eventId]: {
          ...existingEvent,
          [emoji]: { reacted },
        },
      },
    }
  }

  const overridesFor = (channelId: string | null | undefined, eventId: string | null | undefined) => {
    if (!channelId || !eventId) {
      return {}
    }
    return overrides.value[channelId]?.[eventId] ?? {}
  }

  const resolveReactions = (
    channelId: string | null | undefined,
    eventId: string | null | undefined,
    serverReactions: ServerReaction[] = [],
    currentUserId: string | null | undefined,
  ): ReactionSummary[] => {
    const { order, map } = resolveBaseReactions(serverReactions, currentUserId)
    const scopedOverrides = overridesFor(channelId, eventId)

    Object.entries(scopedOverrides).forEach(([emojiKey, override]) => {
      if (!map.has(emojiKey)) {
        map.set(emojiKey, {
          emoji: emojiKey,
          count: 0,
          reacted: false,
        })
        order.push(emojiKey)
      }
      const summary = map.get(emojiKey)
      if (!summary) {
        return
      }
      if (override.reacted && !summary.reacted) {
        summary.count += 1
      } else if (!override.reacted && summary.reacted && summary.count > 0) {
        summary.count -= 1
      }
      summary.reacted = override.reacted
      map.set(emojiKey, summary)
    })

    return order
      .map((emoji) => map.get(emoji))
      .filter((entry): entry is ReactionSummary => Boolean(entry && entry.count > 0))
  }

  const toggleReaction = async ({
    channelId,
    eventId,
    emoji,
    currentlyReacted,
  }: ToggleReactionOptions) => {
    if (!channelId || !eventId) {
      return
    }

    const normalizedEmoji = sanitizeEmoji(emoji)
    const nextReacted = !currentlyReacted
    applyOverride(channelId, eventId, normalizedEmoji, nextReacted)

    try {
      await api()(`/channels/${channelId}/events/${eventId}/reactions`, {
        method: 'POST',
        body: {
          emoji: normalizedEmoji,
          action: nextReacted ? 'add' : 'remove',
        },
      })
      recordNetworkBreadcrumb('api', {
        message: 'Reaction toggled',
        level: 'info',
        data: { channelId, eventId, emoji: normalizedEmoji, action: nextReacted ? 'add' : 'remove' },
      })
    } catch (err) {
      const status = (err as { response?: { status?: number } }).response?.status
      if (status === 404 || status === 501) {
        return
      }
      applyOverride(channelId, eventId, normalizedEmoji, currentlyReacted)
      recordNetworkBreadcrumb('api', {
        message: 'Reaction toggle failed',
        level: 'error',
        data: {
          channelId,
          eventId,
          emoji: normalizedEmoji,
          error: extractErrorMessage(err),
        },
      })
      throw err
    }
  }

  return {
    overrides,
    COMMON_REACTIONS,
    resolveReactions,
    toggleReaction,
  }
})
