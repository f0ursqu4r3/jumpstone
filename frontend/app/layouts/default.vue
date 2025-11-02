<script setup lang="ts">
import { computed, ref } from 'vue';

const guilds = [
  {
    id: 'openguild',
    name: 'OpenGuild Core',
    initials: 'OG',
    active: true,
    notificationCount: 2,
  },
  {
    id: 'design-lab',
    name: 'Design Lab',
    initials: 'DL',
    active: false,
    notificationCount: 0,
  },
  {
    id: 'infra',
    name: 'Infra Ops',
    initials: 'IO',
    active: false,
    notificationCount: 5,
  },
];

const channels = [
  {
    id: 'general',
    label: 'general',
    kind: 'text' as const,
    unread: 3,
    description: 'Roadmap, weekly sync notes, launch prep',
  },
  {
    id: 'announcements',
    label: 'announcements',
    kind: 'text' as const,
    icon: 'i-heroicons-megaphone',
    description: 'Ship updates from the core team',
  },
  { id: 'frontend-team', label: 'frontend-team', kind: 'text' as const },
  { id: 'voice-standup', label: 'Daily standup', kind: 'voice' as const },
  { id: 'voice-warroom', label: 'War room', kind: 'voice' as const },
];

const activeGuild = computed(
  () => guilds.find((guild) => guild.active) ?? guilds[0]
);
const activeChannel = computed(() => channels[0]);
const mobileSidebarOpen = ref(false);
</script>

<template>
  <div class="relative flex h-screen overflow-hidden bg-slate-950">
    <AppGuildRail :guilds="guilds" />

    <AppChannelSidebar
      :guild-name="activeGuild?.name || ''"
      :channels="channels"
      class="hidden lg:flex"
    />

    <USlideover v-model="mobileSidebarOpen" side="left">
      <template #content>
        <div class="flex h-full w-[18rem] flex-col bg-slate-950">
          <AppChannelSidebar
            :guild-name="activeGuild?.name || ''"
            :channels="channels"
            class="flex"
          />
        </div>
      </template>
    </USlideover>

    <div class="flex flex-1 flex-col">
      <AppTopbar
        :channel-name="activeChannel?.label || ''"
        :topic="activeChannel?.description || ''"
      />
      <main
        class="flex-1 overflow-y-auto bg-linear-to-b from-slate-950 via-slate-950 to-slate-950/80"
      >
        <div class="mx-auto w-full max-w-4xl px-4 py-6 sm:px-6 lg:px-10">
          <slot />
        </div>
      </main>
      <footer
        class="border-t border-white/5 bg-slate-950/80 px-6 py-3 text-xs text-slate-500"
      >
        Prototype UI - Federation awareness not yet connected
      </footer>
    </div>

    <div class="fixed left-4 top-4 z-40 flex items-center gap-2 lg:hidden">
      <UButton
        icon="i-heroicons-bars-3"
        color="neutral"
        variant="ghost"
        @click="mobileSidebarOpen = true"
        aria-label="Open navigation"
      />
      <div
        class="rounded-full bg-slate-900/80 px-3 py-1 text-sm font-semibold text-white shadow-lg shadow-slate-900/40 backdrop-blur"
      >
        #{{ activeChannel?.label || '' }}
      </div>
    </div>
  </div>
</template>
