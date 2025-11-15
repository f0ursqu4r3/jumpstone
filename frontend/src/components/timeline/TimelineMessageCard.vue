<script setup lang="ts">
import { computed } from 'vue'

import type { TimelineMessage } from '@/types/messaging'
const props = defineProps<{
  message: TimelineMessage
  reactionPalette: readonly string[]
  isEditing: boolean
  editDraft: string
  editOriginal: string
  editHasChanges: boolean
  editSaving: boolean
  editError: string | null
  reactionButtonClasses: (active: boolean) => string[]
  canEditMessage: (message: { isAuthor: boolean; optimistic: boolean }) => boolean
  canReportMessage: () => boolean
}>()

const emit = defineEmits<{
  (event: 'retry', localId: string): void
  (event: 'edit', messageId: string): void
  (event: 'cancel-edit'): void
  (event: 'save-edit'): void
  (event: 'update:editDraft', value: string): void
  (event: 'toggle-reaction', payload: { emoji: string; currentlyReacted: boolean }): void
  (event: 'select-reaction', emoji: string): void
  (event: 'copy-meta'): void
  (event: 'report'): void
}>()

const formattedOriginal = computed(() => props.editOriginal || '—')
const isSystemEvent = computed(() => props.message.eventType !== 'message')
const showActions = computed(() => props.message.isAuthor || props.canReportMessage())
const originChip = computed(() => {
  if (!props.message.originServer) {
    return null
  }
  return {
    label: props.message.remote ? `Remote · ${props.message.originServer}` : 'Local origin',
    color: props.message.remote ? ('warning' as const) : ('neutral' as const),
  }
})
const editingHint = computed(() =>
  props.editHasChanges
    ? 'Looks good—save to publish the update.'
    : 'No changes yet. Edit your message and save to update the timeline.',
)
</script>

<template>
  <article class="group space-y-4">
    <header class="flex justify-between items-start gap-3">
      <div class="flex min-w-0 space-y-1 gap-2">
        <div class="flex flex-wrap items-center gap-2">
          <p class="text-sm font-semibold text-white">
            {{ message.sender }}
          </p>
          <UBadge
            v-if="isSystemEvent"
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
        </div>
        <div class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
          <span>{{ message.time }}</span>
          <UBadge
            v-if="originChip"
            size="xs"
            variant="soft"
            :color="originChip.color"
            :label="originChip.label"
          />
        </div>
      </div>
      <UPopover v-if="showActions">
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
              v-if="message.isAuthor"
              type="button"
              class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-slate-200 hover:bg-white/5 disabled:opacity-40"
              :disabled="!canEditMessage(message)"
              @click="(emit('edit', message.id), close())"
            >
              <span>Edit message</span>
              <UIcon name="i-heroicons-pencil-square" class="h-4 w-4" />
            </button>
            <button
              type="button"
              class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-slate-200 hover:bg-white/5"
              @click="(emit('copy-meta'), close())"
            >
              <span>Copy event ID</span>
              <UIcon name="i-heroicons-clipboard" class="h-4 w-4" />
            </button>
            <button
              v-if="canReportMessage()"
              type="button"
              class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-slate-200 hover:bg-white/5"
              @click="(emit('report'), close())"
            >
              <span>Report message</span>
              <UIcon name="i-heroicons-flag" class="h-4 w-4" />
            </button>
          </div>
        </template>
      </UPopover>
    </header>

    <section v-if="isEditing" class="space-y-2">
      <textarea
        :value="editDraft"
        class="w-full rounded-xl border border-white/10 bg-slate-900/60 p-3 text-sm text-white focus:border-sky-400 focus:outline-none focus:ring-0"
        rows="3"
        @input="emit('update:editDraft', ($event.target as HTMLTextAreaElement).value)"
      />
      <p
        class="rounded-xl border border-white/5 bg-slate-900/40 p-2 text-xs text-slate-400 whitespace-pre-line wrap-break-word"
      >
        <span class="font-semibold text-slate-300">Original:</span> {{ formattedOriginal }}
      </p>
      <div class="flex flex-wrap items-center gap-3 text-xs">
        <span :class="editHasChanges ? 'text-emerald-300' : 'text-slate-500'">
          {{ editingHint }}
        </span>
        <span v-if="editError" class="text-rose-300">{{ editError }}</span>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <UButton size="xs" color="info" :loading="editSaving" @click="emit('save-edit')">
          Save
        </UButton>
        <UButton size="xs" variant="ghost" color="neutral" @click="emit('cancel-edit')">
          Cancel
        </UButton>
      </div>
    </section>
    <p v-else class="text-sm text-slate-200 whitespace-pre-line wrap-break-word">
      {{ message.content }}
    </p>

    <section v-if="message.optimistic" class="flex flex-wrap items-center gap-2 text-xs">
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
        @click="message.localId && emit('retry', message.localId)"
      >
        Retry
      </UButton>
    </section>
    <section v-else-if="isSystemEvent" class="text-xs text-slate-500">
      System event placeholder — richer rendering lands in Week 6.
    </section>

    <section class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
      <template v-if="message.reactions.length">
        <button
          v-for="reaction in message.reactions"
          :key="reaction.emoji"
          type="button"
          :class="reactionButtonClasses(reaction.reacted)"
          @click="
            emit('toggle-reaction', { emoji: reaction.emoji, currentlyReacted: reaction.reacted })
          "
        >
          <span class="text-base leading-none">{{ reaction.emoji }}</span>
          <span class="text-[10px]">{{ reaction.count }}</span>
        </button>
      </template>
      <span v-else>No reactions yet</span>
      <div
        class="transition-opacity duration-150 opacity-100 md:opacity-0 md:group-hover:opacity-100 md:group-focus-within:opacity-100"
      >
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
                @click="(emit('select-reaction', emoji), close())"
              >
                {{ emoji }}
              </UButton>
            </div>
          </template>
        </UPopover>
      </div>
      <UButton
        size="xs"
        variant="ghost"
        color="neutral"
        icon="i-heroicons-clipboard"
        @click="emit('copy-meta')"
      >
        Copy meta
      </UButton>
    </section>
  </article>
</template>
