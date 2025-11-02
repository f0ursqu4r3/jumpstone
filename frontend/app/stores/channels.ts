import { defineStore } from 'pinia';

export type ChannelKind = 'text' | 'voice';

export interface ChannelSummary {
  id: string;
  label: string;
  kind: ChannelKind;
  icon?: string;
  unread?: number;
  description?: string;
}

const STUB_CHANNELS: Record<string, ChannelSummary[]> = {
  openguild: [
    {
      id: 'general',
      label: 'general',
      kind: 'text',
      unread: 3,
      description: 'Roadmap, weekly sync notes, launch prep',
    },
    {
      id: 'announcements',
      label: 'announcements',
      kind: 'text',
      icon: 'i-heroicons-megaphone',
      description: 'Ship updates from the core team',
    },
    { id: 'frontend-team', label: 'frontend-team', kind: 'text' },
    { id: 'voice-standup', label: 'Daily standup', kind: 'voice' },
    { id: 'voice-warroom', label: 'War room', kind: 'voice' },
  ],
  'design-lab': [
    {
      id: 'design-changelog',
      label: 'design-changelog',
      kind: 'text',
      description: 'Figma updates and feedback threads',
    },
    { id: 'design-crit', label: 'design-crit', kind: 'voice' },
  ],
  infra: [
    {
      id: 'ops-announcements',
      label: 'ops-announcements',
      kind: 'text',
      description: 'Rollout notices and rotation changes',
    },
    { id: 'pager', label: 'pager-duty', kind: 'voice' },
  ],
};

export const useChannelStore = defineStore('channels', () => {
  const channelsByGuild = ref<Record<string, ChannelSummary[]>>({});
  const activeGuildId = ref<string | null>(null);
  const activeChannelId = ref<string | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const hydrated = ref(false);
  const lastFetchedAt = ref<number | null>(null);

  const channelsForGuild = computed(() => {
    if (!activeGuildId.value) {
      return [];
    }
    return channelsByGuild.value[activeGuildId.value] ?? [];
  });

  const activeChannel = computed(() => {
    if (!activeGuildId.value || !activeChannelId.value) {
      return null;
    }

    const scoped = channelsByGuild.value[activeGuildId.value] ?? [];
    return (
      scoped.find((channel) => channel.id === activeChannelId.value) ?? null
    );
  });

  function hydrate(force = false) {
    if (loading.value) {
      return;
    }

    if (hydrated.value && !force) {
      return;
    }

    loading.value = true;
    error.value = null;

    try {
      channelsByGuild.value = STUB_CHANNELS;
      hydrated.value = true;
      lastFetchedAt.value = Date.now();

      if (activeGuildId.value) {
        ensureActiveChannelForGuild(activeGuildId.value);
      }
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : 'Failed to hydrate channels';
    } finally {
      loading.value = false;
    }
  }

  function setActiveGuild(guildId: string | null) {
    if (!guildId) {
      activeGuildId.value = null;
      activeChannelId.value = null;
      return;
    }

    activeGuildId.value = guildId;
    ensureActiveChannelForGuild(guildId);
  }

  function setActiveChannel(channelId: string) {
    if (!activeGuildId.value) {
      error.value = 'No active guild selected';
      return;
    }

    const scoped = channelsByGuild.value[activeGuildId.value] ?? [];
    if (!scoped.some((channel) => channel.id === channelId)) {
      error.value = `Unknown channel: ${channelId}`;
      return;
    }

    activeChannelId.value = channelId;
    error.value = null;
  }

  function ensureActiveChannelForGuild(guildId: string) {
    const scoped = channelsByGuild.value[guildId] ?? [];
    if (!scoped.length) {
      activeChannelId.value = null;
      return;
    }

    if (
      !activeChannelId.value ||
      !scoped.some((channel) => channel.id === activeChannelId.value)
    ) {
      const first = scoped[0];
      if (first) {
        activeChannelId.value = first.id;
      } else {
        activeChannelId.value = null;
      }
    }
  }

  return {
    // state
    channelsByGuild,
    activeGuildId,
    activeChannelId,
    loading,
    error,
    hydrated,
    lastFetchedAt,
    channelsForGuild,
    activeChannel,
    // actions
    hydrate,
    setActiveGuild,
    setActiveChannel,
  };
});
