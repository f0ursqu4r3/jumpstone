import { defineStore } from 'pinia';

export interface GuildSummary {
  id: string;
  name: string;
  initials: string;
  notificationCount?: number;
}

interface GuildState {
  guilds: GuildSummary[];
  activeGuildId: string | null;
  loading: boolean;
  error: string | null;
  hydrated: boolean;
  lastFetchedAt: number | null;
}

const STUB_GUILDS: GuildSummary[] = [
  {
    id: 'openguild',
    name: 'OpenGuild Core',
    initials: 'OG',
    notificationCount: 2,
  },
  {
    id: 'design-lab',
    name: 'Design Lab',
    initials: 'DL',
    notificationCount: 0,
  },
  {
    id: 'infra',
    name: 'Infra Ops',
    initials: 'IO',
    notificationCount: 5,
  },
];

export const useGuildStore = defineStore('guilds', {
  state: (): GuildState => ({
    guilds: [],
    activeGuildId: null,
    loading: false,
    error: null,
    hydrated: false,
    lastFetchedAt: null,
  }),

  getters: {
    activeGuild(state): GuildSummary | null {
      if (!state.activeGuildId) {
        return null;
      }

      return (
        state.guilds.find((guild) => guild.id === state.activeGuildId) ?? null
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
        this.guilds = STUB_GUILDS;
        this.activeGuildId =
          this.activeGuildId ?? STUB_GUILDS[0]?.id ?? null;
        this.hydrated = true;
        this.lastFetchedAt = Date.now();
      } catch (err) {
        this.error =
          err instanceof Error ? err.message : 'Failed to hydrate guilds';
      } finally {
        this.loading = false;
      }
    },

    setActiveGuild(guildId: string) {
      if (!this.guilds.some((guild) => guild.id === guildId)) {
        this.error = `Unknown guild: ${guildId}`;
        return;
      }

      this.activeGuildId = guildId;
      this.error = null;
    },
  },
});
