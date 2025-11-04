<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'

import { useSessionStore } from '@/stores/session'
import { createDefaultDeviceId } from '@/utils/device'
import type { OnboardingSlide } from '@/types/ui'

import AppOnboardingCarousel from '@/components/app/AppOnboardingCarousel.vue'

const sessionStore = useSessionStore()
const {
  isAuthenticated: isAuthenticatedRef,
  identifier: identifierRef,
  deviceId: deviceIdRef,
  deviceName: deviceNameRef,
  loading: loadingRef,
  fieldErrors: fieldErrorsRef,
  error: errorRef,
} = storeToRefs(sessionStore)
const route = useRoute()
const router = useRouter()

onMounted(() => {
  document.title = 'Sign in · OpenGuild'
})

const sanitizeRedirect = (value: unknown): string | null => {
  if (typeof value !== 'string') {
    return null
  }
  if (!value.startsWith('/')) {
    return null
  }
  if (value === '/login') {
    return '/'
  }
  return value
}

const redirectTarget = computed(() => sanitizeRedirect(route.query.redirect) ?? '/')

if (typeof window !== 'undefined' && isAuthenticatedRef.value) {
  router.replace(redirectTarget.value)
}

type LoginFormField = 'identifier' | 'secret' | 'deviceId' | 'deviceName'

const onboardingSlides: OnboardingSlide[] = [
  {
    id: 'backend-setup',
    eyebrow: 'Week 1',
    title: 'Point the client at your OpenGuild backend',
    description:
      'Update VITE_API_BASE_URL to the Axum server from docs/SETUP.md. BRAIN.txt captures the config precedence if you need overrides.',
    ctaLabel: 'Open setup guide',
    href: 'https://github.com/jumpstone/jumpstone/blob/main/docs/SETUP.md',
    icon: 'i-heroicons-wrench-screwdriver',
  },
  {
    id: 'timeline-sync',
    eyebrow: 'Week 2',
    title: 'Track Vue delivery alongside the backend',
    description:
      'Review docs/FRONTEND_TIMELINE.md before each push to keep milestones aligned with the Axum services.',
    ctaLabel: 'View frontend timeline',
    to: '/roadmap',
    icon: 'i-heroicons-map',
  },
  {
    id: 'qa-smoke',
    eyebrow: 'Week 3',
    title: 'Run the authentication smoke tests',
    description:
      'Execute the new Vitest suite after touching auth flows to cover POST /sessions/login and /users/register.',
    ctaLabel: 'Testing checklist',
    href: 'https://github.com/jumpstone/jumpstone/blob/main/docs/TESTING.md',
    icon: 'i-heroicons-check-badge',
  },
] as const

const form = reactive<Record<LoginFormField, string>>({
  identifier: identifierRef.value ?? '',
  secret: '',
  deviceId: deviceIdRef.value ?? '',
  deviceName: deviceNameRef.value ?? '',
})

const errors = reactive<Record<LoginFormField, string>>({
  identifier: '',
  secret: '',
  deviceId: '',
  deviceName: '',
})

const generalError = ref('')
const submitting = computed(() => loadingRef.value)

const clearFieldErrors = () => {
  ;(Object.keys(errors) as LoginFormField[]).forEach((key) => {
    errors[key] = ''
  })
}

const applyBackendErrors = () => {
  const backendErrors = fieldErrorsRef.value

  ;(Object.keys(errors) as LoginFormField[]).forEach((key) => {
    if (backendErrors[key]) {
      errors[key] = backendErrors[key] ?? ''
    }
  })
}

const validate = () => {
  clearFieldErrors()
  let valid = true

  if (!form.identifier.trim()) {
    errors.identifier = 'Identifier is required.'
    valid = false
  }

  if (form.secret.length < 8) {
    errors.secret = 'Secret must be at least 8 characters.'
    valid = false
  }

  if (!form.deviceId.trim()) {
    errors.deviceId = 'Device ID is required.'
    valid = false
  }

  if (form.deviceId && form.deviceId.trim().length < 3) {
    errors.deviceId = 'Device ID must be at least 3 characters.'
    valid = false
  }

  if (form.deviceName && form.deviceName.trim().length < 3) {
    errors.deviceName = 'Device name must be at least 3 characters.'
    valid = false
  }

  return valid
}

const handleSubmit = async () => {
  generalError.value = ''

  if (!validate()) {
    return
  }

  try {
    await sessionStore.login({
      identifier: form.identifier,
      secret: form.secret,
      deviceId: form.deviceId,
      deviceName: form.deviceName || undefined,
    })

    form.secret = ''
    await router.replace(redirectTarget.value)
  } catch (error) {
    applyBackendErrors()

    generalError.value =
      errorRef.value ||
      (error instanceof Error ? error.message : 'Unable to complete sign in right now.')
  }
}

onMounted(() => {
  if (!form.deviceId) {
    form.deviceId = createDefaultDeviceId()
  }
})
</script>

<template>
  <div class="grid gap-10 lg:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)]">
    <section class="space-y-8">
      <div class="space-y-3">
        <p class="text-[11px] font-semibold uppercase tracking-[0.4em] text-sky-400">OpenGuild</p>
        <h1 class="text-3xl font-semibold text-white sm:text-4xl">Sign in to continue</h1>
        <p class="text-sm text-slate-300">
          Use your homeserver credentials to unlock the guild control room.
        </p>
      </div>

      <form
        class="space-y-5 rounded-2xl border border-white/5 bg-slate-950/60 p-6 shadow-inner shadow-slate-950/40 backdrop-blur"
        @submit.prevent="handleSubmit"
      >
        <UAlert
          v-if="generalError"
          color="error"
          variant="soft"
          icon="i-heroicons-shield-exclamation"
          :description="generalError"
        />

        <UFormField label="Identifier" :error="errors.identifier" required>
          <UInput
            v-model="form.identifier"
            placeholder="you@example.org"
            autocomplete="username"
            class="w-full"
          />
        </UFormField>

        <UFormField
          label="Secret"
          :error="errors.secret"
          description="Minimum eight characters. Matches backend password policy."
          required
        >
          <UInput
            v-model="form.secret"
            type="password"
            autocomplete="current-password"
            placeholder="••••••••"
            class="w-full"
          />
        </UFormField>

        <UFormField
          label="Device identifier"
          :error="errors.deviceId"
          description="Used to bind your refresh token to this browser."
        >
          <UInput
            v-model="form.deviceId"
            autocomplete="off"
            placeholder="browser-dev"
            class="w-full"
          />
        </UFormField>

        <UFormField
          label="Device name"
          :error="errors.deviceName"
          description="Optional label shown in the device management list."
          hint="Optional"
        >
          <UInput
            v-model="form.deviceName"
            autocomplete="off"
            placeholder="MacBook Pro"
            class="w-full"
          />
        </UFormField>

        <div class="space-y-3 pt-2">
          <UButton
            type="submit"
            color="info"
            label="Sign in"
            class="w-full justify-center"
            :loading="submitting"
          />
          <UButton
            to="/styleguide"
            color="neutral"
            variant="ghost"
            label="Preview the styleguide"
            class="w-full justify-center"
          />
        </div>
      </form>

      <div class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
        <span>Need an account?</span>
        <RouterLink to="/register" class="font-semibold text-sky-300 transition hover:text-sky-200">
          Create one
        </RouterLink>
        <span class="hidden sm:inline">·</span>
        <RouterLink to="/roadmap" class="text-slate-400 transition hover:text-slate-200">
          View the roadmap
        </RouterLink>
      </div>
    </section>

    <section class="lg:mt-2">
      <AppOnboardingCarousel :slides="onboardingSlides" class="h-full" />
    </section>
  </div>
</template>
