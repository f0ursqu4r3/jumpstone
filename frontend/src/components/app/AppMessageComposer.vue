<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { useConnectivityStore } from '@/stores/connectivity'
import { useMessageComposerStore } from '@/stores/messages'
import type { RealtimeStatus } from '@/stores/realtime'

const props = defineProps<{
  channelId: string | null
  channelName: string
  realtimeStatus: RealtimeStatus
  attemptingReconnect: boolean
  disabled?: boolean
}>()

const emit = defineEmits<{
  (event: 'typing', payload: { channelId: string | null; preview: string }): void
}>()

const messageStore = useMessageComposerStore()
const connectivityStore = useConnectivityStore()

const message = ref('')
const error = ref<string | null>(null)
const submitting = ref(false)
const textareaRef = ref<HTMLTextAreaElement | null>(null)

const queueCount = computed(() => messageStore.queuedCount)
const hasFailures = computed(() => messageStore.hasFailures)
const degradedMessage = computed(() => connectivityStore.degradedMessage)
const online = computed(() => connectivityStore.online)

const composerDisabled = computed(() => props.disabled || submitting.value || !props.channelId)
const sendDisabled = computed(() => {
  if (composerDisabled.value) {
    return true
  }
  return !message.value.trim().length
})

const connectionLabel = computed(() => {
  if (!props.channelId) {
    return 'Select a channel to start messaging'
  }

  if (!online.value) {
    return 'Offline · messages will queue until you reconnect'
  }

  if (props.attemptingReconnect) {
    return 'Reconnecting to realtime feed…'
  }

  switch (props.realtimeStatus) {
    case 'connected':
      return 'Live updates active'
    case 'connecting':
      return 'Connecting to realtime…'
    case 'paused':
      return 'Realtime paused (background)'
    case 'error':
      return 'Realtime disconnected · retrying'
    default:
      return 'Realtime idle'
  }
})

const connectionIcon = computed(() => {
  if (!online.value) {
    return 'i-heroicons-cloud-slash'
  }
  if (props.attemptingReconnect) {
    return 'i-heroicons-arrow-path'
  }
  switch (props.realtimeStatus) {
    case 'connected':
      return 'i-heroicons-sparkles'
    case 'paused':
      return 'i-heroicons-pause-circle'
    case 'error':
      return 'i-heroicons-exclamation-triangle'
    default:
      return 'i-heroicons-signal'
  }
})

const connectionColor = computed(() => {
  if (!online.value) {
    return 'text-rose-400'
  }
  if (props.attemptingReconnect || props.realtimeStatus === 'connecting') {
    return 'text-amber-300'
  }
  if (props.realtimeStatus === 'connected') {
    return 'text-emerald-400'
  }
  if (props.realtimeStatus === 'error') {
    return 'text-rose-300'
  }
  return 'text-slate-400'
})

const typingPreviewDelay = 250
let typingTimer: ReturnType<typeof setTimeout> | null = null

const emitTyping = (value: string) => {
  emit('typing', {
    channelId: props.channelId,
    preview: value.slice(0, 120),
  })
}

const scheduleTyping = (value: string) => {
  if (typingTimer) {
    clearTimeout(typingTimer)
  }
  typingTimer = setTimeout(() => {
    emitTyping(value)
    typingTimer = null
  }, typingPreviewDelay)
}

const resetTypingTimer = () => {
  if (typingTimer) {
    clearTimeout(typingTimer)
    typingTimer = null
  }
}

const handleSubmit = async () => {
  if (!props.channelId || sendDisabled.value) {
    return
  }

  submitting.value = true
  error.value = null

  try {
    const result = await messageStore.sendMessage(props.channelId, message.value)
    if (result.ok || result.queued) {
      message.value = ''
      emitTyping('')
      if (textareaRef.value) {
        textareaRef.value.style.height = 'auto'
      }
    } else if (result.error) {
      error.value = result.error
    }
  } catch (err) {
    if (err instanceof Error) {
      error.value = err.message
    } else {
      error.value = 'Failed to send message.'
    }
  } finally {
    submitting.value = false
  }
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    handleSubmit()
  }
}

const updateAutosize = () => {
  const el = textareaRef.value
  if (!el) {
    return
  }
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 280)}px`
}

watch(message, (value) => {
  scheduleTyping(value)
  updateAutosize()
})

watch(
  () => props.channelId,
  () => {
    message.value = ''
    error.value = null
    resetTypingTimer()
    emitTyping('')
    updateAutosize()
  },
)

onBeforeUnmount(() => {
  resetTypingTimer()
})

const retryFailures = () => {
  messageStore.flushQueue().catch((err) => {
    console.warn('Failed to flush queued messages', err)
  })
}
</script>

<template>
  <div class="space-y-3 rounded-3xl border border-white/5 bg-slate-950/70 p-4 shadow-inner shadow-slate-950/40">
    <label class="flex flex-col gap-3 text-sm text-slate-200">
      <span class="flex items-center justify-between">
        <span class="font-semibold">
          Message #{{ channelName || 'channel' }}
        </span>
        <span class="text-xs text-slate-500">Shift + Enter for newline</span>
      </span>
      <textarea
        ref="textareaRef"
        v-model="message"
        :disabled="composerDisabled"
        class="min-h-[3.5rem] max-h-72 w-full resize-none rounded-2xl border border-white/10 bg-slate-900/80 px-4 py-3 text-sm text-white placeholder:text-slate-500 focus:border-sky-400 focus:outline-none focus:ring-2 focus:ring-sky-500/30 disabled:cursor-not-allowed disabled:opacity-60"
        :placeholder="
          channelId
            ? `Message #${channelName || 'channel'} (Enter to send)`
            : 'Select a channel to start messaging'
        "
        @keydown="handleKeydown"
      />
    </label>

    <div class="flex flex-wrap items-center justify-between gap-3 text-xs text-slate-400">
      <div class="flex items-center gap-2">
        <UButton
          icon="i-heroicons-face-smile"
          color="neutral"
          variant="ghost"
          size="sm"
          :disabled="composerDisabled"
        />
        <UTooltip text="File uploads coming soon">
          <UButton
            icon="i-heroicons-paper-clip"
            color="neutral"
            variant="ghost"
            size="sm"
            disabled
          />
        </UTooltip>
        <UTooltip text="Slash commands arriving with Week 6">
          <UButton icon="i-heroicons-command-line" color="neutral" variant="ghost" size="sm" disabled />
        </UTooltip>
      </div>

      <div class="flex items-center gap-2">
        <span v-if="queueCount" class="flex items-center gap-1 text-amber-200">
          <UIcon name="i-heroicons-arrow-up-tray" class="h-4 w-4" />
          {{ queueCount }} queued
        </span>
        <UButton
          color="info"
          :disabled="sendDisabled"
          :loading="submitting"
          @click="handleSubmit"
        >
          Send
        </UButton>
      </div>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-3 text-[11px] text-slate-500">
      <div class="flex items-center gap-2">
        <UIcon :name="connectionIcon" :class="['h-4 w-4', connectionColor]" />
        <span>{{ connectionLabel }}</span>
      </div>
      <div class="flex items-center gap-2">
        <span v-if="degradedMessage" class="text-amber-200">
          {{ degradedMessage }}
        </span>
        <UButton
          v-if="hasFailures"
          size="xs"
          color="neutral"
          variant="ghost"
          @click="retryFailures"
        >
          Retry queued
        </UButton>
      </div>
    </div>

    <UAlert
      v-if="error"
      color="warning"
      variant="soft"
      icon="i-heroicons-exclamation-circle"
      :description="error"
      title="Message not sent"
    />
  </div>
</template>
