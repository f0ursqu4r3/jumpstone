<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'

import { useSessionStore } from '@/stores/session'

import Button from '@/components/ui/Button.vue'
import Input from '@/components/ui/Input.vue'

const props = defineProps<{
  channelName: string
  topic?: string
}>()

const channelName = computed(() => props.channelName)
const sessionStore = useSessionStore()
const { isAuthenticated: isAuthenticatedRef } = storeToRefs(sessionStore)
const isAuthenticated = computed(() => isAuthenticatedRef.value)
const route = useRoute()
const router = useRouter()

const goToLogin = async () => {
  const redirect = route.path === '/login' ? null : route.fullPath
  await router.push(redirect ? { path: '/login', query: { redirect } } : { path: '/login' })
}

const handleAccountAction = async () => {
  if (isAuthenticated.value) {
    sessionStore.logout()
  }

  await goToLogin()
}
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
      <Input
        placeholder="Search"
        icon="i-heroicons-magnifying-glass-20-solid"
        color="neutral"
        variant="soft"
        class="hidden w-64 lg:block"
      />
      <Button
        icon="i-heroicons-bell-alert"
        color="neutral"
        variant="ghost"
        aria-label="Notifications"
      />
      <Button icon="i-heroicons-queue-list" color="neutral" variant="ghost" aria-label="Inbox" />
      <Button
        v-if="!isAuthenticated"
        color="info"
        variant="soft"
        icon="i-heroicons-arrow-right-end-on-rectangle"
        @click="handleAccountAction"
      >
        Sign in
      </Button>
      <Button
        v-else
        color="neutral"
        variant="ghost"
        icon="i-heroicons-arrow-left-on-rectangle"
        @click="handleAccountAction"
      >
        Sign out
      </Button>
    </div>
  </header>
</template>
