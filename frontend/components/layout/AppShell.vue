<template>
  <div class="flex min-h-screen bg-background text-slate-100">
    <aside
      class="flex w-20 flex-col items-center gap-4 border-r border-surface-muted bg-background-elevated py-6"
    >
      <div class="text-xs uppercase tracking-[0.3em] text-slate-500">
        OG
      </div>
      <nav class="flex flex-1 flex-col items-center gap-4">
        <UTooltip
          v-for="guild in guilds"
          :key="guild.id"
          :text="guild.name"
          :open-delay="100"
        >
          <UButton
            :color="guild.active ? 'primary' : 'gray'"
            :variant="guild.active ? 'solid' : 'soft'"
            class="h-12 w-12 rounded-2xl font-semibold transition"
          >
            {{ guild.initials }}
          </UButton>
        </UTooltip>
        <UButton
          icon="i-heroicons-plus"
          color="gray"
          variant="ghost"
          class="h-12 w-12 rounded-2xl border border-dashed border-slate-600 text-slate-400"
        />
      </nav>
    </aside>

    <aside
      class="hidden w-72 flex-col border-r border-surface-muted bg-background-elevated/80 lg:flex"
    >
      <div class="flex items-center justify-between px-5 py-4">
        <div>
          <p class="text-xs uppercase tracking-widest text-slate-500">
            Guild
          </p>
          <p class="text-sm font-medium text-white">
            {{ activeGuild?.name ?? 'Select a guild' }}
          </p>
        </div>
        <UButton color="primary" variant="soft" size="xs">
          Invite
        </UButton>
      </div>
      <div class="flex-1 overflow-y-auto px-3 pb-4">
        <template
          v-for="(channel, index) in channels"
          :key="channel.id"
        >
          <p
            v-if="shouldShowCategoryLabel(index)"
            class="mt-4 px-2 text-[0.65rem] uppercase tracking-widest text-slate-500"
          >
            {{ channel.category }}
          </p>
          <UButton
            color="gray"
            variant="ghost"
            class="group mt-1 flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm transition"
            :class="[
              channel.active
                ? 'bg-surface-subtle text-white shadow-elevated-sm'
                : 'text-slate-300 hover:bg-surface-muted/70',
            ]"
          >
            <span
              class="flex h-5 w-5 items-center justify-center text-xs text-slate-400"
            >
              <UIcon
                v-if="channel.locked"
                name="i-heroicons-lock-closed"
                class="h-3 w-3"
              />
              <span v-else>#</span>
            </span>
            <span class="flex-1 truncate">
              {{ channel.name }}
            </span>
            <span
              v-if="channel.unread"
              class="ml-2 inline-flex h-2 w-2 rounded-full bg-brand-accent"
            />
          </UButton>
        </template>
      </div>
    </aside>

    <div class="flex flex-1 flex-col">
      <header
        class="flex h-14 items-center justify-between border-b border-surface-muted bg-background-elevated/80 px-6"
      >
        <div class="flex items-center gap-3">
          <UBadge color="primary" variant="outline">
            {{ activeChannel?.category ?? 'general' }}
          </UBadge>
          <h1 class="text-lg font-semibold text-white">
            {{ activeChannel?.name ?? 'Welcome' }}
          </h1>
        </div>
        <div class="flex items-center gap-4 text-sm text-slate-400">
          <div class="flex items-center gap-2">
            <span class="inline-flex h-2 w-2 rounded-full bg-brand-primary" />
            <span>{{ statusText }}</span>
          </div>
          <span v-if="latencyMs !== null">
            {{ latencyLabel }}
          </span>
          <UButton
            variant="ghost"
            color="gray"
            size="sm"
            icon="i-heroicons-arrow-right-circle"
          >
            Switch guild
          </UButton>
        </div>
      </header>

      <main class="flex-1 overflow-y-auto bg-background px-6 py-6">
        <slot />
      </main>

      <footer
        class="flex h-12 items-center justify-between border-t border-surface-muted bg-background-elevated/70 px-6 text-xs text-slate-400"
      >
        <div class="flex items-center gap-3">
          <UAvatar
            v-if="currentUser"
            :text="currentUserInitials"
            size="sm"
            class="bg-surface-subtle text-slate-100"
          />
          <div class="flex flex-col">
            <span class="font-medium text-white">
              {{ currentUser?.name ?? 'Guest' }}
            </span>
            <UBadge
              size="xs"
              :color="currentUserStatusColor"
              variant="soft"
              class="w-fit uppercase tracking-widest"
            >
              {{ currentUserStatusLabel }}
            </UBadge>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <slot name="status" />
          <UBadge variant="ghost" color="gray">
            {{ guilds.length }} Guilds
          </UBadge>
        </div>
      </footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

type GuildNavItem = {
  id: string;
  name: string;
  initials: string;
  active?: boolean;
};

type ChannelNavItem = {
  id: string;
  name: string;
  category?: string;
  unread?: boolean;
  locked?: boolean;
  active?: boolean;
};

type CurrentUser = {
  name: string;
  status: string;
};

const props = withDefaults(
  defineProps<{
    guilds?: GuildNavItem[];
    channels?: ChannelNavItem[];
    statusText?: string;
    latencyMs?: number | null;
    currentUser?: CurrentUser | null;
  }>(),
  {
    guilds: () => [],
    channels: () => [],
    statusText: 'All systems nominal',
    latencyMs: null,
    currentUser: null,
  },
);

const statusColors: Record<string, string> = {
  online: 'green',
  idle: 'yellow',
  dnd: 'red',
  offline: 'gray',
};

const activeGuild = computed(() =>
  props.guilds.find((guild) => guild.active) ?? null,
);

const activeChannel = computed(() =>
  props.channels.find((channel) => channel.active) ?? null,
);

const currentUserInitials = computed(() => {
  const name = props.currentUser?.name?.trim();
  if (!name) {
    return '?';
  }
  const parts = name.split(/\s+/);
  if (parts.length === 1) {
    return parts[0][0]?.toUpperCase() ?? '?';
  }
  const first = parts[0][0] ?? '';
  const last = parts[parts.length - 1][0] ?? '';
  return `${first}${last}`.toUpperCase();
});

const currentUserStatusColor = computed(
  () => statusColors[props.currentUser?.status ?? 'offline'] ?? 'gray',
);

const currentUserStatusLabel = computed(() =>
  (props.currentUser?.status ?? 'offline').toUpperCase(),
);

const latencyLabel = computed(() => {
  if (props.latencyMs === null) {
    return null;
  }

  return `Latency: ${props.latencyMs}ms`;
});

const shouldShowCategoryLabel = (index: number) => {
  const current = props.channels[index];
  if (!current?.category) {
    return false;
  }

  if (index === 0) {
    return true;
  }

  return props.channels[index - 1]?.category !== current.category;
};
</script>
