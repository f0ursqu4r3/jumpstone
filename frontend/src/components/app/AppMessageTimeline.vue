<script setup lang="ts">
import { computed, ref } from 'vue'

import { useApiClient } from '@/composables/useApiClient'
import { useReactionStore, type ReactionSummary, type ServerReaction } from '@/stores/reactions'
import { useTimelineStore, type TimelineEntry, type TimelineStatus } from '@/stores/timeline'
import { extractErrorMessage } from '@/utils/errors'

const props = defineProps<{
  channelId?: string | null
  channelName: string
  events: TimelineEntry[]
  loading?: boolean
  error?: string | null
  localOriginHost?: string | null
  remoteServers?: string[]
  currentUserId?: string | null
}>()

const emit = defineEmits<{
  (event: 'refresh'): void
  (event: 'retry', localId: string): void
}>()

const resolveContent = (event: TimelineEntry) => {
  const payload = event.event.content
  if (!payload || typeof payload !== 'object') {
    return ''
  }

  const raw = (payload as { content?: unknown; body?: unknown }).content
  if (typeof raw === 'string') {
    return raw
  }

  if (typeof (payload as { body?: unknown }).body === 'string') {
    return (payload as { body?: string }).body ?? ''
  }

  if ('text' in payload && typeof (payload as { text?: unknown }).text === 'string') {
    return (payload as { text: string }).text
  }

  try {
    return JSON.stringify(payload)
  } catch {
    return '[Unsupported payload]'
  }
}

const toDate = (event: TimelineEntry) => {
  const epoch = event.event.origin_ts
  if (typeof epoch === 'number' && Number.isFinite(epoch)) {
    return new Date(epoch)
  }
  return new Date(0)
}

const formatDateLabel = (value: Date) => {
  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'full',
    }).format(value)
  } catch {
    return value.toDateString()
  }
}

const formatTimeLabel = (value: Date) => {
  try {
    return new Intl.DateTimeFormat(undefined, {
      timeStyle: 'short',
    }).format(value)
  } catch {
    return value.toLocaleTimeString()
  }
}

const statusDescriptor = (status?: TimelineStatus | null) => {
  switch (status) {
    case 'pending':
      return {
        icon: 'i-heroicons-arrow-path',
        label: 'Sending…',
        color: 'text-sky-400',
        spin: true,
      }
    case 'queued':
      return {
        icon: 'i-heroicons-cloud-arrow-up',
        label: 'Queued',
        color: 'text-amber-300',
        spin: false,
      }
    case 'failed':
      return {
        icon: 'i-heroicons-exclamation-triangle',
        label: 'Delivery failed',
        color: 'text-rose-400',
        spin: false,
      }
    case 'sent':
      return {
        icon: 'i-heroicons-check-circle',
        label: 'Sent',
        color: 'text-emerald-400',
        spin: false,
      }
    default:
      return {
        icon: 'i-heroicons-clock',
        label: 'Pending',
        color: 'text-slate-400',
        spin: false,
      }
  }
}

const baseItemClass = 'relative flex gap-4 rounded-2xl border p-4 transition duration-200 ease-out'

const computeItemClasses = (message: { optimistic: boolean; status?: TimelineStatus }) => {
  if (!message.optimistic) {
    return [
      baseItemClass,
      'border-transparent bg-white/5 hover:border-sky-500/20 hover:bg-sky-500/5',
    ]
  }

  if (message.status === 'failed') {
    return [baseItemClass, 'border-rose-500/40 bg-rose-500/10']
  }

  if (message.status === 'queued') {
    return [baseItemClass, 'border-amber-300/40 bg-amber-500/10']
  }

  if (message.status === 'sent') {
    return [baseItemClass, 'border-emerald-400/40 bg-emerald-500/10']
  }

  return [baseItemClass, 'border-sky-500/30 bg-sky-500/10']
}

const reactionStore = useReactionStore()
const reactionPalette = reactionStore.COMMON_REACTIONS
const timelineStore = useTimelineStore()
const api = useApiClient()

const editingMessageId = ref<string | null>(null)
const editDraft = ref('')
const editOriginal = ref('')
const editSaving = ref(false)
const editError = ref<string | null>(null)

const normalizedLocalHost = computed(() => {
  if (!props.localOriginHost) {
    return null
  }
  return props.localOriginHost.toLowerCase()
})

const isRemoteOrigin = (origin?: string | null) => {
  if (!origin) {
    return false
  }
  const normalized = origin.toLowerCase()
  const localHost = normalizedLocalHost.value
  if (localHost && normalized === localHost) {
    return false
  }
  return true
}

const extractServerReactions = (
  content: Record<string, unknown> | undefined | null,
): ServerReaction[] => {
  if (!content || typeof content !== 'object') {
    return []
  }
  const raw = (content as { reactions?: unknown }).reactions
  if (!Array.isArray(raw)) {
    return []
  }
  return raw
    .map((entry) => {
      if (!entry || typeof entry !== 'object') {
        return null
      }
      const emoji =
        typeof (entry as { emoji?: string }).emoji === 'string'
          ? (entry as { emoji?: string }).emoji
          : null
      if (!emoji) {
        return null
      }
      const countValue = Number((entry as { count?: number }).count ?? 0)
      const reactorsRaw = (entry as { reactors?: unknown }).reactors
      const reactors = Array.isArray(reactorsRaw)
        ? reactorsRaw.filter((value): value is string => typeof value === 'string')
        : undefined
      return {
        emoji,
        count: Number.isFinite(countValue) ? countValue : 0,
        reactors,
      } as ServerReaction
    })
    .filter((entry): entry is ServerReaction => entry !== null)
}

const groupedEvents = computed(() => {
  const groups: Array<{
    date: string
    items: Array<{
      id: string
      localId?: string
      sender: string
      time: string
      content: string
      eventType: string
      originServer: string | null
      remote: boolean
      optimistic: boolean
      status: TimelineStatus | undefined
      statusMessage: string | null
      statusMeta: ReturnType<typeof statusDescriptor>
      reactions: ReactionSummary[]
      eventId: string | null
      channelId: string | null
    }>
  }> = []

  const sorted = [...props.events]

  sorted.forEach((entry) => {
    const occurredAt = toDate(entry)
    const dateLabel = formatDateLabel(occurredAt)
    const timeLabel = formatTimeLabel(occurredAt)
    const latestGroup = groups[groups.length - 1]

    const originServer =
      typeof entry.event.origin_server === 'string' ? entry.event.origin_server : null
    const serverReactions = extractServerReactions(entry.event.content as Record<string, unknown>)
    const reactions = reactionStore.resolveReactions(
      entry.channel_id,
      entry.event.event_id,
      serverReactions,
      props.currentUserId ?? null,
    )

    const isAuthor =
      typeof props.currentUserId === 'string' &&
      props.currentUserId.length > 0 &&
      entry.event.sender === props.currentUserId

    const record = {
      id: entry.localId ?? `${entry.channel_id}-${entry.sequence}`,
      localId: entry.localId,
      sender: entry.event.sender,
      time: timeLabel,
      content: resolveContent(entry),
      eventType: entry.event.event_type,
      originServer,
      remote: isRemoteOrigin(originServer),
      optimistic: Boolean(entry.optimistic),
      status: entry.status,
      statusMessage: entry.statusMessage ?? null,
      statusMeta: statusDescriptor(entry.status),
      reactions,
      eventId: entry.event.event_id ?? null,
      channelId: entry.channel_id ?? null,
      isAuthor,
    }

    if (!latestGroup || latestGroup.date !== dateLabel) {
      groups.push({
        date: dateLabel,
        items: [record],
      })
      return
    }

    latestGroup.items.push(record)
  })

  return groups
})

const hasEvents = computed(() => groupedEvents.value.length > 0)

const describeMessage = (message: {
  sender: string
  time: string
  content: string
  eventType: string
}) => {
  const content = message.content.trim()
  const summary = content.length
    ? content.length > 80
      ? `${content.slice(0, 77)}…`
      : content
    : message.eventType
  return `${message.sender} at ${message.time}: ${summary}`
}

const editHasChanges = computed(() => {
  if (!editingMessageId.value) {
    return false
  }
  return editDraft.value.trim() !== editOriginal.value.trim()
})

const reactionButtonClasses = (active: boolean) => [
  'flex items-center gap-1 rounded-2xl border px-2 py-1 text-xs font-medium transition',
  active
    ? 'border-sky-400/50 bg-sky-400/10 text-sky-100'
    : 'border-white/10 bg-white/5 text-slate-300 hover:border-sky-400/30 hover:bg-slate-800/40',
]

const isEditingMessage = (messageId: string) => editingMessageId.value === messageId

const beginEdit = (message: { id: string; content: string; isAuthor: boolean }) => {
  if (!message.isAuthor) {
    return
  }
  editingMessageId.value = message.id
  editDraft.value = message.content
  editOriginal.value = message.content
  editError.value = null
}

const cancelEdit = () => {
  editingMessageId.value = null
  editDraft.value = ''
  editOriginal.value = ''
  editError.value = null
}

const handleEditSave = async (message: {
  id: string
  channelId: string | null
  eventId: string | null
}) => {
  if (!message.channelId || !message.eventId) {
    editError.value = 'Unable to determine message target.'
    return
  }

  const trimmed = editDraft.value.trim()
  if (!trimmed.length) {
    editError.value = 'Message cannot be empty.'
    return
  }

  editSaving.value = true
  editError.value = null

  try {
    await api(`/channels/${message.channelId}/events/${message.eventId}`, {
      method: 'PATCH',
      body: JSON.stringify({ content: trimmed }),
      headers: {
        'content-type': 'application/json',
      },
    })
  } catch (err) {
    const status = (err as { response?: { status?: number } }).response?.status
    if (status !== 404 && status !== 501) {
      editError.value = extractErrorMessage(err) || 'Unable to save changes.'
      return
    }
  } finally {
    editSaving.value = false
  }

  timelineStore.updateEventContent(message.channelId, message.eventId, trimmed)
  editingMessageId.value = null
  editDraft.value = ''
  editOriginal.value = ''
}

const messageActionsAvailable = () => true
const canEditMessage = (message: { isAuthor: boolean; optimistic: boolean }) =>
  message.isAuthor && !message.optimistic
const canReportMessage = () => true

const handleReportMessage = (message: {
  channelId: string | null
  eventId: string | null
  sender: string
}) => {
  console.info('Report message placeholder', {
    channelId: message.channelId,
    eventId: message.eventId,
    sender: message.sender,
  })
}

const handleReactionToggle = (
  message: {
    channelId: string | null
    eventId: string | null
  },
  emoji: string,
  currentlyReacted: boolean,
) => {
  if (!message.channelId || !message.eventId) {
    return
  }
  reactionStore
    .toggleReaction({
      channelId: message.channelId,
      eventId: message.eventId,
      emoji,
      currentlyReacted,
    })
    .catch((err) => {
      console.warn('Failed to toggle reaction', err)
    })
}

const handleReactionPaletteSelect = (
  message: { channelId: string | null; eventId: string | null; reactions: ReactionSummary[] },
  emoji: string,
) => {
  const existing = message.reactions.find((reaction) => reaction.emoji === emoji)
  handleReactionToggle(message, emoji, existing?.reacted ?? false)
}

const copyMetadata = async (payload: { id: string; origin?: string | null }) => {
  const content = JSON.stringify(payload, null, 2)
  try {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(content)
      return
    }
  } catch (err) {
    console.warn('Failed to copy event metadata', err)
  }
  const textarea = document.createElement('textarea')
  textarea.value = content
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'absolute'
  textarea.style.left = '-9999px'
  document.body.appendChild(textarea)
  textarea.select()
  try {
    document.execCommand('copy')
  } catch (err) {
    console.warn('Fallback copy failed', err)
  }
  document.body.removeChild(textarea)
}
</script>

<template>
  <div class="space-y-6">
    <header class="flex items-start justify-between gap-4">
      <div>
        <p class="text-sm font-semibold text-slate-400">Channel timeline</p>
        <h2 class="text-2xl font-semibold text-white">#{{ channelName || 'select-a-channel' }}</h2>
      </div>
      <UButton
        icon="i-heroicons-arrow-path"
        color="neutral"
        variant="ghost"
        :loading="loading"
        @click="emit('refresh')"
        aria-label="Refresh timeline"
      />
    </header>

    <div v-if="loading && !hasEvents" class="space-y-4">
      <div v-for="index in 6" :key="index" class="flex gap-3">
        <USkeleton class="h-10 w-10 rounded-full" />
        <div class="flex-1 space-y-2">
          <USkeleton class="h-3 w-1/3 rounded" />
          <USkeleton class="h-3 w-full rounded" />
          <USkeleton class="h-3 w-2/3 rounded" />
        </div>
      </div>
    </div>

    <UAlert
      v-else-if="error"
      color="warning"
      variant="soft"
      icon="i-heroicons-chat-bubble-left-ellipsis"
      :description="error"
      title="Timeline unavailable"
    >
      <template #actions>
        <UButton size="xs" variant="ghost" color="neutral" @click="emit('refresh')">
          Retry
        </UButton>
      </template>
    </UAlert>

    <div
      v-else-if="hasEvents"
      class="space-y-10 rounded-3xl border border-white/5 bg-slate-950/50 p-6 shadow-inner shadow-slate-950/40"
    >
      <div v-for="group in groupedEvents" :key="group.date" class="space-y-4">
        <div class="flex items-center gap-3">
          <div
            class="h-px flex-1 bg-linear-to-r from-transparent via-slate-700/50 to-transparent"
          />
          <span class="text-xs font-semibold uppercase tracking-wide text-slate-500">
            {{ group.date }}
          </span>
          <div
            class="h-px flex-1 bg-linear-to-r from-transparent via-slate-700/50 to-transparent"
          />
        </div>

        <ul class="space-y-6" role="list">
          <li
            v-for="message in group.items"
            :key="message.id"
            :class="computeItemClasses(message)"
            role="listitem"
            tabindex="0"
            :aria-label="describeMessage(message)"
          >
            <div
              class="flex size-10 shrink-0 items-center justify-center rounded-full bg-slate-900 text-sm font-semibold uppercase text-slate-200"
            >
              {{ message.sender.slice(0, 2) }}
            </div>
            <div class="flex-1 space-y-2">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="flex flex-wrap items-center gap-2">
                  <p class="text-sm font-semibold text-white">
                    {{ message.sender }}
                  </p>
                  <UBadge
                    v-if="message.eventType !== 'message'"
                    size="xs"
                    variant="soft"
                    color="neutral"
                    :label="message.eventType"
                  />
                  <UBadge
                    v-else-if="message.optimistic"
                    size="xs"
                    variant="soft"
                    color="info"
                    label="Optimistic"
                  />
                  <span class="text-xs text-slate-500">{{ message.time }}</span>
                  <UBadge
                    v-if="message.originServer"
                    size="xs"
                    :color="message.remote ? 'warning' : 'neutral'"
                    variant="soft"
                    :label="message.remote ? `Remote · ${message.originServer}` : 'Local origin'"
                  />
                </div>
                <UPopover v-if="messageActionsAvailable(message)">
                  <UButton
                    size="xs"
                    variant="ghost"
                    color="neutral"
                    icon="i-heroicons-ellipsis-vertical"
                    aria-label="Message actions"
                  />
                  <template #content="{ close }">
                    <div class="w-48 space-y-1 p-2 text-sm">
                      <button
                        type="button"
                        class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-slate-200 hover:bg-white/5 disabled:opacity-40"
                        :disabled="!canEditMessage(message)"
                        @click="(beginEdit(message), close())"
                      >
                        <span>Edit message</span>
                        <UIcon name="i-heroicons-pencil-square" class="h-4 w-4" />
                      </button>
                      <button
                        type="button"
                        class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-slate-200 hover:bg-white/5"
                        @click="
                          (copyMetadata({
                            id: message.eventId ?? message.id,
                            origin: message.originServer,
                          }),
                          close())
                        "
                      >
                        <span>Copy event ID</span>
                        <UIcon name="i-heroicons-clipboard" class="h-4 w-4" />
                      </button>
                      <button
                        v-if="canReportMessage()"
                        type="button"
                        class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-slate-200 hover:bg-white/5"
                        @click="(handleReportMessage(message), close())"
                      >
                        <span>Report message</span>
                        <UIcon name="i-heroicons-flag" class="h-4 w-4" />
                      </button>
                    </div>
                  </template>
                </UPopover>
              </div>
              <div v-if="isEditingMessage(message.id)" class="space-y-3">
                <textarea
                  v-model="editDraft"
                  class="w-full rounded-xl border border-white/10 bg-slate-900/60 p-3 text-sm text-white focus:border-sky-400 focus:outline-none focus:ring-0"
                  rows="3"
                />
                <div
                  class="grid gap-3 rounded-2xl border border-white/10 bg-slate-950/40 p-3 text-xs text-slate-200 sm:grid-cols-2"
                >
                  <div class="space-y-1">
                    <p class="font-semibold uppercase tracking-wide text-slate-500">Original</p>
                    <p
                      class="rounded-xl border border-white/5 bg-slate-900/60 p-2 text-sm text-slate-300 whitespace-pre-line wrap-break-word"
                    >
                      {{ editOriginal || '—' }}
                    </p>
                  </div>
                  <div class="space-y-1">
                    <p class="font-semibold uppercase tracking-wide text-slate-500">
                      Revised preview
                    </p>
                    <p
                      :class="[
                        'rounded-xl border p-2 text-sm whitespace-pre-line wrap-break-word',
                        editHasChanges
                          ? 'border-emerald-500/30 bg-emerald-500/5 text-emerald-100'
                          : 'border-white/5 bg-slate-900/60 text-slate-300',
                      ]"
                    >
                      {{ editDraft || '—' }}
                    </p>
                    <p
                      class="text-[11px]"
                      :class="editHasChanges ? 'text-emerald-300' : 'text-slate-500'"
                    >
                      {{
                        editHasChanges
                          ? 'Looks good—save to publish the update.'
                          : 'No changes yet.'
                      }}
                    </p>
                  </div>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                  <UButton
                    size="xs"
                    color="info"
                    :loading="editSaving"
                    @click="handleEditSave(message)"
                  >
                    Save
                  </UButton>
                  <UButton size="xs" variant="ghost" color="neutral" @click="cancelEdit">
                    Cancel
                  </UButton>
                  <span v-if="editError" class="text-xs text-rose-300">{{ editError }}</span>
                </div>
              </div>
              <p v-else class="text-sm text-slate-200 whitespace-pre-line wrap-break-word">
                {{ message.content }}
              </p>
              <div v-if="message.optimistic" class="flex flex-wrap items-center gap-2 text-xs">
                <UIcon
                  :name="message.statusMeta.icon"
                  :class="[
                    'h-4 w-4',
                    message.statusMeta.color,
                    message.statusMeta.spin ? 'animate-spin' : '',
                  ]"
                />
                <span :class="['font-semibold', message.statusMeta.color]">
                  {{ message.statusMeta.label }}
                </span>
                <span v-if="message.statusMessage" class="text-slate-400">
                  · {{ message.statusMessage }}
                </span>
                <UButton
                  v-if="message.status === 'failed' && message.localId"
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  @click="emit('retry', message.localId)"
                >
                  Retry
                </UButton>
              </div>
              <p v-else-if="message.eventType !== 'message'" class="text-xs text-slate-500">
                System event placeholder — richer rendering lands in Week 6.
              </p>
              <div class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
                <template v-if="message.reactions.length">
                  <button
                    v-for="reaction in message.reactions"
                    :key="reaction.emoji"
                    type="button"
                    :class="reactionButtonClasses(reaction.reacted)"
                    @click="handleReactionToggle(message, reaction.emoji, reaction.reacted)"
                  >
                    <span class="text-base leading-none">{{ reaction.emoji }}</span>
                    <span class="text-[10px]">{{ reaction.count }}</span>
                  </button>
                </template>
                <span v-else class="text-slate-500">No reactions yet</span>
                <UPopover>
                  <UButton
                    size="xs"
                    variant="ghost"
                    color="neutral"
                    icon="i-heroicons-face-smile"
                    aria-label="Add reaction"
                  >
                    React
                  </UButton>
                  <template #content="{ close }">
                    <div class="flex flex-wrap gap-2 p-3">
                      <UButton
                        v-for="emoji in reactionPalette"
                        :key="emoji"
                        size="xs"
                        variant="ghost"
                        color="neutral"
                        @click="(handleReactionPaletteSelect(message, emoji), close())"
                      >
                        {{ emoji }}
                      </UButton>
                    </div>
                  </template>
                </UPopover>
                <UButton
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  icon="i-heroicons-clipboard"
                  @click="copyMetadata({ id: message.id, origin: message.originServer })"
                >
                  Copy meta
                </UButton>
              </div>
            </div>
          </li>
        </ul>
      </div>
    </div>

    <div
      v-else
      class="flex flex-col items-center justify-center gap-3 rounded-3xl border border-dashed border-white/10 bg-slate-950/40 p-10 text-center"
    >
      <UIcon name="i-heroicons-chat-bubble-oval-left-ellipsis" class="h-10 w-10 text-slate-600" />
      <div class="space-y-1">
        <p class="text-sm font-semibold text-white">No messages yet</p>
        <p class="text-xs text-slate-500">
          Once the backend starts emitting events they will appear here.
        </p>
      </div>
      <UButton
        size="sm"
        variant="ghost"
        color="neutral"
        icon="i-heroicons-arrow-path"
        @click="emit('refresh')"
      >
        Refresh timeline
      </UButton>
    </div>
  </div>
</template>
