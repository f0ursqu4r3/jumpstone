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

interface ChannelState {
  channelsByGuild: Record<string, ChannelSummary[]>;
  activeGuildId: string | null;
  activeChannelId: string | null;
  loading: boolean;
  error: string | null;
  hydrated: boolean;
  lastFetchedAt: number | null;
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

export const useChannelStore = defineStore('channels', {
  state: (): ChannelState => ({
    channelsByGuild: {},
    activeGuildId: null,
    activeChannelId: null,
    loading: false,
    error: null,
    hydrated: false,
    lastFetchedAt: null,
  }),

  getters: {
    channelsForGuild: (state) => {
      if (!state.activeGuildId) {
        return [];
      }
      return state.channelsByGuild[state.activeGuildId] ?? [];
    },
    activeChannel(state): ChannelSummary | null {
      if (!state.activeGuildId || !state.activeChannelId) {
        return null;
      }

      const scoped = state.channelsByGuild[state.activeGuildId] ?? [];
      return (
        scoped.find((channel) => channel.id === state.activeChannelId) ?? null
      );
    },
  },

  actions: {
    hydrate(force = false) {
      if (this.loading) {
        return;
      }

      if (this.hydrated && !force) {
        return;
      }

      this.loading = true;
      this.error = null;

      try {
        this.channelsByGuild = STUB_CHANNELS;
        this.hydrated = true;
        this.lastFetchedAt = Date.now();

        if (this.activeGuildId) {
          this.ensureActiveChannelForGuild(this.activeGuildId);
        }
      } catch (err) {
        this.error =
          err instanceof Error ? err.message : 'Failed to hydrate channels';
      } finally {
        this.loading = false;
      }
    },

    setActiveGuild(guildId: string | null) {
      if (!guildId) {
        this.activeGuildId = null;
        this.activeChannelId = null;
        return;
      }

      this.activeGuildId = guildId;
      this.ensureActiveChannelForGuild(guildId);
    },

    setActiveChannel(channelId: string) {
      if (!this.activeGuildId) {
        this.error = 'No active guild selected';
        return;
      }

      const scoped = this.channelsByGuild[this.activeGuildId] ?? [];
      if (!scoped.some((channel) => channel.id === channelId)) {
        this.error = `Unknown channel: ${channelId}`;
        return;
      }

      this.activeChannelId = channelId;
      this.error = null;
    },

    ensureActiveChannelForGuild(guildId: string) {
      const scoped = this.channelsByGuild[guildId] ?? [];
      if (!scoped.length) {
        this.activeChannelId = null;
        return;
      }

      if (
        !this.activeChannelId ||
        !scoped.some((channel) => channel.id === this.activeChannelId)
      ) {
        this.activeChannelId = scoped[0].id;
      }
    },
  },
});
