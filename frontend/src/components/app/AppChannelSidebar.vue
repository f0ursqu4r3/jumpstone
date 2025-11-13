<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'

import { useSessionStore } from '@/stores/session'
import type { ChannelEntry } from '@/types/ui'

import Avatar from '@/components/ui/Avatar.vue'
import Badge from '@/components/ui/Badge.vue'
import Button from '@/components/ui/Button.vue'

const props = withDefaults(
  defineProps<{
    guildName: string
    channels: ChannelEntry[]
    loading?: boolean
    canCreateChannel?: boolean
    createChannelDisabledReason?: string
  }>(),
  {
    loading: false,
    canCreateChannel: true,
    createChannelDisabledReason: '',
  },
)

const emit = defineEmits<{
  (event: 'select-channel', channelId: string): void
  (event: 'create-channel'): void
  (event: 'open-guild-settings'): void
}>()

const guildName = computed(() => props.guildName)
const channels = computed(() => props.channels)
const loading = computed(() => props.loading)
const createChannelTooltip = computed(() => {
  if (props.canCreateChannel) {
    return 'Create channel'
  }
  return props.createChannelDisabledReason || 'Insufficient permissions to create channels.'
})
const createChannelDisabledMessage = computed(() => {
  if (props.canCreateChannel) {
    return ''
  }
  return props.createChannelDisabledReason || ''
})
const sessionStore = useSessionStore()
const {
  isAuthenticated: isAuthenticatedRef,
  profile: profileRef,
  identifier: identifierRef,
} = storeToRefs(sessionStore)

const isAuthenticated = computed(() => isAuthenticatedRef.value)
const profile = computed(() => profileRef.value)
const accountLabel = computed(() => {
  if (profile.value?.displayName) {
    return profile.value.displayName
  }
  if (profile.value?.username) {
    return profile.value.username
  }
  return identifierRef.value || 'Signed out user'
})
const accountStatus = computed(() => {
  if (!isAuthenticated.value) {
    return 'Logged out'
  }

  if (profile.value?.serverName) {
    return `Online · ${profile.value.serverName}`
  }

  return 'Online'
})
const accountStatusClass = computed(() =>
  isAuthenticated.value ? 'text-emerald-400' : 'text-slate-500',
)
const avatarUrl = computed(() => {
  if (profile.value?.avatarUrl) {
    return profile.value.avatarUrl
  }
  const seed =
    profile.value?.displayName || profile.value?.username || identifierRef.value || 'OpenGuild'
  return `https://api.dicebear.com/7.x/initials/svg?seed=${encodeURIComponent(seed)}`
})

const route = useRoute()
const router = useRouter()

const goToLogin = async () => {
  const redirect = route.path === '/login' ? null : route.fullPath
  await router.push(redirect ? { path: '/login', query: { redirect } } : { path: '/login' })
}

const handleSignOut = async () => {
  if (!isAuthenticated.value) {
    await goToLogin()
    return
  }

  sessionStore.logout()
  await goToLogin()
}

const groupedChannels = computed(() => {
  type BucketChild = {
    id: string
    label: string
    icon: string
    badge?: {
      label: string
      color:
        | 'primary'
        | 'secondary'
        | 'info'
        | 'success'
        | 'warning'
        | 'error'
        | 'neutral'
        | undefined
    }
    description?: string
    active: boolean
  }

  const buckets: Record<'text' | 'voice', { label: string; children: BucketChild[] }> = {
    text: { label: 'Text Channels', children: [] },
    voice: { label: 'Voice Channels', children: [] },
  }

  channels.value.forEach((channel) => {
    const icon =
      channel.icon ??
      (channel.kind === 'voice' ? 'i-heroicons-speaker-wave' : 'i-heroicons-hashtag')

    buckets[channel.kind].children.push({
      id: channel.id,
      label: `${channel.kind === 'text' ? '#' : ''}${channel.label}`,
      icon,
      badge: channel.unread
        ? {
            label: channel.unread > 9 ? '9+' : channel.unread.toString(),
            color: 'info',
          }
        : undefined,
      description: channel.description,
      active: Boolean(channel.active),
    })
  })

  return Object.values(buckets).filter((bucket) => bucket.children.length > 0)
})
</script>

<template>
  <aside class="flex h-full w-72 flex-col justify-between border-r border-white/5">
    <div class="flex min-h-0 flex-1 flex-col">
      <div class="flex items-start justify-between gap-2 p-2">
        <div>
          <p class="font-semibold text-slate-50">
            {{ guildName }}
          </p>
          <p class="text-xs text-slate-500">Internal build</p>
        </div>
        <Button
          icon="i-heroicons-cog-6-tooth"
          color="neutral"
          variant="ghost"
          aria-label="Guild settings"
          @click="emit('open-guild-settings')"
        />
      </div>

      <div class="px-2">
        <UTooltip :text="createChannelTooltip" placement="right" :content="{ side: 'right' }">
          <Button
            color="info"
            variant="soft"
            class="mt-2 w-full justify-center"
            icon="i-heroicons-plus-circle"
            :loading="loading"
            :disabled="loading || !props.canCreateChannel"
            @click="emit('create-channel')"
          >
            New channel
          </Button>
        </UTooltip>
        <p v-if="createChannelDisabledMessage" class="mt-2 text-xs text-amber-200/70">
          {{ createChannelDisabledMessage }}
        </p>
      </div>

      <USeparator class="mt-4 opacity-50" />

      <div class="flex-1 space-y-4 overflow-y-auto p-2">
        <div v-if="loading" class="space-y-3">
          <div
            v-for="index in 6"
            :key="`loading-${index}`"
            class="flex items-center gap-3 rounded-lg px-2 py-2"
          >
            <USkeleton class="h-5 w-5 rounded" />
            <div class="flex-1 space-y-2">
              <USkeleton class="h-3 w-24 rounded" />
              <USkeleton class="h-3 w-32 rounded" />
            </div>
          </div>
        </div>
        <section v-else v-for="group in groupedChannels" :key="group.label" class="space-y-3">
          <p class="text-xs font-semibold uppercase tracking-wide text-slate-500">
            {{ group.label }}
          </p>
          <ul class="flex flex-col gap-1">
            <li v-for="channel in group.children" :key="channel.id">
              <button
                type="button"
                class="flex w-full items-center justify-between rounded-md px-3 py-1 text-sm font-medium transition focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 cursor-pointer"
                :class="[
                  channel.active
                    ? 'bg-slate-800 text-white shadow-inner shadow-sky-500/10'
                    : 'text-slate-500 hover:bg-slate-800 hover:text-white',
                ]"
                @click="emit('select-channel', channel.id)"
              >
                <div class="flex items-center gap-2 text-left">
                  <UIcon :name="channel.icon" class="size-4 shrink-0" />
                  <div class="flex flex-col text-left">
                    <span>{{ channel.label.replace(/^#/g, '') }}</span>
                  </div>
                </div>
                <div v-if="channel.badge">
                  <Badge :color="channel.badge.color" size="sm">
                    {{ channel.badge.label }}
                  </Badge>
                </div>
              </button>
            </li>
          </ul>
        </section>
        <div
          v-if="!loading && groupedChannels.length === 0"
          class="rounded-xl border border-dashed border-white/10 bg-slate-950/30 px-4 py-6 text-center text-sm text-slate-400"
        >
          <UIcon name="i-heroicons-sparkles" class="mb-3 inline-flex h-6 w-6 text-slate-500" />
          <p class="font-semibold text-white">No channels yet</p>
          <p class="mt-1 text-xs text-slate-500">
            Create a channel to start conversations. You can add text or voice rooms as needed.
          </p>
          <Button
            class="mt-3"
            color="info"
            variant="ghost"
            icon="i-heroicons-plus-circle"
            @click="emit('create-channel')"
          >
            Create channel
          </Button>
        </div>
      </div>
    </div>

    <div>
      <USeparator class="opacity-50" />
      <div class="flex items-center gap-3 p-2">
        <Avatar :name="accountLabel" size="sm" :src="avatarUrl" />
        <div class="flex-1 text-sm">
          <p class="font-semibold text-white">
            {{ accountLabel }}
          </p>
          <p v-if="profile?.email" class="text-xs text-slate-500">
            {{ profile.email }}
          </p>
          <p class="text-xs" :class="accountStatusClass">
            {{ accountStatus }}
          </p>
        </div>
        <Button
          icon="i-heroicons-arrow-left-on-rectangle"
          variant="ghost"
          color="neutral"
          :aria-label="isAuthenticated ? 'Sign out' : 'Sign in'"
          @click="handleSignOut"
        />
      </div>
    </div>
  </aside>
</template>
