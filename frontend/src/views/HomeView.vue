<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'

import AppDeviceBootstrapModal from '@/components/app/AppDeviceBootstrapModal.vue'
import AppGlobalSearchModal from '@/components/app/AppGlobalSearchModal.vue'
import AppMessageComposer from '@/components/app/AppMessageComposer.vue'
import AppMessageTimeline from '@/components/timeline/AppMessageTimeline.vue'
import { GuildSurfaceCard } from '@/components/primitives'
import { getFeatureFlags } from '@/config/features'
import { getRuntimeConfig } from '@/config/runtime'
import { useChannelStore } from '@/stores/channels'
import { useConnectivityStore } from '@/stores/connectivity'
import { useSessionDevicesStore } from '@/stores/devices'
import { useGuildStore } from '@/stores/guilds'
import { useMessageComposerStore } from '@/stores/messages'
import { useMlsStore } from '@/stores/mls'
import { useSessionStore } from '@/stores/session'
import { useSystemStore } from '@/stores/system'
import { useTimelineStore } from '@/stores/timeline'
import { useRealtimeStore } from '@/stores/realtime'
import { useFederationStore } from '@/stores/federation'
import {
  deriveGuildPermissions,
  permissionGuidance,
  resolveChannelRole,
  resolveGuildRole,
} from '@/utils/permissions'
import { recordBreadcrumb } from '@/utils/telemetry'

const timelineEntries = [
  {
    id: 'mls-key-packages',
    title: 'MLS key packages surface in dashboard',
    author: 'Lia Chen',
    time: 'Today · 09:15',
    summary:
      'HomeView now fetches `/mls/key-packages`, badges rotation timestamps, and lets operators copy HPKE + signature keys with telemetry breadcrumbs.',
    tag: 'Week 9',
  },
  {
    id: 'device-bootstrap-modal',
    title: 'Device bootstrap modal ships for MLS prep',
    author: 'Maya Singh',
    time: 'Today · 08:42',
    summary:
      'A guided modal walks admins through naming devices, running the CLI, and verifying handshake vectors so new hardware lands cleanly.',
    tag: 'Device Prep',
  },
  {
    id: 'handshake-persistence',
    title: 'Handshake verification stored locally',
    author: 'Kai Patel',
    time: 'Yesterday · 17:05',
    summary:
      'Fetching handshake test vectors now records the verification timestamp, muting repeated prompts until the TTL expires.',
    tag: 'Federation',
  },
] as const

const upcomingTasks = [
  {
    id: 'mls-rotation-ui',
    label: 'Expose key package rotation actions',
    owner: 'lia',
    status: 'Planned',
  },
  {
    id: 'device-enrolment-api',
    label: 'Wire MLS enrolment endpoint once available',
    owner: 'maya',
    status: 'Researching',
  },
  {
    id: 'mls-telemetry',
    label: 'Expand MLS readiness telemetry dashboards',
    owner: 'kai',
    status: 'Backlog',
  },
] as const

const HANDSHAKE_REVIEW_TTL_MS = 12 * 60 * 60 * 1000

const channelStore = useChannelStore()
const guildStore = useGuildStore()
const timelineStore = useTimelineStore()
const realtimeStore = useRealtimeStore()
const messageComposerStore = useMessageComposerStore()
const connectivityStore = useConnectivityStore()
const sessionDevicesStore = useSessionDevicesStore()
const federationStore = useFederationStore()
const mlsStore = useMlsStore()

const featureFlags = getFeatureFlags()

const realtimeStatus = realtimeStore.status
const realtimeAttemptingReconnect = realtimeStore.attemptingReconnect

const {
  activeChannelId: activeChannelIdRef,
  activeChannel: activeChannelRef,
  channelsForGuild: channelsForGuildRef,
  loading: channelStoreLoadingRef,
} = storeToRefs(channelStore)

const { activeGuild: activeGuildRef } = storeToRefs(guildStore)

const activeGuildId = computed(() => activeGuildRef.value?.id ?? null)

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

    const options = loadedChannels.has(channelId) ? { refresh: true } : { force: true }

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

watch(
  () => activeGuildId.value,
  (guildId) => {
    if (!guildId) {
      return
    }
    federationStore.fetchContext(guildId).catch((err) => {
      console.warn('Failed to load federation context', err)
    })
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

const normalizeHost = (value: string | null | undefined) => {
  if (!value) {
    return null
  }
  try {
    const sanitized = value.replace(/^https?:\/\//, '').split('/')[0] ?? value
    return sanitized.split(':')[0]?.toLowerCase() ?? sanitized.toLowerCase()
  } catch {
    return value.toLowerCase()
  }
}

const localOriginHost = computed(() => {
  if (sessionServerName.value && sessionServerName.value !== 'Local server') {
    return sessionServerName.value
  }
  const base = apiBaseHost.value
  return base || null
})

const normalizedLocalOriginHost = computed(() => normalizeHost(localOriginHost.value))

const timelineOriginFilter = ref<'all' | 'local' | 'remote'>('all')
const originFilterOptions = [
  { value: 'all', label: 'All events' },
  { value: 'local', label: 'Local' },
  { value: 'remote', label: 'Remote' },
]

const isRemoteEvent = (event: (typeof timelineEvents.value)[number]) => {
  const origin = normalizeHost(event?.event.origin_server ?? null)
  if (!origin) {
    return false
  }
  const localHost = normalizedLocalOriginHost.value
  if (localHost && origin === localHost) {
    return false
  }
  return true
}

const filteredTimelineEvents = computed(() => {
  if (timelineOriginFilter.value === 'all') {
    return timelineEvents.value
  }
  const wantRemote = timelineOriginFilter.value === 'remote'
  return timelineEvents.value.filter((event) => isRemoteEvent(event) === wantRemote)
})

const federationContext = computed(() => federationStore.contextForGuild(activeGuildId.value))
const federationRemoteServers = computed(() => federationContext.value?.remoteServers ?? [])
const federationTrustLevel = computed(() => federationContext.value?.trustLevel ?? 'trusted')
const hasRemoteServers = computed(() => federationRemoteServers.value.length > 0)
const trustAlertVariant = computed(() =>
  federationTrustLevel.value === 'trusted'
    ? 'info'
    : federationTrustLevel.value === 'limited'
      ? 'warning'
      : 'error',
)

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
const hasChannels = computed(() =>
  channelsForGuildRef.value ? channelsForGuildRef.value.length > 0 : false,
)
const channelListLoading = computed(() => channelStoreLoadingRef.value)

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

const handleTyping = ({ channelId, preview }: { channelId: string | null; preview: string }) => {
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
const { readiness: readinessRef } = storeToRefs(systemStore)

const sessionStore = useSessionStore()
const {
  profile: profileRef,
  profileLoading: profileLoadingRef,
  profileError: profileErrorRef,
  displayName: displayNameRef,
  profileAvatar: profileAvatarRef,
  identifier: identifierRef,
  deviceId: sessionDeviceIdRef,
  isAuthenticated: isAuthenticatedRef,
} = storeToRefs(sessionStore)

const {
  handshakeVectors: handshakeVectorsRef,
  handshakeLoading: handshakeLoadingRef,
  handshakeError: handshakeErrorRef,
} = storeToRefs(federationStore)

const {
  devices: devicesRef,
  loading: devicesLoadingRef,
  error: devicesErrorRef,
  hydrated: devicesHydratedRef,
} = storeToRefs(sessionDevicesStore)

const {
  keyPackages: keyPackagesRef,
  loading: keyPackagesLoadingRef,
  error: keyPackagesErrorRef,
  lastFetchedAt: keyPackagesFetchedAtRef,
} = storeToRefs(mlsStore)

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

watch(
  () => isAuthenticatedRef.value,
  (authenticated) => {
    if (!authenticated || devicesHydratedRef.value) {
      return
    }
    sessionDevicesStore.fetchDevices().catch((err) => {
      console.warn('Failed to load session devices', err)
    })
  },
  { immediate: true },
)

watch(
  () => isAuthenticatedRef.value,
  (authenticated) => {
    if (!featureFlags.mlsReadiness || !authenticated) {
      return
    }
    mlsStore.fetchKeyPackages().catch((err) => {
      console.warn('Failed to load MLS key packages', err)
    })
  },
  { immediate: true },
)

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

const sessionProfile = computed(() => profileRef.value)
const sessionProfileLoading = computed(() => profileLoadingRef.value)
const sessionProfileError = computed(() => profileErrorRef.value)
const sessionDisplayName = computed(() => displayNameRef.value || identifierRef.value || '—')
const sessionUsername = computed(() => sessionProfile.value?.username ?? identifierRef.value ?? '—')
const storageAudit = computed(() => sessionStore.storageAudit)
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
const platformRoles = computed(() => sessionProfile.value?.roles ?? [])
const sessionGuilds = computed(() => sessionProfile.value?.guilds ?? [])
const sessionChannels = computed(() => sessionProfile.value?.channels ?? [])
const sessionDevices = computed(() =>
  devicesRef.value.length ? devicesRef.value : (sessionProfile.value?.devices ?? []),
)
const devicesLoading = computed(() => devicesLoadingRef.value)
const devicesError = computed(() => devicesErrorRef.value)

const activeGuildRole = computed(() => resolveGuildRole(activeGuildId.value, sessionGuilds.value))
const activeChannelRole = computed(() =>
  resolveChannelRole(activeChannelId.value, sessionChannels.value),
)
const guildPermissions = computed(() =>
  deriveGuildPermissions(
    activeGuildRole.value,
    platformRoles.value || [],
    activeChannelRole.value?.role ?? null,
  ),
)
const canSendMessages = computed(() => guildPermissions.value.canSendMessages)
const canCreateChannels = computed(() => guildPermissions.value.canCreateChannels)
const canViewAdminPanel = computed(
  () => featureFlags.adminPanel && guildPermissions.value.canManageGuild,
)
const sendPermissionMessage = computed(() =>
  canSendMessages.value ? null : permissionGuidance('sendMessages', guildPermissions.value),
)
const composerDisabled = computed(
  () => !activeChannelId.value || channelListLoading.value || !canSendMessages.value,
)
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
  {
    label: 'Storage backend',
    value:
      storageAudit.value?.available === false && storageAudit.value?.reason
        ? `${storageAudit.value.type} · ${storageAudit.value.reason}`
        : (storageAudit.value?.type ?? 'unknown'),
  },
])

const sessionUserId = computed(() => {
  const userId = sessionProfile.value?.userId
  if (userId && userId.length) {
    return userId
  }
  const identifier = identifierRef.value
  return identifier && identifier.length ? identifier : null
})

const sessionDeviceId = computed(() => sessionDeviceIdRef.value || '')
const keyPackages = computed(() => keyPackagesRef.value ?? [])
const keyPackagesLoading = computed(() => keyPackagesLoadingRef.value)
const keyPackagesError = computed(() => keyPackagesErrorRef.value)
const keyPackagesLastFetchedLabel = computed(() =>
  keyPackagesFetchedAtRef.value ? formatDateTime(keyPackagesFetchedAtRef.value) : '—',
)
const localKeyIdentityCandidates = computed(() => {
  const candidates = new Set<string>()
  if (sessionDeviceId.value) {
    candidates.add(sessionDeviceId.value)
  }
  if (identifierRef.value) {
    candidates.add(identifierRef.value)
  }
  if (sessionProfile.value?.username) {
    candidates.add(sessionProfile.value.username)
  }
  return Array.from(candidates).filter(Boolean)
})
const hasLocalKeyPackage = computed(() => {
  if (!featureFlags.mlsReadiness) {
    return true
  }
  const candidates = localKeyIdentityCandidates.value
  if (!candidates.length) {
    return true
  }
  if (!keyPackages.value.length) {
    return false
  }
  return candidates.some((candidate) =>
    keyPackages.value.some((pkg) => pkg.identity === candidate),
  )
})
const missingKeyPackageIdentity = computed(() => {
  if (hasLocalKeyPackage.value) {
    return null
  }
  return localKeyIdentityCandidates.value[0] ?? null
})
const showMissingKeyPackageAlert = computed(
  () =>
    featureFlags.mlsReadiness &&
    !keyPackagesLoading.value &&
    !keyPackagesError.value &&
    Boolean(localKeyIdentityCandidates.value.length) &&
    !hasLocalKeyPackage.value,
)

const refreshProfile = async () => {
  try {
    await sessionStore.fetchProfile(true)
  } catch (err) {
    console.warn('Failed to refresh session profile', err)
  }
}

const fetchHandshakeVectors = async () => {
  try {
    await federationStore.fetchHandshakeVectors()
  } catch (err) {
    console.warn('Failed to fetch handshake vectors', err)
  }
}

const refreshKeyPackages = async () => {
  if (!featureFlags.mlsReadiness) {
    return
  }
  try {
    await mlsStore.fetchKeyPackages()
  } catch (err) {
    console.warn('Failed to refresh MLS key packages', err)
  }
}

const copyToClipboard = async (payload: string, fallbackMessage: string) => {
  try {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(payload)
      return
    }
  } catch (err) {
    console.warn(fallbackMessage, err)
  }
  const textarea = document.createElement('textarea')
  textarea.value = payload
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'absolute'
  textarea.style.left = '-9999px'
  document.body.appendChild(textarea)
  textarea.select()
  try {
    document.execCommand('copy')
  } catch (err) {
    console.warn(fallbackMessage, err)
  } finally {
    document.body.removeChild(textarea)
  }
}

const copyRemoteServer = async (server: string) => {
  await copyToClipboard(server, 'Failed to copy remote server host')
}

const handshakeLoading = computed(() => handshakeLoadingRef.value)
const handshakeError = computed(() => handshakeErrorRef.value)
const handshakePreview = computed(
  () => (handshakeVectorsRef.value ?? []).slice(0, 2),
)
const handshakeVerifiedAt = computed(() => federationStore.handshakeVerifiedAt)
const handshakeVerifiedLabel = computed(() => formatDateTime(handshakeVerifiedAt.value))
const handshakeNeedsReview = computed(() => {
  const iso = handshakeVerifiedAt.value
  if (!iso) {
    return true
  }
  const parsed = Date.parse(iso)
  if (Number.isNaN(parsed)) {
    return true
  }
  return Date.now() - parsed > HANDSHAKE_REVIEW_TTL_MS
})
const handshakeStatus = computed(() => ({
  color: handshakeNeedsReview.value ? 'warning' : 'success',
  label: handshakeNeedsReview.value ? 'Needs verification' : 'Verified',
}))

const copyKeyMaterial = async (value: string, meta: { identity: string; field: string }) => {
  if (!value) {
    return
  }
  await copyToClipboard(value, 'Failed to copy MLS key material')
  recordBreadcrumb({
    message: 'Copied MLS key material',
    category: 'mls.copy',
    level: 'info',
    data: meta,
  })
}

const deviceBootstrapOpen = ref(false)
const searchModalOpen = ref(false)
</script>

<template>
  <div class="space-y-10">
    <AppDeviceBootstrapModal
      v-model:open="deviceBootstrapOpen"
      :device-id="sessionDeviceId"
      :identifier="sessionUsername"
      :server-name="sessionServerName"
    />
    <AppGlobalSearchModal v-model:open="searchModalOpen" />

    <section
      class="relative overflow-hidden rounded-3xl border border-slate-800/50 bg-linear-to-br from-slate-900 via-slate-950 to-slate-950/60 px-8 py-10 shadow-xl shadow-slate-950/40"
    >
      <div class="relative z-10 max-w-3xl space-y-4">
        <UBadge variant="soft" color="info" label="Milestone F2 · Week 9" />
        <h1 class="text-3xl font-semibold text-white sm:text-4xl">
          MLS readiness and device bootstrap dashboard
        </h1>
        <p class="text-base text-slate-300 sm:text-lg">
          Review MLS key packages, remote trust indicators, and device bootstrap guidance without
          leaving the home dashboard. Refresh vectors after rotations and capture telemetry as Week
          9 lands.
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
          <UButton
            icon="i-heroicons-magnifying-glass"
            color="neutral"
            label="Search"
            variant="soft"
            @click="searchModalOpen = true"
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

        <UAlert
          v-else-if="hasRemoteServers"
          :color="trustAlertVariant"
          variant="soft"
          icon="i-heroicons-globe-alt"
          title="Remote federation active"
        >
          <template #description>
            <p class="text-xs text-slate-200">This guild pulls events from remote homeservers:</p>
            <ul class="mt-2 list-disc space-y-1 pl-5 text-xs text-slate-200">
              <li
                v-for="server in federationRemoteServers"
                :key="server"
                class="flex items-center gap-2"
              >
                <span>{{ server }}</span>
                <UButton
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  @click="copyRemoteServer(server)"
                >
                  Copy
                </UButton>
              </li>
            </ul>
          </template>
        </UAlert>

        <div
          class="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-white/5 bg-slate-900/40 px-3 py-2"
        >
          <p class="text-xs font-semibold uppercase tracking-wide text-slate-400">
            Event origin filter
          </p>
          <URadioGroup
            v-model="timelineOriginFilter"
            :items="originFilterOptions"
            size="sm"
            class="max-w-xs"
          />
        </div>

        <AppMessageTimeline
          :channel-id="activeChannelId"
          :channel-name="activeChannelName"
          :events="filteredTimelineEvents"
          :loading="timelineLoading"
          :error="timelineError"
          :local-origin-host="localOriginHost || ''"
          :remote-servers="federationRemoteServers"
          :current-user-id="sessionUserId"
          :current-user-role="guildPermissions.role"
          :current-user-permissions="guildPermissions"
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

        <UAlert
          v-if="sendPermissionMessage"
          color="neutral"
          variant="soft"
          icon="i-heroicons-lock-closed"
          title="Messaging restricted"
          :description="sendPermissionMessage"
        />

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
              <UButton
                icon="i-heroicons-clipboard-document-check"
                color="neutral"
                variant="ghost"
              />
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
      <GuildSurfaceCard>
        <template #header>
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-lg font-semibold text-white">Week 9 delivery log</h2>
              <p class="text-sm text-slate-400">
                Key MLS readiness updates: key packages, bootstrap tooling, and trust signals.
              </p>
            </div>
            <UButton
              icon="i-heroicons-arrow-path"
              color="neutral"
              variant="ghost"
              aria-label="Refresh feed"
            />
          </div>
        </template>
        <div class="space-y-8">
          <div v-for="item in timelineEntries" :key="item.id" class="relative pl-8">
            <span
              class="absolute left-0 top-1 h-2.5 w-2.5 rounded-full bg-sky-400 ring-4 ring-sky-500/20"
            />
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
      </GuildSurfaceCard>

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
            <div v-if="devicesLoading" class="space-y-2">
              <USkeleton class="h-3 w-32 rounded" />
              <USkeleton class="h-3 w-40 rounded" />
            </div>
            <UAlert
              v-else-if="devicesError"
              color="warning"
              variant="soft"
              icon="i-heroicons-exclamation-triangle"
              title="Unable to load devices"
              :description="devicesError"
            />
            <div v-else-if="!sessionDevices.length" class="text-xs text-slate-500">
              No device inventory yet. The UI falls back to mock data until `/sessions/devices`
              ships.
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

      <UCard v-if="canViewAdminPanel" class="border border-emerald-500/20 bg-emerald-500/5">
        <template #header>
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-lg font-semibold text-white">Admin controls</h2>
              <p class="text-sm text-emerald-200/80">
                Feature flag `VITE_FEATURE_ADMIN_PANEL` enabled · admin or maintainer role required.
              </p>
            </div>
            <UBadge color="success" variant="soft" label="Preview" />
          </div>
        </template>

        <div class="space-y-4 text-sm text-emerald-100/90">
          <p>
            Manage guild-level settings, promote members, and review audit trails. These controls
            surface automatically once the backend ships the admin APIs.
          </p>
          <ul class="list-disc space-y-2 pl-5">
            <li>Review pending role change requests</li>
            <li>Download compliance activity logs</li>
            <li>Toggle experimental messaging features per guild</li>
          </ul>
          <p class="text-xs text-emerald-200/70">
            Not seeing this panel? Double-check your guild role or reach out to a platform admin.
          </p>
        </div>
      </UCard>

      <UCard class="border border-sky-500/20 bg-sky-500/5">
        <template #header>
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 class="text-lg font-semibold text-white">Federation settings</h2>
              <p class="text-sm text-sky-200/80">Remote homeservers and handshake vectors</p>
            </div>
            <div class="flex items-center gap-2">
              <UBadge
                :color="handshakeStatus.color"
                variant="soft"
                :label="handshakeStatus.label"
              />
              <UButton
                icon="i-heroicons-arrow-path"
                color="neutral"
                variant="ghost"
                :loading="handshakeLoading"
                @click="fetchHandshakeVectors"
              >
                Refresh vectors
              </UButton>
            </div>
          </div>
        </template>

        <div class="space-y-4 text-sm text-slate-100">
          <UAlert
            v-if="handshakeNeedsReview"
            color="warning"
            variant="soft"
            icon="i-heroicons-shield-exclamation"
            title="Handshake verification required"
          >
            <template #description>
              Last verified {{ handshakeVerifiedLabel }}. Refresh the vectors above to store a new
              verification timestamp.
            </template>
          </UAlert>

          <div>
            <p class="text-xs uppercase tracking-wide text-slate-400">Remote servers</p>
            <div v-if="federationRemoteServers.length === 0" class="text-xs text-slate-500">
              No remote servers reported.
            </div>
            <ul v-else class="mt-2 space-y-2">
              <li
                v-for="server in federationRemoteServers"
                :key="server"
                class="flex items-center justify-between rounded bg-slate-900/70 px-3 py-2 text-xs"
              >
                <span>{{ server }}</span>
                <UButton
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  @click="copyRemoteServer(server)"
                >
                  Copy
                </UButton>
              </li>
            </ul>
          </div>

          <div>
            <p class="text-xs uppercase tracking-wide text-slate-400">Handshake vectors</p>
            <p class="text-[11px] text-slate-500">
              Last verified {{ handshakeVerifiedLabel }}
            </p>
            <div v-if="handshakeError" class="text-xs text-rose-200">{{ handshakeError }}</div>
            <div v-else-if="handshakeLoading" class="space-y-2">
              <USkeleton class="h-3 w-40 rounded" />
              <USkeleton class="h-3 w-32 rounded" />
            </div>
            <div v-else-if="handshakePreview.length === 0" class="text-xs text-slate-500">
              Fetch the handshake test vectors to validate MLS readiness.
            </div>
            <ul v-else class="mt-2 space-y-2">
              <li
                v-for="vector in handshakePreview"
                :key="vector.vector_id"
                class="rounded bg-slate-900/70 px-3 py-2 text-xs"
              >
                <div class="flex items-center justify-between text-slate-200">
                  <span>{{ vector.vector_id }}</span>
                  <UBadge color="info" variant="soft" :label="vector.origin" />
                </div>
                <pre
                  class="mt-2 overflow-x-auto rounded bg-slate-900/80 p-2 text-[10px] text-slate-300"
                  >{{ JSON.stringify(vector.payload, null, 2) }}
                </pre>
              </li>
            </ul>
          </div>
        </div>
      </UCard>

      <UCard
        v-if="featureFlags.mlsReadiness"
        class="border border-indigo-500/20 bg-indigo-500/5"
      >
        <template #header>
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 class="text-lg font-semibold text-white">MLS readiness</h2>
              <p class="text-sm text-indigo-100/80">
                Key packages, copy actions, and device bootstrap guidance
              </p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <UButton
                icon="i-heroicons-bolt"
                color="info"
                variant="ghost"
                @click="deviceBootstrapOpen = true"
              >
                Register device
              </UButton>
              <UButton
                icon="i-heroicons-arrow-path"
                color="neutral"
                variant="ghost"
                :loading="keyPackagesLoading"
                @click="refreshKeyPackages"
              >
                Refresh packages
              </UButton>
            </div>
          </div>
        </template>

        <div class="space-y-4 text-sm text-slate-100">
          <UAlert
            v-if="keyPackagesError"
            color="warning"
            variant="soft"
            icon="i-heroicons-exclamation-triangle"
            title="Unable to load key packages"
            :description="keyPackagesError"
          />
          <UAlert
            v-else-if="showMissingKeyPackageAlert"
            color="warning"
            variant="soft"
            icon="i-heroicons-key"
            title="Device missing MLS key package"
          >
            <template #description>
              {{ missingKeyPackageIdentity || 'Current device' }} has no MLS key package yet. Use
              the registration modal to mint one before attempting MLS enrolment.
            </template>
          </UAlert>
          <UAlert
            v-else-if="!keyPackagesLoading && keyPackages.length === 0"
            color="neutral"
            variant="soft"
            icon="i-heroicons-information-circle"
            title="No key packages reported"
            description="Once the backend exposes MLS identities they will appear here."
          />

          <div class="flex flex-wrap items-center justify-between text-[11px] text-slate-400">
            <span>Last fetched {{ keyPackagesLastFetchedLabel }}</span>
            <span v-if="sessionDeviceId">Local device ID {{ sessionDeviceId }}</span>
          </div>

          <div v-if="keyPackagesLoading" class="space-y-3">
            <USkeleton class="h-16 w-full rounded-xl" />
            <USkeleton class="h-16 w-full rounded-xl" />
          </div>

          <ul v-else class="space-y-3">
            <li
              v-for="pkg in keyPackages"
              :key="pkg.identity"
              class="rounded-2xl border border-white/10 bg-slate-950/40 p-4"
            >
              <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p class="text-sm font-semibold text-white">{{ pkg.identity }}</p>
                  <p class="text-xs text-slate-400">{{ pkg.ciphersuite }}</p>
                </div>
                <UBadge
                  color="info"
                  variant="soft"
                  :label="
                    pkg.rotated_at ? `Rotated ${formatDateTime(pkg.rotated_at)}` : 'Rotation pending'
                  "
                />
              </div>

              <div class="mt-4 grid gap-3 sm:grid-cols-2">
                <div class="rounded-xl border border-white/5 bg-slate-900/70 p-3">
                  <p class="text-[10px] uppercase tracking-wide text-slate-500">Signature key</p>
                  <p class="mt-1 break-all font-mono text-[11px] text-slate-100">
                    {{ pkg.signature_key }}
                  </p>
                  <UButton
                    size="xs"
                    variant="ghost"
                    color="neutral"
                    class="mt-2"
                    @click="copyKeyMaterial(pkg.signature_key, { identity: pkg.identity, field: 'signature_key' })"
                  >
                    Copy
                  </UButton>
                </div>

                <div class="rounded-xl border border-white/5 bg-slate-900/70 p-3">
                  <p class="text-[10px] uppercase tracking-wide text-slate-500">HPKE public key</p>
                  <p class="mt-1 break-all font-mono text-[11px] text-slate-100">
                    {{ pkg.hpke_public_key }}
                  </p>
                  <UButton
                    size="xs"
                    variant="ghost"
                    color="neutral"
                    class="mt-2"
                    @click="copyKeyMaterial(pkg.hpke_public_key, { identity: pkg.identity, field: 'hpke_public_key' })"
                  >
                    Copy
                  </UButton>
                </div>
              </div>
            </li>
          </ul>
        </div>
      </UCard>
    </section>
  </div>
</template>
