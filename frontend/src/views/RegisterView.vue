<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'

import AppOnboardingCarousel from '~/components/app/AppOnboardingCarousel.vue'
import { useSessionStore } from '~/stores/session'
import { createDefaultDeviceId } from '~/utils/device'

const sessionStore = useSessionStore()
const {
  loading: loadingRef,
  fieldErrors: fieldErrorsRef,
  error: errorRef,
  identifier: identifierRef,
  deviceId: deviceIdRef,
  deviceName: deviceNameRef,
} = storeToRefs(sessionStore)

const route = useRoute()
const router = useRouter()

onMounted(() => {
  document.title = 'Create account · OpenGuild'
})

const sanitizeRedirect = (value: unknown): string | null => {
  if (typeof value !== 'string') {
    return null
  }
  if (!value.startsWith('/')) {
    return null
  }
  if (value === '/register') {
    return '/'
  }
  return value
}

const redirectTarget = computed(
  () => sanitizeRedirect(route.query.redirect) ?? '/',
)

const onboardingSlides = [
  {
    id: 'naming',
    eyebrow: 'Week 3',
    title: 'Pick a durable username',
    description:
      'Usernames must be unique and at least three characters. Backend validation mirrors this form, so trim whitespace before submitting.',
    ctaLabel: 'Account setup notes',
    href: 'https://github.com/jumpstone/jumpstone/blob/main/docs/SETUP.md#seed-users',
    icon: 'i-heroicons-identification',
  },
  {
    id: 'device',
    eyebrow: 'Session hygiene',
    title: 'Bind refresh tokens to a device ID',
    description:
      'OpenGuild ties refresh tokens to the device identifier you provide. See BRAIN.txt for how the Axum session module enforces this pairing.',
    ctaLabel: 'Review session design',
    href: 'https://github.com/jumpstone/jumpstone/blob/main/BRAIN.txt',
    icon: 'i-heroicons-cpu-chip',
  },
  {
    id: 'qa',
    eyebrow: 'Quality gate',
    title: 'Run the auth smoke suite',
    description:
      'Vitest covers POST /users/register and /sessions/login so you can validate new flows locally before shipping.',
    ctaLabel: 'Testing checklist',
    href: 'https://github.com/jumpstone/jumpstone/blob/main/docs/TESTING.md',
    icon: 'i-heroicons-clipboard-document-check',
  },
] as const

type RegisterField = 'username' | 'password' | 'confirm' | 'deviceId' | 'deviceName'

const form = reactive<Record<RegisterField, string>>({
  username: identifierRef.value ?? '',
  password: '',
  confirm: '',
  deviceId: deviceIdRef.value ?? '',
  deviceName: deviceNameRef.value ?? '',
})

const errors = reactive<Record<RegisterField, string>>({
  username: '',
  password: '',
  confirm: '',
  deviceId: '',
  deviceName: '',
})

const generalError = ref('')
const submitting = computed(() => loadingRef.value)

const clearFieldErrors = () => {
  ;(Object.keys(errors) as RegisterField[]).forEach((key) => {
    errors[key] = ''
  })
}

const applyBackendErrors = () => {
  const backendErrors = fieldErrorsRef.value

  ;(Object.keys(errors) as RegisterField[]).forEach((key) => {
    if (backendErrors[key]) {
      errors[key] = backendErrors[key] ?? ''
    }
  })
}

const validate = () => {
  clearFieldErrors()
  let valid = true

  if (form.username.trim().length < 3) {
    errors.username = 'Username must be at least 3 characters.'
    valid = false
  }

  if (form.password.length < 8) {
    errors.password = 'Password must be at least 8 characters.'
    valid = false
  }

  if (form.confirm !== form.password) {
    errors.confirm = 'Passwords must match.'
    valid = false
  }

  if (!form.deviceId.trim()) {
    errors.deviceId = 'Device ID is required.'
    valid = false
  } else if (form.deviceId.trim().length < 3) {
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
    await sessionStore.register({
      username: form.username,
      password: form.password,
      deviceId: form.deviceId,
      deviceName: form.deviceName || undefined,
    })

    form.password = ''
    form.confirm = ''
    await router.replace(redirectTarget.value)
  } catch (error) {
    applyBackendErrors()
    generalError.value =
      errorRef.value ||
      (error instanceof Error
        ? error.message
        : 'Unable to complete registration right now.')
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
        <p class="text-[11px] font-semibold uppercase tracking-[0.4em] text-sky-400">
          OpenGuild
        </p>
        <h1 class="text-3xl font-semibold text-white sm:text-4xl">
          Create your operator account
        </h1>
        <p class="text-sm text-slate-300">
          Provision credentials that map to the backend session service, then jump straight into the
          control room.
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

        <UFormField label="Username" :error="errors.username" required>
          <UInput
            v-model="form.username"
            placeholder="guildmaster"
            autocomplete="username"
            class="w-full"
          />
        </UFormField>

        <UFormField
          label="Password"
          :error="errors.password"
          description="Minimum eight characters. Matches backend policy."
          required
        >
          <UInput
            v-model="form.password"
            type="password"
            autocomplete="new-password"
            placeholder="••••••••"
            class="w-full"
          />
        </UFormField>

        <UFormField label="Confirm password" :error="errors.confirm" required>
          <UInput
            v-model="form.confirm"
            type="password"
            autocomplete="new-password"
            placeholder="••••••••"
            class="w-full"
          />
        </UFormField>

        <UFormField
          label="Device identifier"
          :error="errors.deviceId"
          description="Refresh tokens are scoped to this identifier."
        >
          <UInput
            v-model="form.deviceId"
            autocomplete="off"
            placeholder="browser-onboarding"
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
            label="Create account"
            class="w-full justify-center"
            :loading="submitting"
          />
          <UButton
            to="/login"
            color="neutral"
            variant="ghost"
            label="Back to sign in"
            class="w-full justify-center"
          />
        </div>
      </form>

      <div class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
        <span>Already have credentials?</span>
        <RouterLink
          to="/login"
          class="font-semibold text-sky-300 transition hover:text-sky-200"
        >
          Sign in
        </RouterLink>
        <span class="hidden sm:inline">·</span>
        <RouterLink
          to="/roadmap"
          class="text-slate-400 transition hover:text-slate-200"
        >
          View the roadmap
        </RouterLink>
      </div>
    </section>

    <section class="lg:mt-2">
      <AppOnboardingCarousel :slides="onboardingSlides" class="h-full" />
    </section>
  </div>
</template>
