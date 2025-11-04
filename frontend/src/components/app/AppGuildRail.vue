<script setup lang="ts">
import { computed } from 'vue'

import Button from '@/components/ui/Button.vue'
import Badge from '@/components/ui/Badge.vue'
import type { GuildSummary } from '@/types/ui'

const props = withDefaults(
  defineProps<{
    guilds: GuildSummary[]
    loading?: boolean
  }>(),
  {
    loading: false,
  },
)

const emit = defineEmits<{
  (event: 'select', guildId: string): void
  (event: 'create'): void
  (event: 'open-menu'): void
}>()

const hasActiveGuild = computed(() => props.guilds.some((guild) => guild.active))
</script>

<template>
  <aside
    class="hidden h-full w-16 flex-col items-center gap-2 border-r border-white/5 bg-slate-950/80 p-2 md:flex"
  >
    <button
      type="button"
      class="flex size-10 items-center justify-center rounded-xl bg-slate-800 text-lg font-semibold text-slate-200 transition hover:rounded-3xl hover:bg-sky-500 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-500"
      :class="{ 'rounded-3xl bg-primary-500 text-white': !hasActiveGuild }"
      @click="emit('select', '')"
    >
      OG
    </button>

    <div class="flex w-full flex-col gap-2">
      <USeparator class="opacity-50" />

      <template v-if="loading">
        <div v-for="index in 4" :key="index" class="flex justify-center">
          <USkeleton class="h-12 w-12 rounded-2xl" />
        </div>
      </template>

      <template v-else>
        <UTooltip
          v-for="guild in props.guilds"
          :key="guild.id"
          :text="guild.name"
          :content="{ side: 'right' }"
        >
          <button
            type="button"
            class="relative flex size-12 items-center justify-center rounded-xl bg-slate-800 text-sm font-semibold uppercase transition duration-500 hover:bg-brand-500/50 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-brand-500"
            :class="{
              'rounded-3xl border-2 border-brand-500 bg-brand-500 shadow-md shadow-brand-500/50 text-white':
                guild.active,
            }"
            @click="emit('select', guild.id)"
          >
            {{ guild.initials }}
            <Badge
              v-if="guild.notificationCount"
              class="absolute -right-1 -top-1 flex size-5 items-center justify-center rounded-full text-[8pt] font-semibold"
              variant="solid"
            >
              {{ guild.notificationCount }}
            </Badge>
          </button>
        </UTooltip>
      </template>

      <div class="relative flex w-full justify-center">
        <Button
          icon="i-heroicons-plus"
          color="neutral"
          variant="ghost"
          size="md"
          class="justify-center"
          @click="emit('create')"
          aria-label="Create guild"
        />
      </div>
    </div>

    <div class="mt-auto flex w-full flex-col gap-3">
      <Button
        icon="i-heroicons-ellipsis-horizontal"
        color="neutral"
        variant="ghost"
        size="md"
        class="justify-center"
        @click="emit('open-menu')"
        aria-label="Open guild menu"
      />
    </div>
  </aside>
</template>
