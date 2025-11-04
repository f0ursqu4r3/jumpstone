<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'

import Avatar from '@/components/ui/Avatar.vue'
import Badge from '@/components/ui/Badge.vue'
import Button from '@/components/ui/Button.vue'
import { useSessionStore } from '~/stores/session'

interface ChannelEntry {
  id: string
  label: string
  kind: 'text' | 'voice'
  icon?: string
  to?: string
  unread?: number
  description?: string
}

const props = defineProps<{
  guildName: string
  channels: ChannelEntry[]
}>()

const guildName = computed(() => props.guildName)
const channels = computed(() => props.channels)
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
    label: string
    icon: string
    to: string
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
      label: `${channel.kind === 'text' ? '#' : ''}${channel.label}`,
      icon,
      to: channel.to ?? '#',
      badge: channel.unread
        ? {
            label: channel.unread > 9 ? '9+' : channel.unread.toString(),
            color: 'info',
          }
        : undefined,
      description: channel.description,
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
        />
      </div>

      <div class="px-2">
        <UTooltip text="Create channel" placement="right" :content="{ side: 'right' }">
          <Button
            color="info"
            variant="soft"
            class="mt-2 w-full justify-center"
            icon="i-heroicons-plus-circle"
          >
            New channel
          </Button>
        </UTooltip>
      </div>

      <USeparator class="mt-4 opacity-50" />

      <div class="flex-1 space-y-4 overflow-y-auto p-2">
        <section v-for="group in groupedChannels" :key="group.label" class="space-y-3">
          <p class="text-xs font-semibold uppercase tracking-wide text-slate-500">
            {{ group.label }}
          </p>
          <ul>
            <li v-for="channel in group.children" :key="channel.label">
              <RouterLink
                :to="channel.to"
                class="flex items-center justify-between rounded-md px-3 py-2 text-sm font-medium text-slate-300 transition hover:bg-slate-800 hover:text-white"
              >
                <div class="flex items-center gap-2">
                  <UIcon :name="channel.icon" class="h-5 w-5 shrink-0" />
                  <span>{{ channel.label }}</span>
                </div>
                <div v-if="channel.badge">
                  <Badge :color="channel.badge.color" size="sm">
                    {{ channel.badge.label }}
                  </Badge>
                </div>
              </RouterLink>
            </li>
          </ul>
        </section>
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
