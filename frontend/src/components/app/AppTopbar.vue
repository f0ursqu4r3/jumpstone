<script setup lang="ts">
import { computed } from 'vue'

import Button from '@/components/ui/Button.vue'
import Input from '@/components/ui/Input.vue'

const props = defineProps<{
  channelName: string
  topic?: string
  showMobileNavButton?: boolean
}>()

const emit = defineEmits<{
  (event: 'request-mobile-nav'): void
}>()

const channelName = computed(() => props.channelName)
</script>

<template>
  <header
    class="flex items-center justify-between border-b border-white/5 bg-slate-950/70 px-3 py-2 backdrop-blur sm:px-4"
  >
    <div id="messages-topbar-channel" class="flex items-center gap-3">
      <Button
        v-if="props.showMobileNavButton"
        icon="i-heroicons-bars-3"
        color="neutral"
        variant="ghost"
        class="lg:hidden"
        aria-label="Open navigation"
        @click="emit('request-mobile-nav')"
      />
      <div>
        <div class="flex items-center gap-2">
          <UIcon name="i-heroicons-hashtag" class="h-5 w-5 text-slate-400" />
          <h1 class="text-lg font-semibold text-white">
            {{ channelName }}
          </h1>
        </div>
        <p v-if="props.topic" class="mt-1 text-xs text-slate-400">
          {{ props.topic }}
        </p>
      </div>
    </div>
    <div id="messages-topbar-actions" class="flex flex-wrap items-center gap-2 sm:gap-3">
      <slot name="actions" />
      <Input
        placeholder="Search"
        icon="i-heroicons-magnifying-glass-20-solid"
        color="neutral"
        variant="soft"
        class="hidden w-64 lg:block"
      />
      <Button
        icon="i-heroicons-bell-alert"
        color="neutral"
        variant="ghost"
        aria-label="Notifications"
      />
      <Button icon="i-heroicons-queue-list" color="neutral" variant="ghost" aria-label="Inbox" />
    </div>
  </header>
</template>
