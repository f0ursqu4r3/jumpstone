<script setup lang="ts">
import { computed } from 'vue';
import BaseButton from '~/components/ui/BaseButton.vue';
import BaseTooltip from '~/components/ui/BaseTooltip.vue';

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
    class="hidden h-full w-16 flex-col items-center gap-2 border-r border-white/5 bg-slate-950/80 p-2 md:flex"
  >
    <NuxtLink
      class="flex size-10 items-center justify-center rounded-xl bg-slate-800 text-lg font-semibold text-slate-200 transition hover:rounded-3xl hover:bg-sky-500 hover:text-white"
      :class="{ 'rounded-3xl bg-sky-500 text-white': !hasActiveGuild }"
      to="/"
    >
      OG
    </NuxtLink>
    <div class="flex flex-col gap-2 w-full">
      <USeparator class="px-2" />
      <BaseTooltip
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
      </BaseTooltip>

      <USeparator v-if="props.guilds.length > 0" class="px-2" />

      <div class="relative flex justify-center align-middle w-full">
        <BaseButton
          icon="i-heroicons-plus"
          color="neutral"
          variant="ghost"
          size="md"
          class="justify-center"
        />
      </div>
    </div>
    <div class="mt-auto flex w-full flex-col gap-3">
      <BaseButton
        icon="i-heroicons-ellipsis-horizontal"
        color="neutral"
        variant="ghost"
        size="md"
        class="justify-center"
      />
    </div>
  </aside>
</template>
