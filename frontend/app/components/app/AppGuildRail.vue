<script setup lang="ts">
import { computed } from 'vue';

interface GuildSummary {
  id: string;
  name: string;
  initials: string;
  active?: boolean;
  notificationCount?: number;
}

const props = defineProps<{
  guilds: GuildSummary[];
}>();

const hasActiveGuild = computed(() =>
  props.guilds.some((guild) => guild.active)
);
</script>

<template>
  <aside
    class="hidden h-full w-16 flex-col items-center gap-4 border-r border-white/5 bg-slate-950/80 p-2 md:flex"
  >
    <NuxtLink
      class="flex size-12 items-center justify-center rounded-xl bg-slate-800 text-xl font-semibold text-slate-200 transition hover:rounded-3xl hover:bg-sky-500 hover:text-white"
      :class="{ 'rounded-3xl bg-sky-500 text-white': !hasActiveGuild }"
      to="/"
    >
      OG
    </NuxtLink>
    <UTooltip
      v-for="guild in props.guilds"
      :key="guild.id"
      :text="guild.name"
      placement="right"
    >
      <template #trigger>
        <div class="relative">
          <button
            type="button"
            class="flex size-12 items-center justify-center rounded-xl bg-slate-800 text-sm font-semibold uppercase transition hover:rounded-3xl hover:bg-sky-500 hover:text-white"
            :class="{
              'rounded-3xl bg-sky-500 text-white shadow-lg shadow-sky-500/30':
                guild.active,
            }"
          >
            {{ guild.initials }}
          </button>
          <span
            v-if="guild.notificationCount"
            class="absolute -right-1 -top-1 flex h-5 w-5 items-center justify-center rounded-full bg-rose-500 text-[10px] font-semibold text-white"
          >
            {{ guild.notificationCount }}
          </span>
        </div>
      </template>
    </UTooltip>
    <div class="mt-auto flex w-full flex-col gap-3">
      <UButton
        icon="i-heroicons-plus"
        color="neutral"
        variant="ghost"
        size="md"
        class="justify-center"
      />
      <UButton
        icon="i-heroicons-ellipsis-horizontal"
        color="neutral"
        variant="ghost"
        size="md"
        class="justify-center"
      />
    </div>
  </aside>
</template>
