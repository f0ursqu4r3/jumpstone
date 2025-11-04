<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { LocationQueryRaw } from 'vue-router'
import { storeToRefs } from 'pinia'

import AppChannelCreateModal from '@/components/app/AppChannelCreateModal.vue'
import AppChannelSidebar from '@/components/app/AppChannelSidebar.vue'
import AppGuildCreateModal from '@/components/app/AppGuildCreateModal.vue'
import AppGuildRail from '@/components/app/AppGuildRail.vue'
import AppTopbar from '@/components/app/AppTopbar.vue'
import Button from '@/components/ui/Button.vue'
import { extractErrorMessage } from '@/utils/errors'
import { deriveGuildPermissions, permissionGuidance, resolveGuildRole } from '@/utils/permissions'
import { useChannelStore } from '@/stores/channels'
import { useGuildStore } from '@/stores/guilds'
import { useSessionStore } from '@/stores/session'

const route = useRoute()
const router = useRouter()

const guildStore = useGuildStore()
const channelStore = useChannelStore()
const sessionStore = useSessionStore()

const {
  guilds: guildsRef,
  activeGuildId: activeGuildIdRef,
  activeGuild: activeGuildRef,
  loading: guildLoadingRef,
} = storeToRefs(guildStore)

const {
  channelsForGuild: channelsForGuildRef,
  activeChannel: activeChannelRef,
  activeChannelId: activeChannelIdRef,
  loading: channelLoadingRef,
} = storeToRefs(channelStore)

const {
  hydrated: hydratedRef,
  isAuthenticated: isAuthenticatedRef,
  profile: profileRef,
} = storeToRefs(sessionStore)

const mobileSidebarOpen = ref(false)
const ready = ref(false)
const syncingRoute = ref(false)
const showCreateGuildModal = ref(false)
const createGuildLoading = ref(false)
const createGuildError = ref<string | null>(null)
const showCreateChannelModal = ref(false)
const createChannelLoading = ref(false)
const createChannelError = ref<string | null>(null)

const formatCreatedAt = (iso?: string | null) => {
  if (!iso) {
    return null
  }

  const parsed = new Date(iso)
  if (Number.isNaN(parsed.getTime())) {
    return null
  }

  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
    }).format(parsed)
  } catch {
    return parsed.toLocaleDateString()
  }
}

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
    description: channel.description ?? formatCreatedAt(channel.createdAt) ?? undefined,
    icon:
      channel.icon ?? (channel.kind === 'voice' ? 'i-heroicons-speaker-wave' : 'i-heroicons-hashtag'),
  })),
)

const activeGuild = computed(() => activeGuildRef.value ?? guildsRef.value[0])
const activeChannel = computed(() => activeChannelRef.value ?? channels.value[0])
const hasGuilds = computed(() => guilds.value.length > 0)
const guildLoading = computed(() => guildLoadingRef.value)
const channelLoading = computed(() => channelLoadingRef.value)

const hydrated = computed(() => hydratedRef.value)
const isAuthenticated = computed(() => isAuthenticatedRef.value)
const showAppShell = computed(() => hydrated.value && isAuthenticated.value)

const sessionProfile = computed(() => profileRef.value)
const sessionGuilds = computed(() => sessionProfile.value?.guilds ?? [])
const platformRoles = computed(() => sessionProfile.value?.roles ?? [])
const activeGuildRole = computed(() => resolveGuildRole(activeGuildIdRef.value, sessionGuilds.value))
const guildPermissions = computed(() =>
  deriveGuildPermissions(activeGuildRole.value, platformRoles.value || []),
)
const canCreateChannel = computed(() => guildPermissions.value.canCreateChannels)
const createChannelDisabledReason = computed(() =>
  canCreateChannel.value ? '' : permissionGuidance('createChannels', guildPermissions.value),
)

const updateRouteQuery = (
  guildId?: string | null,
  channelId?: string | null,
  replace = false,
) => {
  if (syncingRoute.value) {
    return
  }

  const nextQuery: LocationQueryRaw = {}

  Object.entries(route.query).forEach(([key, value]) => {
    if (key === 'guild' || key === 'channel') {
      return
    }
    nextQuery[key] = value as string | string[] | null | undefined
  })

  if (guildId) {
    nextQuery.guild = guildId
  }

  if (channelId) {
    nextQuery.channel = channelId
  }

  const method = replace ? router.replace : router.push
  method({ path: route.path, query: nextQuery }).catch(() => {})
}

const syncFromRoute = async (options: { updateRoute: boolean }) => {
  syncingRoute.value = true
  try {
    if (!ready.value) {
      await guildStore.hydrate()
    }

    let targetGuildId =
      typeof route.query.guild === 'string' && route.query.guild.length
        ? route.query.guild
        : null

    if (targetGuildId) {
      await guildStore.setActiveGuild(targetGuildId).catch(() => undefined)
    } else if (!activeGuildIdRef.value) {
      const firstGuild = guildsRef.value[0]
      if (firstGuild) {
        await guildStore.setActiveGuild(firstGuild.id)
        targetGuildId = firstGuild.id
      } else {
        targetGuildId = null
      }
    } else {
      targetGuildId = activeGuildIdRef.value
    }

    await channelStore.setActiveGuild(targetGuildId ?? null)

    let targetChannelId =
      typeof route.query.channel === 'string' && route.query.channel.length
        ? route.query.channel
        : null

    if (targetChannelId) {
      await channelStore.setActiveChannel(targetChannelId).catch(() => undefined)
    } else {
      targetChannelId = activeChannelIdRef.value ?? null
    }

    if (options.updateRoute) {
      updateRouteQuery(targetGuildId, targetChannelId, true)
    }
  } finally {
    syncingRoute.value = false
  }
}

const initialize = async () => {
  await syncFromRoute({ updateRoute: true })
  ready.value = true
}

initialize().catch((err) => {
  console.warn('Failed to initialise layout routing', err)
})

watch(
  () => route.query,
  async () => {
    if (!ready.value || syncingRoute.value) {
      return
    }

    await syncFromRoute({ updateRoute: false })
  },
  { deep: true },
)

watch(
  () => [activeGuildIdRef.value, activeChannelIdRef.value] as const,
  ([guildId, channelId]) => {
    if (!ready.value) {
      return
    }

    updateRouteQuery(guildId ?? null, channelId ?? null, true)
  },
)

watch(
  () => activeGuildIdRef.value,
  async (guildId) => {
    if (syncingRoute.value) {
      return
    }

    await channelStore.setActiveGuild(guildId ?? null)
  },
)

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

const handleGuildSelect = async (guildId: string) => {
  const targetGuildId = guildId || guildsRef.value[0]?.id || null
  if (!targetGuildId) {
    return
  }

  await guildStore.setActiveGuild(targetGuildId)
  await channelStore.setActiveGuild(targetGuildId)
  updateRouteQuery(targetGuildId, activeChannelIdRef.value ?? null, true)
}

const handleChannelSelect = async (channelId: string) => {
  await channelStore.setActiveChannel(channelId)
  updateRouteQuery(activeGuildIdRef.value ?? null, activeChannelIdRef.value ?? null, true)
  mobileSidebarOpen.value = false
}

const handleCreateChannel = () => {
  if (!canCreateChannel.value) {
    createChannelError.value = permissionGuidance('createChannels', guildPermissions.value)
    return
  }
  createChannelError.value = null
  showCreateChannelModal.value = true
}

const handleOpenGuildMenu = () => {
  console.info('Guild settings menu not yet implemented')
}

const handleCreateGuild = () => {
  createGuildError.value = null
  showCreateGuildModal.value = true
}

const submitCreateGuild = async (name: string) => {
  const trimmed = name.trim()
  if (!trimmed) {
    createGuildError.value = 'Guild name is required.'
    return
  }

  createGuildLoading.value = true
  createGuildError.value = null

  try {
    const record = await guildStore.createGuild(trimmed)
    await guildStore.setActiveGuild(record.guild_id)
    await channelStore.setActiveGuild(record.guild_id)
    updateRouteQuery(record.guild_id, activeChannelIdRef.value ?? null, true)
    showCreateGuildModal.value = false
  } catch (err) {
    createGuildError.value = extractErrorMessage(err) || 'Unable to create guild.'
  } finally {
    createGuildLoading.value = false
  }
}

const resetCreateGuildError = () => {
  createGuildError.value = null
}

const submitCreateChannel = async (name: string) => {
  const guildId = activeGuildIdRef.value
  if (!guildId) {
    createChannelError.value = 'Select a guild first.'
    return
  }

  const trimmed = name.trim()
  if (!trimmed) {
    createChannelError.value = 'Channel name is required.'
    return
  }

  createChannelLoading.value = true
  createChannelError.value = null

  try {
    const record = await channelStore.createChannel(guildId, trimmed)
    await channelStore.setActiveChannel(record.channel_id)
    updateRouteQuery(guildId, record.channel_id, true)
    showCreateChannelModal.value = false
  } catch (err) {
    createChannelError.value = extractErrorMessage(err) || 'Unable to create channel.'
  } finally {
    createChannelLoading.value = false
  }
}

const resetCreateChannelError = () => {
  createChannelError.value = null
}
</script>

<template>
  <div class="relative flex h-screen overflow-hidden bg-slate-950">
    <AppGuildRail
      v-if="showAppShell"
      :guilds="guilds"
      :loading="guildLoading"
      @select="handleGuildSelect"
      @create="handleCreateGuild"
      @open-menu="handleOpenGuildMenu"
    />

    <AppChannelSidebar
      v-if="showAppShell && hasGuilds"
      :guild-name="activeGuild?.name || ''"
      :channels="channels"
      :loading="channelLoading"
      :can-create-channel="canCreateChannel && !createChannelLoading"
      :create-channel-disabled-reason="createChannelDisabledReason"
      class="hidden lg:flex"
      @select-channel="handleChannelSelect"
      @create-channel="handleCreateChannel"
      @open-guild-settings="handleOpenGuildMenu"
    />

    <USlideover v-if="showAppShell && hasGuilds" v-model="mobileSidebarOpen" side="left">
      <template #content>
        <div class="flex h-full w-[18rem] flex-col bg-slate-950">
          <AppChannelSidebar
            :guild-name="activeGuild?.name || ''"
            :channels="channels"
            :loading="channelLoading"
            :can-create-channel="canCreateChannel && !createChannelLoading"
            :create-channel-disabled-reason="createChannelDisabledReason"
            class="flex"
            @select-channel="handleChannelSelect"
            @create-channel="handleCreateChannel"
            @open-guild-settings="handleOpenGuildMenu"
          />
        </div>
      </template>
    </USlideover>

    <div class="flex h-full flex-1 flex-col" v-if="showAppShell">
      <template v-if="hasGuilds">
        <AppTopbar
          :channel-name="activeChannel?.label || ''"
          :topic="activeChannel?.description || ''"
        />
        <main
          class="flex-1 overflow-y-auto bg-linear-to-b from-slate-950 via-slate-950 to-slate-950/80"
        >
          <div class="mx-auto w-full max-w-4xl px-4 py-6 sm:px-6 lg:px-10">
            <slot />
          </div>
        </main>
      </template>
      <template v-else>
        <main
          class="flex flex-1 items-center justify-center bg-linear-to-b from-slate-950 via-slate-950 to-slate-950/80 px-4"
        >
          <UCard class="w-full max-w-lg border border-white/5 bg-slate-950/70">
            <template #header>
              <div class="space-y-1">
                <h2 class="text-lg font-semibold text-white">No guilds yet</h2>
                <p class="text-sm text-slate-400">
                  Create your first guild to unlock channels, timelines, and collaboration spaces.
                </p>
              </div>
            </template>
            <div class="space-y-4 text-sm text-slate-300">
              <p>
                Use the button on the left rail to add a guild. Once created, you will be able to
                add channels and invite teammates.
              </p>
              <UButton color="info" @click="handleCreateGuild" icon="i-heroicons-plus">
                Create guild
              </UButton>
            </div>
          </UCard>
        </main>
      </template>
      <footer class="border-t border-white/5 bg-slate-950/80 px-6 py-3 text-xs text-slate-500">
        Prototype UI - Federation awareness not yet connected
      </footer>
    </div>

    <div
      v-if="showAppShell && hasGuilds"
      class="fixed left-4 top-4 z-40 flex items-center gap-2 lg:hidden"
    >
      <Button
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

  <AppGuildCreateModal
    v-model:open="showCreateGuildModal"
    :loading="createGuildLoading"
    :error="createGuildError"
    @submit="submitCreateGuild"
    @reset-error="resetCreateGuildError"
  />
  <AppChannelCreateModal
    v-model:open="showCreateChannelModal"
    :loading="createChannelLoading"
    :error="createChannelError"
    @submit="submitCreateChannel"
    @reset-error="resetCreateChannelError"
  />
</template>
