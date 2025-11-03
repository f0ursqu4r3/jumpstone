<script setup lang="ts">
import { computed } from 'vue'
import { storeToRefs } from 'pinia'

import { getRuntimeConfig } from '~/config/runtime'
import { useSessionStore } from '~/stores/session'
import { useSystemStore } from '~/stores/system'
import type { ComponentStatus } from '~/types/api'

const timelineEntries = [
  {
    id: 'kickoff',
    title: 'Frontend kickoff',
    author: 'Lia Chen',
    time: 'Today - 10:21 AM',
    summary:
      'Scaffolded Nuxt UI shell, navigation rails, and responsive layout baseline. Connected to roadmap tasks in docs/FRONTEND_TIMELINE.md.',
    tag: 'Milestone F0',
  },
  {
    id: 'design-sync',
    title: 'Design tokens imported',
    author: 'Ben Flores',
    time: 'Yesterday - 5:08 PM',
    summary:
      'Brand palette and typography landed in Tailwind config export. Dark mode defaults match app frame preview in Figma.',
    tag: 'Design',
  },
  {
    id: 'api-handshake',
    title: 'Session API handshake',
    author: 'Maya Singh',
    time: 'Yesterday - 11:32 AM',
    summary:
      'Login and refresh endpoints connected in sandbox. Captured QA steps in docs/TESTING.md for replay.',
    tag: 'Platform',
  },
] as const

const upcomingTasks = [
  {
    id: 'storybook',
    label: 'Wire Storybook with Nuxt UI tokens',
    owner: 'lia',
    status: 'In review',
  },
  {
    id: 'pinia-stores',
    label: 'Scaffold session and guild stores',
    owner: 'maya',
    status: 'Unstarted',
  },
  {
    id: 'api-client',
    label: 'HTTP client with retries and telemetry',
    owner: 'kai',
    status: 'Blocked',
  },
] as const

const quickMetrics = [
  { label: 'Open guilds', value: '6', trend: '+2 this week' },
  { label: 'Active channels', value: '28', trend: 'Guild sync focus' },
  { label: 'Pending invites', value: '14', trend: 'Awaiting approval' },
] as const

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
        <UBadge variant="soft" color="info" label="Milestone F0" />
        <h1 class="text-3xl font-semibold text-white sm:text-4xl">
          Welcome to the OpenGuild frontend workspace
        </h1>
        <p class="text-base text-slate-300 sm:text-lg">
          The navigation shell is ready. Next up: component stories, Pinia stores, and API wiring.
          Use this dashboard to track progress and jump into the developer docs.
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
      <UCard class="border border-white/5 bg-slate-950/60">
        <template #header>
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-lg font-semibold text-white">Release timeline</h2>
              <p class="text-sm text-slate-400">Snapshot of the workstreams landing this week.</p>
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
      </UCard>

      <div class="space-y-6">
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
                  class="rounded-md bg-slate-900/80 px-3 py-2 text-sm text-slate-200"
                >
                  <div class="flex items-center justify-between">
                    <span class="font-medium">
                      {{ device.deviceName || device.deviceId }}
                    </span>
                    <UBadge
                      v-if="device.userAgent"
                      color="neutral"
                      variant="soft"
                      :label="device.userAgent"
                    />
                  </div>
                  <p class="text-xs text-slate-500">
                    ID: {{ device.deviceId }} · Last seen:
                    {{ formatDateTime(device.lastSeenAt) }}
                  </p>
                </li>
              </ul>
            </div>
          </div>
        </UCard>

        <UCard class="border border-white/5 bg-slate-950/60">
          <template #header>
            <div class="flex items-center justify-between">
              <h2 class="text-lg font-semibold text-white">Backend status</h2>
              <UButton
                icon="i-heroicons-arrow-path"
                color="neutral"
                variant="ghost"
                :loading="backendPending"
                @click="refreshBackend()"
                aria-label="Refresh backend status"
              />
            </div>
          </template>

          <div v-if="backendPending" class="space-y-4">
            <div class="h-4 w-32 animate-pulse rounded bg-slate-800" />
            <div class="space-y-3 rounded-2xl bg-slate-900/60 p-4">
              <div class="h-4 w-full animate-pulse rounded bg-slate-800" />
              <div class="h-4 w-4/5 animate-pulse rounded bg-slate-800" />
              <div class="h-4 w-3/5 animate-pulse rounded bg-slate-800" />
            </div>
            <div class="h-4 w-40 animate-pulse rounded bg-slate-800" />
          </div>

          <div v-else-if="backendError" class="space-y-4">
            <UAlert
              color="warning"
              variant="soft"
              title="Unable to reach backend"
              :description="backendErrorMessage"
            />
            <p class="text-xs text-slate-500">
              Check that the Rust server is running locally ({{ apiBaseHost }}) or update
              <code class="text-slate-200">VITE_API_BASE_URL</code>
              in your environment.
            </p>
          </div>

          <div v-else class="space-y-4">
            <div class="flex items-center justify-between gap-4">
              <div>
                <p class="text-xs uppercase tracking-wide text-slate-400">Overall</p>
                <UBadge :label="readinessStatusLabel" :color="readinessBadgeColor" variant="soft" />
              </div>
              <div class="text-right">
                <p class="text-xs uppercase tracking-wide text-slate-400">Version</p>
                <p class="text-sm font-semibold text-white">
                  {{ backendVersion }}
                </p>
              </div>
            </div>

            <div class="rounded-2xl border border-white/5 bg-slate-900/60 p-4">
              <p class="text-xs uppercase tracking-wide text-slate-400">Components</p>
              <ul class="mt-3 space-y-3">
                <li
                  v-for="component in componentStatuses"
                  :key="component.name"
                  class="flex items-start justify-between gap-4"
                >
                  <div>
                    <p class="text-sm font-medium text-white">
                      {{ component.name }}
                    </p>
                    <p v-if="component.details" class="text-xs text-slate-500">
                      {{ component.details }}
                    </p>
                  </div>
                  <UBadge
                    :label="componentStatusLabel(component.status)"
                    :color="componentBadgeColor(component.status)"
                    variant="subtle"
                  />
                </li>
                <li v-if="!componentStatuses.length" class="text-xs text-slate-500">
                  No service components reported. Verify the backend readiness endpoint.
                </li>
              </ul>
            </div>

            <p class="text-xs text-slate-500">Uptime: {{ uptime }}</p>
          </div>
        </UCard>

        <UCard class="border border-white/5 bg-slate-950/60">
          <template #header>
            <h2 class="text-lg font-semibold text-white">Quick metrics</h2>
          </template>
          <dl class="space-y-4">
            <div v-for="metric in quickMetrics" :key="metric.label">
              <dt class="text-xs uppercase tracking-wide text-slate-400">
                {{ metric.label }}
              </dt>
              <dd class="mt-1 text-2xl font-semibold text-white">
                {{ metric.value }}
              </dd>
              <p class="text-xs text-slate-500">
                {{ metric.trend }}
              </p>
            </div>
          </dl>
        </UCard>

        <UCard class="border border-white/5 bg-slate-950/60">
          <template #header>
            <h2 class="text-lg font-semibold text-white">Upcoming tasks</h2>
          </template>
          <div class="space-y-4">
            <div
              v-for="task in upcomingTasks"
              :key="task.id"
              class="flex items-start justify-between gap-4"
            >
              <div>
                <p class="text-sm font-medium text-white">
                  {{ task.label }}
                </p>
                <p class="text-xs text-slate-500">Owner - {{ task.owner }}</p>
              </div>
              <UBadge color="info" variant="soft" :label="task.status" />
            </div>
          </div>
        </UCard>
      </div>
    </section>
  </div>
</template>
