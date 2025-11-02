import { defineStore } from 'pinia';

export interface GuildSummary {
  id: string;
  name: string;
  initials: string;
  notificationCount?: number;
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

export const useGuildStore = defineStore('guilds', () => {
  const guilds = ref<GuildSummary[]>([]);
  const activeGuildId = ref<string | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const hydrated = ref(false);
  const lastFetchedAt = ref<number | null>(null);

  const activeGuild = computed(() => {
    if (!activeGuildId.value) {
      return null;
    }

    return (
      guilds.value.find((guild) => guild.id === activeGuildId.value) ?? null
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
      guilds.value = STUB_GUILDS;
      activeGuildId.value = activeGuildId.value ?? STUB_GUILDS[0]?.id ?? null;
      hydrated.value = true;
      lastFetchedAt.value = Date.now();
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : 'Failed to hydrate guilds';
    } finally {
      loading.value = false;
    }
  }

  function setActiveGuild(guildId: string) {
    if (!guilds.value.some((guild) => guild.id === guildId)) {
      error.value = `Unknown guild: ${guildId}`;
      return;
    }

    activeGuildId.value = guildId;
    error.value = null;
  }

  return {
    // state
    guilds,
    activeGuildId,
    loading,
    error,
    hydrated,
    lastFetchedAt,
    activeGuild,
    // actions
    hydrate,
    setActiveGuild,
  };
});
