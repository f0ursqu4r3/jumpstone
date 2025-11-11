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

const formattedDraft = computed(() => props.editDraft || '—')
const formattedOriginal = computed(() => props.editOriginal || '—')
</script>

<template>
  <div class="space-y-2">
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
      <span class="ml-auto" />
      <UPopover v-if="message.isAuthor || canReportMessage()">
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
    </div>

    <div v-if="isEditing" class="space-y-3">
      <textarea
        :value="editDraft"
        class="w-full rounded-xl border border-white/10 bg-slate-900/60 p-3 text-sm text-white focus:border-sky-400 focus:outline-none focus:ring-0"
        rows="3"
        @input="emit('update:editDraft', ($event.target as HTMLTextAreaElement).value)"
      />
      <div
        class="grid gap-3 rounded-2xl border border-white/10 bg-slate-950/40 p-3 text-xs text-slate-200 sm:grid-cols-2"
      >
        <div class="space-y-1">
          <p class="font-semibold uppercase tracking-wide text-slate-500">Original</p>
          <p
            class="rounded-xl border border-white/5 bg-slate-900/60 p-2 text-sm text-slate-300 whitespace-pre-line wrap-break-word"
          >
            {{ formattedOriginal }}
          </p>
        </div>
        <div class="space-y-1">
          <p class="font-semibold uppercase tracking-wide text-slate-500">Revised preview</p>
          <p
            :class="[
              'rounded-xl border p-2 text-sm whitespace-pre-line wrap-break-word',
              editHasChanges
                ? 'border-emerald-500/30 bg-emerald-500/5 text-emerald-100'
                : 'border-white/5 bg-slate-900/60 text-slate-300',
            ]"
          >
            {{ formattedDraft }}
          </p>
          <p class="text-[11px]" :class="editHasChanges ? 'text-emerald-300' : 'text-slate-500'">
            {{ editHasChanges ? 'Looks good—save to publish the update.' : 'No changes yet.' }}
          </p>
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <UButton size="xs" color="info" :loading="editSaving" @click="emit('save-edit')">
          Save
        </UButton>
        <UButton size="xs" variant="ghost" color="neutral" @click="emit('cancel-edit')">
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
        @click="message.localId && emit('retry', message.localId)"
      >
        Retry
      </UButton>
    </div>
    <div v-else-if="message.eventType !== 'message'" class="text-xs text-slate-500">
      System event placeholder — richer rendering lands in Week 6.
    </div>
    <div class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
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
              @click="(emit('select-reaction', emoji), close())"
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
        @click="emit('copy-meta')"
      >
        Copy meta
      </UButton>
    </div>
  </div>
</template>
