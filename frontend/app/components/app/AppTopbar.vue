<script setup lang="ts">
import { computed } from 'vue';
import { useSessionStore } from '~/stores/session';

const props = defineProps<{
  channelName: string;
  topic?: string;
}>();

const channelName = computed(() => props.channelName);
const sessionStore = useSessionStore();
const isAuthenticated = computed(() => sessionStore.isAuthenticated);

const handleAccountAction = async () => {
  if (isAuthenticated.value) {
    sessionStore.logout();
  }

  await navigateTo('/login');
};
</script>

<template>
  <header
    class="flex items-center justify-between border-b border-white/5 bg-slate-950/70 p-2 backdrop-blur"
  >
    <div class="flex items-baseline gap-4">
      <div>
        <div class="flex items-center gap-2">
          <UIcon name="i-heroicons-hashtag" class="h-5 w-5 text-slate-400" />
          <h1 class="text-lg font-semibold text-white">
            {{ channelName }}
          </h1>
        </div>
        <p v-if="props.topic" class="mt-1 text-xs text-slate-400">
          {{ props.topic }}
        </p>
      </div>
    </div>
    <div class="flex items-center gap-3">
      <UInput
        placeholder="Search"
        icon="i-heroicons-magnifying-glass-20-solid"
        color="neutral"
        variant="soft"
        class="w-64 hidden lg:block"
      />
      <UButton
        icon="i-heroicons-bell-alert"
        color="neutral"
        variant="ghost"
        aria-label="Notifications"
      />
      <UButton
        icon="i-heroicons-queue-list"
        color="neutral"
        variant="ghost"
        aria-label="Inbox"
      />
      <UButton
        v-if="!isAuthenticated"
        color="info"
        variant="soft"
        label="Sign in"
        @click="handleAccountAction"
      />
      <UButton
        v-else
        color="neutral"
        variant="ghost"
        icon="i-heroicons-arrow-left-on-rectangle"
        label="Sign out"
        @click="handleAccountAction"
      />
    </div>
  </header>
</template>
