<template>
  <div class="flex min-h-screen bg-background text-slate-100">
    <aside
      class="flex w-20 flex-col items-center gap-4 border-r border-surface-muted bg-background-elevated py-6"
    >
      <div class="text-xs uppercase tracking-[0.3em] text-slate-500">
        OG
      </div>
      <nav class="flex flex-1 flex-col items-center gap-4">
        <button
          v-for="guild in guilds"
          :key="guild.id"
          :title="guild.name"
          type="button"
          class="flex h-12 w-12 items-center justify-center rounded-2xl border border-transparent text-sm font-semibold transition"
          :class="[
            guild.active
              ? 'bg-brand-primary text-white shadow-elevated-sm'
              : 'bg-surface-subtle text-slate-300 hover:bg-surface-muted border-surface-muted/60',
          ]"
        >
          <span>{{ guild.initials }}</span>
        </button>
        <button
          type="button"
          class="flex h-12 w-12 items-center justify-center rounded-2xl border border-dashed border-slate-600 text-slate-500 transition hover:border-slate-400 hover:text-white"
        >
          +
        </button>
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
        <UiButton variant="secondary" size="xs">
          Invite
        </UiButton>
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
          <button
            type="button"
            class="group mt-1 flex w-full items-center gap-2 rounded-md px-2 py-2 text-sm transition"
            :class="[
              channel.active
                ? 'bg-surface-subtle text-white shadow-elevated-sm'
                : 'text-slate-300 hover:bg-surface-muted/70',
            ]"
          >
            <span
              class="flex h-5 w-5 items-center justify-center text-xs text-slate-400"
            >
              <svg
                v-if="channel.locked"
                class="h-3 w-3"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 20 20"
                fill="currentColor"
                aria-hidden="true"
              >
                <path
                  fill-rule="evenodd"
                  d="M5 8V6a5 5 0 1110 0v2h.5A1.5 1.5 0 0117 9.5v7A1.5 1.5 0 0115.5 18h-11A1.5 1.5 0 013 16.5v-7A1.5 1.5 0 014.5 8H5zm2-2a3 3 0 116 0v2H7V6zm3 5a1 1 0 00-1 1v2a1 1 0 002 0v-2a1 1 0 00-1-1z"
                  clip-rule="evenodd"
                />
              </svg>
              <span v-else>#</span>
            </span>
            <span class="flex-1 truncate">
              {{ channel.name }}
            </span>
            <span
              v-if="channel.unread"
              class="ml-2 inline-flex h-2 w-2 rounded-full bg-brand-accent"
            />
          </button>
        </template>
      </div>
    </aside>

    <div class="flex flex-1 flex-col">
      <header
        class="flex h-14 items-center justify-between border-b border-surface-muted bg-background-elevated/80 px-6"
      >
        <div class="flex items-center gap-3">
          <UiBadge variant="outline">
            {{ activeChannel?.category ?? 'general' }}
          </UiBadge>
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
          <UiButton variant="ghost" size="sm">
            <template #icon>
              <svg
                class="h-4 w-4"
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
                stroke-width="1.5"
                stroke="currentColor"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  d="M8.25 4.5l7.5 7.5-7.5 7.5"
                />
              </svg>
            </template>
            Switch guild
          </UiButton>
        </div>
      </header>

      <main class="flex-1 overflow-y-auto bg-background px-6 py-6">
        <slot />
      </main>

      <footer
        class="flex h-12 items-center justify-between border-t border-surface-muted bg-background-elevated/70 px-6 text-xs text-slate-400"
      >
        <div class="flex items-center gap-3">
          <UiAvatar
            v-if="currentUser"
            size="sm"
            :name="currentUser.name"
          />
          <div class="flex flex-col">
            <span class="font-medium text-white">
              {{ currentUser?.name ?? 'Guest' }}
            </span>
            <span class="text-[0.7rem] uppercase tracking-widest text-slate-500">
              {{ currentUser?.status ?? 'offline' }}
            </span>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <slot name="status" />
          <UiBadge variant="ghost">
            {{ guilds.length }} Guilds
          </UiBadge>
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

const activeGuild = computed(() =>
  props.guilds.find((guild) => guild.active) ?? null,
);

const activeChannel = computed(() =>
  props.channels.find((channel) => channel.active) ?? null,
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
