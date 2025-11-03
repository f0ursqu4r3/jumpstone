<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'

import AppChannelSidebar from '~/components/app/AppChannelSidebar.vue'
import AppGuildRail from '~/components/app/AppGuildRail.vue'
import AppTopbar from '~/components/app/AppTopbar.vue'
import BaseButton from '~/components/ui/BaseButton.vue'
import { useChannelStore } from '~/stores/channels'
import { useGuildStore } from '~/stores/guilds'
import { useSessionStore } from '~/stores/session'

const guildStore = useGuildStore()
const {
  guilds: guildsRef,
  activeGuildId: activeGuildIdRef,
  activeGuild: activeGuildRef,
} = storeToRefs(guildStore)

const channelStore = useChannelStore()
const {
  channelsForGuild: channelsForGuildRef,
  activeChannel: activeChannelRef,
  activeChannelId: activeChannelIdRef,
} = storeToRefs(channelStore)

guildStore.hydrate()
channelStore.hydrate()

watch(
  () => activeGuildIdRef.value,
  (guildId) => {
    channelStore.setActiveGuild(guildId ?? null)
  },
  { immediate: true },
)

const guilds = computed(() =>
  guildsRef.value.map((guild) => ({
    ...guild,
    active: guild.id === activeGuildIdRef.value,
  })),
)
const channels = computed(() =>
  (channelsForGuildRef.value ?? []).map((channel) => ({
    ...channel,
    active: channel.id === activeChannelIdRef.value,
  })),
)
const activeGuild = computed(() => activeGuildRef.value ?? guildsRef.value[0])
const activeChannel = computed(() => activeChannelRef.value ?? channels.value[0])
const mobileSidebarOpen = ref(false)

const sessionStore = useSessionStore()
const { hydrated: hydratedRef, isAuthenticated: isAuthenticatedRef } = storeToRefs(sessionStore)
const route = useRoute()
const router = useRouter()
const hydrated = computed(() => hydratedRef.value)
const isAuthenticated = computed(() => isAuthenticatedRef.value)
const showAppShell = computed(() => hydrated.value && isAuthenticated.value)

const goToLogin = async () => {
  const redirect = route.path === '/login' ? null : route.fullPath
  await router.push(redirect ? { path: '/login', query: { redirect } } : { path: '/login' })
}

watch(
  showAppShell,
  (visible) => {
    if (!visible) {
      mobileSidebarOpen.value = false
    }
  },
  { flush: 'post' },
)
</script>

<template>
  <div class="relative flex h-screen overflow-hidden bg-slate-950">
    <AppGuildRail v-if="showAppShell" :guilds="guilds" />

    <AppChannelSidebar
      v-if="showAppShell"
      :guild-name="activeGuild?.name || ''"
      :channels="channels"
      class="hidden lg:flex"
    />

    <USlideover v-if="showAppShell" v-model="mobileSidebarOpen" side="left">
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

    <div class="flex h-full flex-1 flex-col" v-if="showAppShell">
      <AppTopbar
        :channel-name="activeChannel?.label || ''"
        :topic="activeChannel?.description || ''"
      />
      <main
        class="flex-1 overflow-y-auto bg-gradient-to-b from-slate-950 via-slate-950 to-slate-950/80"
      >
        <div class="mx-auto w-full max-w-4xl px-4 py-6 sm:px-6 lg:px-10">
          <slot />
        </div>
      </main>
      <footer class="border-t border-white/5 bg-slate-950/80 px-6 py-3 text-xs text-slate-500">
        Prototype UI - Federation awareness not yet connected
      </footer>
    </div>

    <div v-if="showAppShell" class="fixed left-4 top-4 z-40 flex items-center gap-2 lg:hidden">
      <BaseButton
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

    <div
      v-if="!hydrated"
      class="absolute inset-0 z-50 flex items-center justify-center bg-slate-950"
    >
      <div class="space-y-4 text-center">
        <USkeleton class="mx-auto h-12 w-12 rounded-full" />
        <div class="space-y-2">
          <USkeleton class="mx-auto h-4 w-56 rounded" />
          <USkeleton class="mx-auto h-4 w-40 rounded" />
        </div>
      </div>
    </div>

    <div
      v-else-if="!isAuthenticated"
      class="absolute inset-0 z-50 flex flex-col items-center justify-center bg-slate-950 px-6"
    >
      <div class="max-w-md space-y-6 text-center">
        <UIcon name="i-heroicons-lock-closed" class="mx-auto h-10 w-10 text-slate-500" />
        <div class="space-y-2">
          <h1 class="text-xl font-semibold text-white">Sign in to access OpenGuild</h1>
          <p class="text-sm text-slate-400">
            Your session expired or you have not signed in yet. Continue to the authentication
            portal to resume work.
          </p>
        </div>
        <div class="flex flex-col gap-3 sm:flex-row sm:justify-center">
          <UButton color="info" label="Go to sign in" @click="goToLogin" />
          <UButton to="/styleguide" variant="ghost" color="neutral" label="View styleguide" />
        </div>
      </div>
    </div>
  </div>
</template>
