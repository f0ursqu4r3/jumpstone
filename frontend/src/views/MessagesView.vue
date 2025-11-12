<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'

import AppMessageComposer from '@/components/app/AppMessageComposer.vue'
import AppMessageTimeline from '@/components/timeline/AppMessageTimeline.vue'
import { getRuntimeConfig } from '@/config/runtime'
import { useChannelStore } from '@/stores/channels'
import { useConnectivityStore } from '@/stores/connectivity'
import { useFederationStore } from '@/stores/federation'
import { useGuildStore } from '@/stores/guilds'
import { useMessageComposerStore } from '@/stores/messages'
import { useNotificationStore } from '@/stores/notifications'
import { useRealtimeStore } from '@/stores/realtime'
import { useSessionStore } from '@/stores/session'
import { useTimelineStore } from '@/stores/timeline'
import {
  deriveGuildPermissions,
  permissionGuidance,
  resolveChannelRole,
  resolveGuildRole,
} from '@/utils/permissions'

const runtimeConfig = getRuntimeConfig()

const channelStore = useChannelStore()
const guildStore = useGuildStore()
const timelineStore = useTimelineStore()
const realtimeStore = useRealtimeStore()
const messageComposerStore = useMessageComposerStore()
const notificationStore = useNotificationStore()
const sessionStore = useSessionStore()
const federationStore = useFederationStore()
const connectivityStore = useConnectivityStore()

const realtimeStatus = realtimeStore.status
const realtimeAttemptingReconnect = realtimeStore.attemptingReconnect

const {
  activeChannelId: activeChannelIdRef,
  activeChannel: activeChannelRef,
  loading: channelLoadingRef,
} = storeToRefs(channelStore)
const { activeGuild: activeGuildRef } = storeToRefs(guildStore)
const {
  eventsByChannel: eventsByChannelRef,
  loadingByChannel: loadingByChannelRef,
  errorByChannel: errorByChannelRef,
} = storeToRefs(timelineStore)
const {
  profile: profileRef,
  profileLoading: profileLoadingRef,
  identifier: identifierRef,
  isAuthenticated: isAuthenticatedRef,
} = storeToRefs(sessionStore)
const { degradedMessage: degradedMessageRef, online: onlineRef } = storeToRefs(connectivityStore)

const activeGuildId = computed(() => activeGuildRef.value?.id ?? null)
const activeChannelId = computed(() => activeChannelIdRef.value ?? null)
const activeChannelName = computed(() => activeChannelRef.value?.label ?? '')

const typingPreview = ref<string | null>(null)
let typingPreviewTimer: ReturnType<typeof setTimeout> | null = null
const loadedChannels = new Set<string>()

const markActiveChannelRead = () => {
  const channelId = activeChannelIdRef.value
  if (!channelId) {
    return
  }
  const sequence = timelineStore.getCommittedSequence(channelId)
  if (sequence !== null && typeof sequence !== 'undefined') {
    channelStore.markChannelReadRemote(channelId, sequence).catch((err) => {
      console.warn('Failed to persist read state', err)
    })
  }
}

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
  () => [activeChannelIdRef.value, timelineEvents.value.length],
  () => {
    markActiveChannelRead()
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

watch(
  () => isAuthenticatedRef.value,
  (authenticated) => {
    if (!authenticated || profileRef.value || profileLoadingRef.value) {
      return
    }
    sessionStore.fetchProfile().catch((err) => {
      console.warn('Failed to hydrate session profile', err)
    })
  },
  { immediate: true },
)

watch(
  () => isAuthenticatedRef.value,
  (authenticated) => {
    if (authenticated) {
      notificationStore.connect()
    } else {
      notificationStore.disconnect()
    }
  },
  { immediate: true },
)

watch(
  () => isAuthenticatedRef.value,
  (authenticated) => {
    if (!authenticated) {
      return
    }
    channelStore.syncUnreadState().catch((err) => {
      console.warn('Failed to sync unread counts', err)
    })
  },
  { immediate: true },
)

watch(
  () => onlineRef.value,
  (online) => {
    if (!isAuthenticatedRef.value) {
      return
    }
    if (!online) {
      notificationStore.pause()
    } else {
      notificationStore.connect()
    }
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

const refreshTimeline = async () => {
  const channelId = activeChannelIdRef.value
  if (!channelId) {
    return
  }

  try {
    await timelineStore.loadChannel(channelId, { refresh: true, force: true })
    loadedChannels.add(channelId)
  } catch (err) {
    console.warn('Failed to refresh channel timeline', err)
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

const sessionProfile = computed(() => profileRef.value)

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

const localOriginHost = computed(() => {
  if (sessionServerName.value && sessionServerName.value !== 'Local server') {
    return sessionServerName.value
  }
  const base = apiBaseHost.value
  return base || null
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

const normalizedLocalOriginHost = computed(() => normalizeHost(localOriginHost.value))

const timelineOriginFilter = ref<'all' | 'local' | 'remote'>('all')
const originFilterOptions = [
  { value: 'all', label: 'All events' },
  { value: 'local', label: 'Local' },
  { value: 'remote', label: 'Remote' },
]

const setTimelineOriginFilter = (value: 'all' | 'local' | 'remote') => {
  timelineOriginFilter.value = value
}

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

const degradedMessage = computed(() => degradedMessageRef.value)

const federationContext = computed(() => federationStore.contextForGuild(activeGuildId.value))
const federationRemoteServers = computed(() => federationContext.value?.remoteServers ?? [])
const hasRemoteServers = computed(() => federationRemoteServers.value.length > 0)

const copyToClipboard = async (payload: string) => {
  try {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(payload)
      return
    }
  } catch (err) {
    console.warn('Clipboard API failed', err)
  }

  if (typeof document === 'undefined') {
    return
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
    console.warn('Fallback copy failed', err)
  } finally {
    document.body.removeChild(textarea)
  }
}

const copyRemoteServer = async (server: string) => {
  await copyToClipboard(server)
}

const sessionUserId = computed(() => {
  const userId = sessionProfile.value?.userId
  if (userId && userId.length) {
    return userId
  }
  const identifier = identifierRef.value
  return identifier && identifier.length ? identifier : null
})

const platformRoles = computed(() => sessionProfile.value?.roles ?? [])
const sessionGuilds = computed(() => sessionProfile.value?.guilds ?? [])
const sessionChannels = computed(() => sessionProfile.value?.channels ?? [])

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

const sendPermissionMessage = computed(() =>
  guildPermissions.value.canSendMessages
    ? null
    : permissionGuidance('sendMessages', guildPermissions.value),
)

const composerDisabled = computed(
  () =>
    !activeChannelId.value || channelLoadingRef.value || !guildPermissions.value.canSendMessages,
)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-4">
    <Teleport defer to="#messages-topbar-channel">
      <div v-if="activeChannelId" class="flex items-center justify-center gap-2 h-full">
        <UPopover>
          <UButton
            variant="link"
            color="neutral"
            icon="i-heroicons-signal-16-solid"
            size="xs"
            class="cursor-pointer"
            :label="`${timelineOriginFilter === 'all' ? 'All events' : timelineOriginFilter === 'local' ? 'Local' : 'Remote'}`"
            :disabled="!activeChannelId"
          />
          <template #content="{ close }">
            <div class="w-48 space-y-1 p-2 text-sm text-slate-200">
              <button
                v-for="option in originFilterOptions"
                :key="option.value"
                type="button"
                class="flex w-full items-center justify-between rounded px-2 py-1 text-left hover:bg-white/5"
                @click="
                  () => {
                    setTimelineOriginFilter(option.value as 'all' | 'local' | 'remote')
                    close()
                  }
                "
              >
                <span>{{ option.label }}</span>
                <UIcon
                  v-if="timelineOriginFilter === option.value"
                  name="i-heroicons-check"
                  class="h-4 w-4 text-sky-300"
                />
              </button>
            </div>
          </template>
        </UPopover>
        <!-- <UTooltip text="Refresh timeline">
          <UButton
        icon="i-heroicons-arrow-path"
        variant="ghost"
        color="neutral"
        :loading="timelineLoading"
        :disabled="!activeChannelId"
        @click="refreshTimeline"
        aria-label="Refresh messages"
          />
        </UTooltip> -->
      </div>
    </Teleport>

    <UAlert
      v-if="degradedMessage"
      color="warning"
      variant="soft"
      icon="i-heroicons-exclamation-triangle"
      title="Connectivity notice"
      :description="degradedMessage"
    />

    <UAlert
      v-if="hasRemoteServers"
      color="info"
      variant="soft"
      icon="i-heroicons-globe-alt"
      title="Remote federation active"
    >
      <template #description>
        <p class="text-xs text-slate-200">This channel includes events from remote homeservers:</p>
        <ul class="mt-2 list-disc space-y-1 pl-5 text-xs text-slate-200">
          <li
            v-for="server in federationRemoteServers"
            :key="server"
            class="flex items-center gap-2"
          >
            <span>{{ server }}</span>
            <UButton size="xs" variant="ghost" color="neutral" @click="copyRemoteServer(server)">
              Copy
            </UButton>
          </li>
        </ul>
      </template>
    </UAlert>

    <div class="flex-1 min-h-0 overflow-hidden">
      <AppMessageTimeline
        class="h-full"
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
    </div>

    <div
      v-if="typingPreview"
      class="flex flex-wrap items-center gap-2 rounded-lg border border-sky-500/10 bg-sky-500/5 p-2 text-xs text-slate-300"
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
</template>
