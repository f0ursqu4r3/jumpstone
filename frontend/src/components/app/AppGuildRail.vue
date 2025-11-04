<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import Button from '@/components/ui/Button.vue'
import Badge from '@/components/ui/Badge.vue'

interface GuildSummary {
  id: string
  name: string
  initials: string
  active?: boolean
  notificationCount?: number
}

const props = defineProps<{
  guilds: GuildSummary[]
}>()

const hasActiveGuild = computed(() => props.guilds.some((guild) => guild.active))
</script>

<template>
  <aside
    class="hidden h-full w-16 flex-col items-center gap-2 border-r border-white/5 bg-slate-950/80 p-2 md:flex"
  >
    <RouterLink
      class="flex size-10 items-center justify-center rounded-xl bg-slate-800 text-lg font-semibold text-slate-200 transition hover:rounded-3xl hover:bg-sky-500 hover:text-white"
      :class="{ 'rounded-3xl bg-primary-500 text-white': !hasActiveGuild }"
      to="/"
    >
      OG
    </RouterLink>
    <div class="flex w-full flex-col gap-2">
      <USeparator class="opacity-50" />
      <UTooltip
        v-for="guild in props.guilds"
        :key="guild.id"
        :text="guild.name"
        :content="{ side: 'right' }"
      >
        <div class="relative">
          <Button
            type="button"
            class="flex size-12 items-center justify-center rounded-xl bg-slate-800 text-sm font-semibold uppercase transition duration-500 hover:bg-brand-500/50 hover:text-white"
            :class="{
              'rounded-3xl border-2 border-brand-500 bg-brand-500 shadow-md shadow-brand-500/50':
                guild.active,
            }"
          >
            {{ guild.initials }}
          </Button>
          <Badge
            v-if="guild.notificationCount"
            class="absolute -right-1 -top-1 flex size-5 items-center justify-center rounded-full text-[8pt] font-semibold"
            variant="solid"
          >
            {{ guild.notificationCount }}
          </Badge>
        </div>
      </UTooltip>

      <div class="relative flex w-full justify-center">
        <Button
          icon="i-heroicons-plus"
          color="neutral"
          variant="ghost"
          size="md"
          class="justify-center"
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
      />
    </div>
  </aside>
</template>
