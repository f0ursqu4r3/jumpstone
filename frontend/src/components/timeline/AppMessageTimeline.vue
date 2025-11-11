<script setup lang="ts">
import { computed, ref } from 'vue'
import { storeToRefs } from 'pinia'

import TimelineMessageCard from '@/components/timeline/TimelineMessageCard.vue'
import { useApiClient } from '@/composables/useApiClient'
import { useReactionStore, type ReactionSummary, type ServerReaction } from '@/stores/reactions'
import { useSessionStore } from '@/stores/session'
import { useTimelineStore, type TimelineEntry, type TimelineStatus } from '@/stores/timeline'
import type { TimelineMessage } from '@/types/messaging'
import { extractErrorMessage } from '@/utils/errors'
import type { GuildPermissionSnapshot } from '@/utils/permissions'

const props = defineProps<{
  channelId?: string | null
  channelName: string
  events: TimelineEntry[]
  loading?: boolean
  error?: string | null
  localOriginHost?: string | null
  remoteServers?: string[]
  currentUserId?: string | null
  currentUserRole?: string | null
  currentUserPermissions?: GuildPermissionSnapshot | null
}>()

const emit = defineEmits<{
  (event: 'refresh'): void
  (event: 'retry', localId: string): void
}>()

const sessionStore = useSessionStore()
const { profile: sessionProfile } = storeToRefs(sessionStore)

type DirectoryEntry = {
  displayName?: string
  username?: string
}

const normalizeId = (value?: string | null) => {
  if (!value || typeof value !== 'string') {
    return null
  }
  const trimmed = value.trim()
  return trimmed.length ? trimmed : null
}

const coerceDirectoryRecord = (hintId: string | null, value: unknown) => {
  if (!value || typeof value !== 'object') {
    return null
  }
  const base = value as {
    id?: string
    user_id?: string
    userId?: string
    display_name?: string
    displayName?: string
    username?: string
  }
  const resolvedId =
    normalizeId(base.user_id) ??
    normalizeId(base.userId) ??
    normalizeId(base.id) ??
    normalizeId(hintId)

  if (!resolvedId) {
    return null
  }

  return {
    id: resolvedId,
    displayName: normalizeId(base.display_name) ?? normalizeId(base.displayName) ?? undefined,
    username: normalizeId(base.username) ?? undefined,
  }
}

const buildDirectoryMap = (input: unknown) => {
  if (!input) {
    return null
  }

  const map = new Map<string, DirectoryEntry>()
  const addRecord = (record: { id: string; displayName?: string; username?: string }) => {
    const key = record.id.toLowerCase()
    map.set(key, {
      displayName: record.displayName ?? record.username,
      username: record.username,
    })
  }

  if (Array.isArray(input)) {
    input.forEach((entry) => {
      const record = coerceDirectoryRecord(null, entry)
      if (record) {
        addRecord(record)
      }
    })
  } else if (typeof input === 'object') {
    Object.entries(input as Record<string, unknown>).forEach(([key, value]) => {
      const record = coerceDirectoryRecord(key, value)
      if (record) {
        addRecord(record)
      }
    })
  }

  return map.size ? map : null
}

const userDirectory = computed(() => {
  const metadata = sessionProfile.value?.metadata
  if (!metadata || typeof metadata !== 'object') {
    return null
  }
  const source =
    (metadata as { users?: unknown }).users ?? (metadata as { roster?: unknown }).roster ?? null
  return buildDirectoryMap(source ?? null)
})

const resolveSenderName = (senderId: string | undefined | null) => {
  if (!senderId) {
    return null
  }
  const directory = userDirectory.value
  if (!directory) {
    return null
  }
  const key = senderId.toLowerCase()
  const entry = directory.get(key)
  if (!entry) {
    return null
  }
  return entry.displayName ?? entry.username ?? null
}

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

const baseItemClass = 'relative flex gap-4 rounded-lg px-3 py-2 transition duration-150 ease-out'

const computeItemClasses = (message: { optimistic: boolean; status?: TimelineStatus }) => {
  if (!message.optimistic) {
    return [baseItemClass, 'bg-transparent hover:bg-white/5']
  }

  if (message.status === 'failed') {
    return [baseItemClass, 'bg-rose-500/10']
  }

  if (message.status === 'queued') {
    return [baseItemClass, 'bg-amber-500/10']
  }

  if (message.status === 'sent') {
    return [baseItemClass, 'bg-emerald-500/10']
  }

  return [baseItemClass, 'bg-sky-500/5']
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
    items: Array<
      TimelineMessage & {
        status?: TimelineStatus | undefined
        statusMessage: string | null
        statusMeta: ReturnType<typeof statusDescriptor>
      }
    >
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

    const senderId = typeof entry.event.sender === 'string' ? entry.event.sender : ''
    const senderName = resolveSenderName(senderId) ?? senderId
    const senderLabel = senderName && senderName.length ? senderName : 'Unknown user'

    const record: TimelineMessage & {
      status?: TimelineStatus | undefined
      statusMessage: string | null
      statusMeta: ReturnType<typeof statusDescriptor>
    } = {
      id: entry.localId ?? `${entry.channel_id}-${entry.sequence}`,
      localId: entry.localId,
      senderId,
      sender: senderLabel,
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
  <div class="flex h-full flex-col gap-2">
    <div v-if="loading && !hasEvents" class="flex-1 overflow-y-auto space-y-2 pr-1">
      <div v-for="index in 6" :key="index" class="flex gap-3">
        <USkeleton class="h-10 w-10 rounded-full" />
        <div class="flex-1 space-y-2">
          <USkeleton class="h-3 w-1/3 rounded" />
          <USkeleton class="h-3 w-full rounded" />
          <USkeleton class="h-3 w-2/3 rounded" />
        </div>
      </div>
    </div>

    <div v-else-if="error" class="flex-1 overflow-y-auto pr-1">
      <UAlert
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
    </div>

    <div
      v-else-if="hasEvents"
      class="flex-1 overflow-y-auto rounded-lg border border-white/5 bg-slate-950/50 p-2 shadow-inner shadow-slate-950/40"
    >
      <div class="space-y-10">
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
                <TimelineMessageCard
                  :message="message"
                  :reaction-palette="reactionPalette"
                  :is-editing="isEditingMessage(message.id)"
                  :edit-draft="editDraft"
                  :edit-original="editOriginal"
                  :edit-has-changes="editHasChanges"
                  :edit-saving="editSaving"
                  :edit-error="editError"
                  :reaction-button-classes="reactionButtonClasses"
                  :can-edit-message="canEditMessage"
                  :can-report-message="canReportMessage"
                  @retry="(localId) => emit('retry', localId)"
                  @edit="beginEdit(message)"
                  @cancel-edit="cancelEdit"
                  @save-edit="handleEditSave(message)"
                  @update:editDraft="(value) => (editDraft = value)"
                  @toggle-reaction="
                    (payload) =>
                      handleReactionToggle(message, payload.emoji, payload.currentlyReacted)
                  "
                  @select-reaction="(emoji) => handleReactionPaletteSelect(message, emoji)"
                  @copy-meta="() => copyMetadata({ id: message.id, origin: message.originServer })"
                  @report="() => handleReportMessage(message)"
                />
              </div>
            </li>
          </ul>
        </div>
      </div>
    </div>

    <div
      v-else
      class="flex flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-white/10 bg-slate-950/40 p-2 text-center"
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
