<template>
  <LayoutAppShell
    :guilds="demoGuilds"
    :channels="demoChannels"
    :status-text="statusText"
    :latency-ms="latencyMs"
    :current-user="currentUser"
  >
    <NuxtPage />
  </LayoutAppShell>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const runtimeConfig = useRuntimeConfig();

const statusText = computed(
  () => `API: ${runtimeConfig.public.apiBaseUrl ?? 'not configured'}`,
);

const latencyMs = 42;

const currentUser = {
  name: 'Alice Rivers',
  status: 'online',
};

const demoGuilds = [
  { id: '1', name: 'OpenGuild Core', initials: 'OG', active: true },
  { id: '2', name: 'Frontend', initials: 'FE' },
  { id: '3', name: 'Backend', initials: 'BE' },
];

const demoChannels = [
  {
    id: 'ch-general',
    name: 'general',
    category: 'Text Channels',
    active: true,
  },
  {
    id: 'ch-builds',
    name: 'build-notes',
    category: 'Text Channels',
    unread: true,
  },
  {
    id: 'ch-admin',
    name: 'admin',
    category: 'Text Channels',
    locked: true,
  },
  {
    id: 'ch-voice',
    name: 'standup',
    category: 'Voice Channels',
  },
];
</script>
