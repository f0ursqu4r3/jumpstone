<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'

import AppMessageComposer from '@/components/app/AppMessageComposer.vue'
import AppMessageTimeline from '@/components/app/AppMessageTimeline.vue'
import { getRuntimeConfig } from '@/config/runtime'
import { useChannelStore } from '@/stores/channels'
import { useConnectivityStore } from '@/stores/connectivity'
import { useGuildStore } from '@/stores/guilds'
import { useMessageComposerStore } from '@/stores/messages'
import { useSessionStore } from '@/stores/session'
import { useSystemStore } from '@/stores/system'
import { useTimelineStore } from '@/stores/timeline'
import { useRealtimeStore } from '@/stores/realtime'
import type { ComponentStatus } from '@/types/api'

const timelineEntries = [
  {
    id: 'guild-sync',
    title: 'Guild roster hydrates from /guilds',
    author: 'Lia Chen',
    time: 'Today · 09:15',
    summary:
      'Pinia guild store now sources data from the backend and updates the rail instantly. The Vue layout syncs query params so deep links land on the right workspace.',
    tag: 'Week 4',
  },
  {
    id: 'channel-sidebar',
    title: 'Channel sidebar switches via store wiring',
    author: 'Maya Singh',
    time: 'Today · 08:42',
    summary:
      'Channel store fetches `/guilds/{guild_id}/channels` on selection, highlights unread stubs, and disables CTA buttons while loading.',
    tag: 'UI Shell',
  },
  {
    id: 'timeline-fetch',
    title: 'Timeline reads from /channels/{id}/events',
    author: 'Kai Patel',
    time: 'Yesterday · 17:05',
    summary:
      'The new AppMessageTimeline component renders canonical events, groups by day, and surfaces refresh actions for manual QA runs.',
    tag: 'Messaging',
  },
] as const

const upcomingTasks = [
  {
    id: 'guild-create-modal',
    label: 'Guild creation modal (POST /guilds)',
    owner: 'lia',
    status: 'Planned',
  },
  {
    id: 'channel-empty-states',
    label: 'Channel empty and invite-only states',
    owner: 'maya',
    status: 'In progress',
  },
  {
    id: 'timeline-virtualize',
    label: 'Virtualize timeline for >200 events',
    owner: 'kai',
    status: 'Backlog',
  },
] as const

const channelStore = useChannelStore()
const guildStore = useGuildStore()
const timelineStore = useTimelineStore()
const realtimeStore = useRealtimeStore()
const messageComposerStore = useMessageComposerStore()
const connectivityStore = useConnectivityStore()

const realtimeStatus = realtimeStore.status
const realtimeAttemptingReconnect = realtimeStore.attemptingReconnect

const {
  activeChannelId: activeChannelIdRef,
  activeChannel: activeChannelRef,
  channelsForGuild: channelsForGuildRef,
  loading: channelStoreLoadingRef,
} = storeToRefs(channelStore)

const { activeGuild: activeGuildRef } = storeToRefs(guildStore)

const {
  eventsByChannel: eventsByChannelRef,
  loadingByChannel: loadingByChannelRef,
  errorByChannel: errorByChannelRef,
} = storeToRefs(timelineStore)

const { degradedMessage: degradedMessageRef } = storeToRefs(connectivityStore)

const degradedMessage = computed(() => degradedMessageRef.value)
const activeChannelId = computed(() => activeChannelIdRef.value ?? null)

const typingPreview = ref<string | null>(null)
let typingPreviewTimer: ReturnType<typeof setTimeout> | null = null

const loadedChannels = new Set<string>()

watch(
  () => activeChannelIdRef.value,
  async (channelId) => {
    if (!channelId) {
      return
    }

    const options = loadedChannels.has(channelId)
      ? { refresh: true }
      : { force: true }

    try {
      await timelineStore.loadChannel(channelId, options)
      loadedChannels.add(channelId)
    } catch (err) {
      console.warn('Failed to load channel timeline', err)
    }
  },
  { immediate: true },
)

watch(
  () => activeChannelIdRef.value,
  (channelId) => {
    realtimeStore.connect(channelId ?? null)
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  realtimeStore.disconnect()
  if (typingPreviewTimer) {
    clearTimeout(typingPreviewTimer)
    typingPreviewTimer = null
  }
})

const timelineEvents = computed(() => {
  const channelId = activeChannelIdRef.value
  if (!channelId) {
    return []
  }
  return eventsByChannelRef.value[channelId] ?? []
})

const timelineLoading = computed(() => {
  const channelId = activeChannelIdRef.value
  if (!channelId) {
    return false
  }
  return Boolean(loadingByChannelRef.value[channelId])
})

const timelineError = computed(() => {
  const channelId = activeChannelIdRef.value
  if (!channelId) {
    return null
  }
  return errorByChannelRef.value[channelId] ?? null
})

const activeChannelName = computed(() => activeChannelRef.value?.label ?? '')
const activeGuildName = computed(() => activeGuildRef.value?.name ?? '—')
const hasChannels = computed(
  () => (channelsForGuildRef.value ? channelsForGuildRef.value.length > 0 : false),
)
const channelListLoading = computed(() => channelStoreLoadingRef.value)
const composerDisabled = computed(() => !activeChannelId.value || channelListLoading.value)

const latestSequenceLabel = computed(() => {
  const events = timelineEvents.value
  const lastEvent = events[events.length - 1]
  if (!lastEvent) {
    return 'No events yet'
  }
  return `Seq ${lastEvent.sequence}`
})

const refreshTimeline = async () => {
  const channelId = activeChannelIdRef.value
  if (!channelId) {
    return
  }

  try {
    await timelineStore.loadChannel(channelId, { refresh: true, force: true })
    loadedChannels.add(channelId)
  } catch (err) {
    console.warn('Failed to refresh timeline', err)
  }
}

const handleRetryOptimistic = async (localId: string) => {
  try {
    await messageComposerStore.retryOptimistic(localId)
  } catch (err) {
    console.warn('Failed to retry optimistic message', err)
  }
}

const handleTyping = ({
  channelId,
  preview,
}: {
  channelId: string | null
  preview: string
}) => {
  if (channelId !== activeChannelId.value) {
    return
  }

  if (typingPreviewTimer) {
    clearTimeout(typingPreviewTimer)
    typingPreviewTimer = null
  }

  const trimmed = preview.trim()
  if (!trimmed.length) {
    typingPreview.value = null
    return
  }

  typingPreview.value = trimmed
  void realtimeStore.sendTypingPreview(channelId, trimmed)
  typingPreviewTimer = setTimeout(() => {
    typingPreview.value = null
    typingPreviewTimer = null
  }, 3000)
}

const quickMetrics = computed(() => [
  {
    label: 'Active guild',
    value: activeGuildName.value,
    trend: 'Synced via /guilds',
  },
  {
    label: 'Active channel',
    value: activeChannelName.value ? `#${activeChannelName.value}` : '—',
    trend: 'Channel store hydrated',
  },
  {
    label: 'Events loaded',
    value: String(timelineEvents.value.length),
    trend: latestSequenceLabel.value,
  },
])

const runtimeConfig = getRuntimeConfig()

const apiBaseHost = computed(() => {
  const base = runtimeConfig.public.apiBaseUrl
  if (!base) {
    return ''
  }

  try {
    return new URL(base).host
  } catch {
    return base
  }
})

const systemStore = useSystemStore()
const {
  readiness: readinessRef,
  status: statusRef,
  components: componentsRef,
  uptimeSeconds: uptimeSecondsRef,
  loading: loadingRef,
  error: errorRef,
  version: versionRef,
} = storeToRefs(systemStore)

const sessionStore = useSessionStore()
const {
  profile: profileRef,
  profileLoading: profileLoadingRef,
  profileError: profileErrorRef,
  displayName: displayNameRef,
  profileAvatar: profileAvatarRef,
  identifier: identifierRef,
  isAuthenticated: isAuthenticatedRef,
} = storeToRefs(sessionStore)

if (!readinessRef.value) {
  systemStore.fetchBackendStatus()
}

if (
  typeof window !== 'undefined' &&
  isAuthenticatedRef.value &&
  !profileRef.value &&
  !profileLoadingRef.value
) {
  sessionStore.fetchProfile().catch((err) => {
    console.warn('Failed to preload session profile', err)
  })
}

const backendPending = computed(() => loadingRef.value)
const backendError = computed(() => errorRef.value)
const backendErrorMessage = computed(() => errorRef.value ?? '')

const refreshBackend = () => systemStore.fetchBackendStatus(true)

const readinessStatus = computed(() => statusRef.value)
const readinessBadgeColor = computed(() => {
  if (readinessStatus.value === 'ready') {
    return 'success'
  }
  if (readinessStatus.value === 'degraded') {
    return 'warning'
  }
  return 'neutral'
})

const readinessStatusLabel = computed(() => {
  const label = readinessStatus.value ?? 'unknown'
  return label.charAt(0).toUpperCase() + label.slice(1)
})

const backendVersion = computed(() => versionRef.value ?? '—')

const componentStatuses = computed<ComponentStatus[]>(() => componentsRef.value ?? [])

const componentBadgeColor = (status: string) => {
  if (status === 'configured') {
    return 'success'
  }
  if (status === 'pending') {
    return 'warning'
  }
  if (status === 'error') {
    return 'error'
  }
  return 'neutral'
}

const componentStatusLabel = (status: string) => status.charAt(0).toUpperCase() + status.slice(1)

const formatDuration = (totalSeconds: number | null | undefined) => {
  if (typeof totalSeconds !== 'number' || !Number.isFinite(totalSeconds)) {
    return '—'
  }

  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = Math.floor(totalSeconds % 60)

  const segments: string[] = []
  if (hours) {
    segments.push(`${hours}h`)
  }
  if (minutes) {
    segments.push(`${minutes}m`)
  }
  if (!segments.length) {
    segments.push(`${seconds}s`)
  }

  return segments.join(' ')
}

const uptime = computed(() => formatDuration(uptimeSecondsRef.value))

const sessionProfile = computed(() => profileRef.value)
const sessionProfileLoading = computed(() => profileLoadingRef.value)
const sessionProfileError = computed(() => profileErrorRef.value)
const sessionDisplayName = computed(() => displayNameRef.value || identifierRef.value || '—')
const sessionUsername = computed(() => sessionProfile.value?.username ?? identifierRef.value ?? '—')
const formatDateTime = (iso: string | null | undefined) => {
  if (!iso) {
    return '—'
  }

  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) {
    return '—'
  }

  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(date)
  } catch {
    return date.toLocaleString()
  }
}
const profileCreatedAt = computed(() => formatDateTime(sessionProfile.value?.createdAt))
const profileUpdatedAt = computed(() => formatDateTime(sessionProfile.value?.updatedAt))
const sessionServerName = computed(() => {
  const serverHint = sessionProfile.value?.serverName
  if (serverHint && serverHint.length) {
    return serverHint
  }

  const host = apiBaseHost.value
  if (host) {
    return host
  }

  return 'Local server'
})
const sessionGuilds = computed(() => sessionProfile.value?.guilds ?? [])
const sessionDevices = computed(() => sessionProfile.value?.devices ?? [])
const sessionAvatarUrl = computed(() => {
  const custom = profileAvatarRef.value
  if (custom) {
    return custom
  }
  const seed = sessionDisplayName.value || sessionUsername.value || 'OpenGuild'
  return `https://api.dicebear.com/7.x/initials/svg?seed=${encodeURIComponent(seed)}`
})
const sessionMetadata = computed(() => [
  { label: 'Server', value: sessionServerName.value || '—' },
  {
    label: 'Default guild',
    value: sessionProfile.value?.defaultGuildId ?? '—',
  },
  { label: 'Locale', value: sessionProfile.value?.locale ?? '—' },
  { label: 'Timezone', value: sessionProfile.value?.timezone ?? '—' },
  { label: 'Created', value: profileCreatedAt.value },
  { label: 'Updated', value: profileUpdatedAt.value },
])

const refreshProfile = async () => {
  try {
    await sessionStore.fetchProfile(true)
  } catch (err) {
    console.warn('Failed to refresh session profile', err)
  }
}
</script>

<template>
  <div class="space-y-10">
    <section
      class="relative overflow-hidden rounded-3xl border border-slate-800/50 bg-linear-to-br from-slate-900 via-slate-950 to-slate-950/60 px-8 py-10 shadow-xl shadow-slate-950/40"
    >
      <div class="relative z-10 max-w-3xl space-y-4">
        <UBadge variant="soft" color="info" label="Milestone F0 · Week 4" />
        <h1 class="text-3xl font-semibold text-white sm:text-4xl">
          Guild and channel shell syncing from the backend
        </h1>
        <p class="text-base text-slate-300 sm:text-lg">
          Pinia stores now hydrate from the Axum APIs. Pick a channel to stream recent events,
          verify the payloads, and update the docs as Week&nbsp;4 work lands.
        </p>
        <div class="flex flex-wrap gap-3 pt-2">
          <UButton
            icon="i-heroicons-rocket-launch"
            color="info"
            label="Open roadmap"
            to="/roadmap"
            variant="solid"
          />
          <UButton
            icon="i-heroicons-academic-cap"
            color="neutral"
            label="Developer setup"
            to="https://github.com/openguild"
            target="_blank"
            variant="ghost"
          />
          <UButton
            icon="i-heroicons-swatch"
            color="neutral"
            label="View styleguide"
            to="/styleguide"
            variant="ghost"
          />
        </div>
      </div>
      <div
        class="pointer-events-none absolute -right-20 -top-20 h-96 w-96 rounded-full bg-sky-500/10 blur-3xl"
      />
    </section>

    <section class="grid gap-6 lg:grid-cols-[2fr_1fr]">
      <div class="space-y-6">
        <UAlert
          v-if="!hasChannels && !channelListLoading"
          color="neutral"
          variant="soft"
          icon="i-heroicons-lock-closed"
          title="Invite-only"
        >
          <template #description>
            No channels are visible yet. Create a channel or request access if this guild is
            invite-only.
          </template>
        </UAlert>

        <UAlert
          v-else-if="degradedMessage"
          color="warning"
          variant="soft"
          icon="i-heroicons-exclamation-triangle"
          title="Connectivity notice"
          :description="degradedMessage"
        />

        <AppMessageTimeline
          :channel-name="activeChannelName"
          :events="timelineEvents"
          :loading="timelineLoading"
          :error="timelineError"
          @refresh="refreshTimeline"
          @retry="handleRetryOptimistic"
        />

        <div
          v-if="typingPreview"
          class="flex flex-wrap items-center gap-2 rounded-2xl border border-sky-500/10 bg-sky-500/5 px-4 py-2 text-xs text-slate-300"
        >
          <UIcon name="i-heroicons-pencil-square" class="h-4 w-4 text-sky-300" />
          <span class="font-semibold text-sky-200">Draft preview</span>
          <span>·</span>
          <span class="truncate">{{ typingPreview }}</span>
        </div>

        <AppMessageComposer
          :channel-id="activeChannelId"
          :channel-name="activeChannelName"
          :realtime-status="realtimeStatus"
          :attempting-reconnect="realtimeAttemptingReconnect"
          :disabled="composerDisabled"
          @typing="handleTyping"
        />
      </div>

      <div class="space-y-6">
        <UCard class="border border-white/5 bg-slate-950/60">
          <template #header>
            <div class="flex items-center justify-between">
              <h2 class="text-lg font-semibold text-white">Week-in-progress</h2>
              <UButton icon="i-heroicons-sparkles" color="neutral" variant="ghost" />
            </div>
          </template>
          <div class="space-y-4">
            <div
              v-for="metric in quickMetrics"
              :key="metric.label"
              class="rounded-xl border border-white/5 bg-slate-900/60 p-3"
            >
              <p class="text-xs uppercase tracking-wide text-slate-500">{{ metric.label }}</p>
              <p class="text-lg font-semibold text-white">
                {{ metric.value }}
              </p>
              <p class="text-xs text-slate-400">{{ metric.trend }}</p>
            </div>
          </div>
        </UCard>

        <UCard class="border border-white/5 bg-slate-950/60">
          <template #header>
            <div class="flex items-center justify-between">
              <h2 class="text-lg font-semibold text-white">Upcoming tasks</h2>
              <UButton icon="i-heroicons-clipboard-document-check" color="neutral" variant="ghost" />
            </div>
          </template>
          <div class="space-y-4">
            <div
              v-for="item in upcomingTasks"
              :key="item.id"
              class="flex items-center justify-between rounded-xl border border-white/5 bg-slate-900/60 px-3 py-2"
            >
              <div>
                <p class="text-sm font-semibold text-white">{{ item.label }}</p>
                <p class="text-xs text-slate-500">Owner: {{ item.owner }}</p>
              </div>
              <UBadge variant="soft" color="neutral" :label="item.status" />
            </div>
          </div>
        </UCard>
      </div>
    </section>

    <section class="grid gap-6 lg:grid-cols-[2fr_1fr]">
      <UCard class="border border-white/5 bg-slate-950/60">
        <template #header>
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-lg font-semibold text-white">Week 4 delivery log</h2>
              <p class="text-sm text-slate-400">Highlights from the guild and channel rollout.</p>
            </div>
            <UButton icon="i-heroicons-arrow-path" color="neutral" variant="ghost" aria-label="Refresh feed" />
          </div>
        </template>
        <div class="space-y-8">
          <div v-for="item in timelineEntries" :key="item.id" class="relative pl-8">
            <span class="absolute left-0 top-1 h-2.5 w-2.5 rounded-full bg-sky-400 ring-4 ring-sky-500/20" />
            <div class="flex flex-wrap items-center gap-3">
              <p class="text-sm font-semibold text-white">
                {{ item.title }}
              </p>
              <UBadge variant="soft" color="neutral" :label="item.tag" />
              <span class="text-xs text-slate-500">
                {{ item.time }}
              </span>
            </div>
            <p class="mt-3 text-sm leading-relaxed text-slate-300">
              {{ item.summary }}
            </p>
            <p class="mt-2 text-xs font-medium text-slate-500">Posted by {{ item.author }}</p>
          </div>
        </div>
      </UCard>

      <UCard class="border border-white/5 bg-slate-950/60">
        <template #header>
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-lg font-semibold text-white">Session overview</h2>
              <p class="text-sm text-slate-400">Active on {{ sessionServerName }}</p>
            </div>
            <UButton
              icon="i-heroicons-arrow-path"
              color="neutral"
              variant="ghost"
              :loading="sessionProfileLoading"
              @click="refreshProfile()"
              aria-label="Refresh profile"
            />
          </div>
        </template>

        <div v-if="sessionProfileLoading" class="space-y-4">
          <div class="flex items-center gap-3">
            <USkeleton class="h-12 w-12 rounded-full" />
            <div class="flex-1 space-y-2">
              <USkeleton class="h-4 w-32 rounded" />
              <USkeleton class="h-3 w-24 rounded" />
            </div>
          </div>
          <div class="grid gap-3 sm:grid-cols-2">
            <USkeleton class="h-3 w-24 rounded" />
            <USkeleton class="h-3 w-28 rounded" />
            <USkeleton class="h-3 w-20 rounded" />
            <USkeleton class="h-3 w-32 rounded" />
          </div>
        </div>

        <div v-else-if="sessionProfileError" class="space-y-4">
          <UAlert
            color="warning"
            variant="soft"
            title="Unable to load profile"
            :description="sessionProfileError"
          />
          <p class="text-xs text-slate-500">
            Check that the `/users/me` endpoint is reachable and that your session token is still
            valid.
          </p>
        </div>

        <div v-else class="space-y-5">
          <div class="flex items-center gap-4">
            <UAvatar :name="sessionDisplayName" :src="sessionAvatarUrl" size="lg" />
            <div class="space-y-1 text-left">
              <p class="text-sm font-semibold text-white">
                {{ sessionDisplayName }}
              </p>
              <p class="text-xs text-slate-400">
                {{ sessionUsername }}
              </p>
            </div>
          </div>

          <div class="grid gap-4 sm:grid-cols-2">
            <div v-for="item in sessionMetadata" :key="item.label" class="space-y-1">
              <p class="text-xs uppercase tracking-wide text-slate-500">
                {{ item.label }}
              </p>
              <p class="text-sm font-medium text-white">
                {{ item.value || '—' }}
              </p>
            </div>
          </div>

          <div class="space-y-3">
            <p class="text-xs uppercase tracking-wide text-slate-500">Guild access</p>
            <div v-if="!sessionGuilds.length" class="text-xs text-slate-500">
              No guild membership reported yet. Connect to the backend to hydrate this list.
            </div>
            <ul v-else class="space-y-2">
              <li
                v-for="guild in sessionGuilds"
                :key="guild.guildId"
                class="flex items-center justify-between rounded-md bg-slate-900/80 px-3 py-2 text-sm text-slate-200"
              >
                <span>{{ guild.name || guild.guildId }}</span>
                <UBadge v-if="guild.role" color="info" variant="soft" :label="guild.role" />
              </li>
            </ul>
          </div>

          <div class="space-y-3">
            <p class="text-xs uppercase tracking-wide text-slate-500">Devices</p>
            <div v-if="!sessionDevices.length" class="text-xs text-slate-500">
              Refresh token store not populated yet. This will display device metadata once the
              backend exposes session inventory.
            </div>
            <ul v-else class="space-y-2">
              <li
                v-for="device in sessionDevices"
                :key="device.deviceId"
                class="rounded-md bg-slate-900/80 px-3 py-2 text-xs text-slate-300"
              >
                <div class="flex items-center justify-between gap-3">
                  <span class="font-medium">{{ device.deviceName || device.deviceId }}</span>
                  <span v-if="device.lastSeenAt" class="text-[10px] text-slate-500">
                    Last seen {{ formatDateTime(device.lastSeenAt) }}
                  </span>
                </div>
                <p v-if="device.ipAddress" class="text-[10px] text-slate-500">
                  IP: {{ device.ipAddress }}
                </p>
              </li>
            </ul>
          </div>
        </div>
      </UCard>
    </section>
  </div>
</template>
