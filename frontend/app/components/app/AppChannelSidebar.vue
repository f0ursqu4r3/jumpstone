<script setup lang="ts">
import { computed } from 'vue';
import { useSessionStore } from '~/stores/session';

interface ChannelEntry {
  id: string;
  label: string;
  kind: 'text' | 'voice';
  icon?: string;
  to?: string;
  unread?: number;
  description?: string;
}

const props = defineProps<{
  guildName: string;
  channels: ChannelEntry[];
}>();

const guildName = computed(() => props.guildName);
const channels = computed(() => props.channels);
const sessionStore = useSessionStore();
const isAuthenticated = computed(() => sessionStore.isAuthenticated);
const accountLabel = computed(
  () => sessionStore.identifier || 'Signed out user'
);
const accountStatus = computed(() =>
  isAuthenticated.value ? 'Online' : 'Logged out'
);
const accountStatusClass = computed(() =>
  isAuthenticated.value ? 'text-emerald-400' : 'text-slate-500'
);
const avatarUrl = computed(() => {
  const seed = sessionStore.identifier || 'OpenGuild';
  return `https://api.dicebear.com/7.x/initials/svg?seed=${encodeURIComponent(
    seed
  )}`;
});

const handleSignOut = async () => {
  if (!isAuthenticated.value) {
    await navigateTo('/login');
    return;
  }

  sessionStore.logout();
  await navigateTo('/login');
};

const groupedChannels = computed(() => {
  const buckets: Record<
    'text' | 'voice',
    { label: string; children: Array<any> }
  > = {
    text: { label: 'Text Channels', children: [] },
    voice: { label: 'Voice Channels', children: [] },
  };

  channels.value.forEach((channel) => {
    const icon =
      channel.icon ??
      (channel.kind === 'voice'
        ? 'i-heroicons-speaker-wave'
        : 'i-heroicons-hashtag');

    buckets[channel.kind].children.push({
      label: `${channel.kind === 'text' ? '#' : ''}${channel.label}`,
      icon,
      to: channel.to ?? '#',
      badge: channel.unread
        ? {
            label: channel.unread > 9 ? '9+' : channel.unread.toString(),
            color: 'sky',
          }
        : undefined,
      description: channel.description,
    });
  });

  return Object.values(buckets).filter((bucket) => bucket.children.length > 0);
});
</script>

<template>
  <aside
    class="flex flex-col h-full w-72 justify-between border-r border-white/5"
  >
    <div class="flex flex-col flex-1 min-h-0">
      <div class="flex items-start justify-between gap-2 p-2">
        <div>
          <p class="text-sm font-semibold">
            {{ guildName }}
          </p>
          <p class="text-xs text-dimmed">Internal build</p>
        </div>
        <UButton
          icon="i-heroicons-cog-6-tooth"
          color="neutral"
          variant="ghost"
          aria-label="Guild settings"
        />
      </div>

      <UTooltip text="Create channel">
        <template #trigger>
          <UButton
            label="New channel"
            color="info"
            variant="soft"
            class="mt-6 w-full justify-center"
            icon="i-heroicons-plus-circle"
          />
        </template>
      </UTooltip>

      <USeparator class="uppercase tracking-wide m-0" />

      <div class="space-y-4 p-2 overflow-y-auto flex-1 min-h-0">
        <section
          v-for="group in groupedChannels"
          :key="group.label"
          class="space-y-3"
        >
          <p
            class="text-xs font-semibold uppercase tracking-wide text-slate-500"
          >
            {{ group.label }}
          </p>
          <ul>
            <li v-for="channel in group.children" :key="channel.label">
              <NuxtLink
                :to="channel.to"
                class="flex items-center justify-between rounded-md px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-800 hover:text-white"
              >
                <div class="flex items-center gap-2">
                  <UIcon :name="channel.icon" class="h-5 w-5 shrink-0" />
                  <span>{{ channel.label }}</span>
                </div>
                <div v-if="channel.badge">
                  <UBadge
                    :label="channel.badge.label"
                    :color="channel.badge.color"
                    size="sm"
                  />
                </div>
              </NuxtLink>
            </li>
          </ul>
        </section>
      </div>
    </div>

    <div>
      <USeparator class="opacity-50" />
      <div class="flex items-center gap-3 p-2">
        <UAvatar
          :name="accountLabel"
          size="sm"
          :src="avatarUrl"
        />
        <div class="flex-1 text-sm">
          <p class="font-semibold text-white">
            {{ accountLabel }}
          </p>
          <p class="text-xs" :class="accountStatusClass">
            {{ accountStatus }}
          </p>
        </div>
        <UButton
          icon="i-heroicons-arrow-left-on-rectangle"
          variant="ghost"
          color="neutral"
          aria-label="Sign out"
          :disabled="!isAuthenticated"
          @click="handleSignOut"
        />
      </div>
    </div>
  </aside>
</template>
