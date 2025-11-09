<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'

import { useSearchStore } from '@/stores/search'

const open = defineModel<boolean>('open', { default: false })

const searchStore = useSearchStore()
const { results, loading, error, lastQuery, lastFetchedAt } = storeToRefs(searchStore)

const query = ref('')
const inputRef = ref<HTMLInputElement | null>(null)

const hasQuery = computed(() => query.value.trim().length > 0)
const showEmptyState = computed(
  () => !loading.value && hasQuery.value && results.value.length === 0 && !error.value,
)

watch(
  () => open.value,
  (isOpen) => {
    if (isOpen) {
      nextTick(() => inputRef.value?.focus())
      return
    }
    query.value = ''
    searchStore.reset()
  },
)

const handleSubmit = async () => {
  await searchStore.performSearch(query.value)
}

const handleKey = async (event: KeyboardEvent) => {
  if (event.key === 'Enter' && query.value.trim().length) {
    event.preventDefault()
    await handleSubmit()
  }
}

const formatTimestamp = (iso: string | null | undefined) => {
  if (!iso) {
    return '—'
  }
  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date(iso))
  } catch {
    return iso
  }
}
</script>

<template>
  <UModal
    v-model:open="open"
    size="xl"
    title="Global search"
    description="Find messages, channels, and members across guilds. API placeholder until search endpoints land."
  >
    <template #content>
      <div class="space-y-5 p-6">
        <div class="flex items-center gap-3">
          <UInput
            ref="inputRef"
            v-model="query"
            icon="i-heroicons-magnifying-glass"
            placeholder="Search messages, channels, members…"
            class="flex-1"
            :disabled="loading"
            @keyup="handleKey"
          />
          <UButton color="info" :loading="loading" :disabled="!hasQuery" @click="handleSubmit">
            Search
          </UButton>
        </div>
        <div class="flex flex-wrap items-center gap-3 text-xs text-slate-500">
          <span v-if="lastQuery">Last query “{{ lastQuery }}”</span>
          <span v-if="lastFetchedAt">· Updated {{ formatTimestamp(lastFetchedAt) }}</span>
        </div>

        <UAlert
          v-if="error"
          color="warning"
          variant="soft"
          icon="i-heroicons-exclamation-triangle"
          :description="error"
        />

        <div v-if="loading" class="space-y-3">
          <div v-for="index in 4" :key="index" class="rounded-2xl border border-white/5 p-4">
            <div class="flex items-center gap-2">
              <USkeleton class="h-6 w-6 rounded-full" />
              <USkeleton class="h-3 w-32 rounded" />
            </div>
            <USkeleton class="mt-3 h-3 w-full rounded" />
            <USkeleton class="mt-2 h-3 w-2/3 rounded" />
          </div>
        </div>

        <div
          v-else-if="showEmptyState"
          class="flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-white/10 bg-slate-950/50 px-6 py-10 text-center"
        >
          <UIcon name="i-heroicons-magnifying-glass" class="h-10 w-10 text-slate-600" />
          <p class="text-sm font-semibold text-white">No results yet</p>
          <p class="text-xs text-slate-500">
            Try another keyword or wait for the backend search API to land.
          </p>
        </div>

        <ul v-else class="space-y-3">
          <li
            v-for="result in results"
            :key="result.id"
            class="rounded-2xl border border-white/5 bg-slate-950/60 p-4"
          >
            <div class="flex flex-wrap items-center gap-2">
              <p class="text-sm font-semibold text-white">{{ result.title }}</p>
              <UBadge variant="soft" color="neutral" :label="result.type" />
              <span v-if="result.subtitle" class="text-xs text-slate-500">
                {{ result.subtitle }}
              </span>
            </div>
            <p v-if="result.snippet" class="mt-2 text-sm text-slate-300">
              {{ result.snippet }}
            </p>
            <div class="mt-2 flex flex-wrap items-center gap-3 text-[11px] text-slate-500">
              <span v-if="result.channelId">Channel: {{ result.channelId }}</span>
              <span v-if="result.eventId">Event: {{ result.eventId }}</span>
            </div>
          </li>
        </ul>
      </div>
    </template>
  </UModal>
</template>
