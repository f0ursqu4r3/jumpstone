<script setup lang="ts">
import { computed } from 'vue'

import type { ChannelEventEnvelope } from '@/types/messaging'

const props = defineProps<{
  channelName: string
  events: ChannelEventEnvelope[]
  loading?: boolean
  error?: string | null
}>()

const emit = defineEmits<{
  (event: 'refresh'): void
}>()

const resolveContent = (event: ChannelEventEnvelope) => {
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

const toDate = (event: ChannelEventEnvelope) => {
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

const groupedEvents = computed(() => {
  const groups: Array<{
    date: string
    items: Array<{
      id: string
      sender: string
      time: string
      content: string
      eventType: string
    }>
  }> = []

  const sorted = [...props.events].sort((a, b) => a.sequence - b.sequence)

  sorted.forEach((entry) => {
    const occurredAt = toDate(entry)
    const dateLabel = formatDateLabel(occurredAt)
    const timeLabel = formatTimeLabel(occurredAt)
    const latestGroup = groups[groups.length - 1]

    const record = {
      id: `${entry.channel_id}-${entry.sequence}`,
      sender: entry.event.sender,
      time: timeLabel,
      content: resolveContent(entry),
      eventType: entry.event.event_type,
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
            class="h-px flex-1 bg-gradient-to-r from-transparent via-slate-700/50 to-transparent"
          />
          <span class="text-xs font-semibold uppercase tracking-wide text-slate-500">
            {{ group.date }}
          </span>
          <div
            class="h-px flex-1 bg-gradient-to-r from-transparent via-slate-700/50 to-transparent"
          />
        </div>

        <ul class="space-y-6">
          <li
            v-for="message in group.items"
            :key="message.id"
            class="relative flex gap-4 rounded-2xl border border-transparent bg-white/5 p-4 transition hover:border-sky-500/20 hover:bg-sky-500/5"
          >
            <div
              class="flex size-10 shrink-0 items-center justify-center rounded-full bg-slate-900 text-sm font-semibold uppercase text-slate-200"
            >
              {{ message.sender.slice(0, 2) }}
            </div>
            <div class="flex-1 space-y-2">
              <div class="flex flex-wrap items-center gap-2">
                <p class="text-sm font-semibold text-white">
                  {{ message.sender }}
                </p>
                <UBadge size="xs" variant="soft" color="neutral" :label="message.eventType" />
                <span class="text-xs text-slate-500">{{ message.time }}</span>
              </div>
              <p class="text-sm text-slate-200 whitespace-pre-line break-words">
                {{ message.content }}
              </p>
              <p
                v-if="message.eventType !== 'message'"
                class="text-xs text-slate-500"
              >
                System event placeholder — richer rendering lands in Week 5.
              </p>
              <div class="flex items-center gap-2 text-xs text-slate-500">
                <UIcon name="i-heroicons-face-smile" class="h-4 w-4" />
                <span>Reactions placeholder · emoji + counts coming soon</span>
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
