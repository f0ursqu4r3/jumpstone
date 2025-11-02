<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { useSessionStore } from '~/stores/session';

definePageMeta({
  layout: 'auth',
});

const sessionStore = useSessionStore();

useHead({
  title: 'Sign in · OpenGuild',
});

if (import.meta.client && sessionStore.isAuthenticated) {
  await navigateTo('/');
}

type LoginFormField = 'identifier' | 'secret' | 'deviceId' | 'deviceName';

const createDefaultDeviceId = () => {
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.randomUUID === 'function'
  ) {
    return `browser-${crypto.randomUUID().slice(0, 8)}`;
  }

  return `browser-${Math.random().toString(36).slice(2, 8)}`;
};

const form = reactive<Record<LoginFormField, string>>({
  identifier: sessionStore.identifier ?? '',
  secret: '',
  deviceId: sessionStore.deviceId ?? '',
  deviceName: sessionStore.deviceName ?? '',
});

const errors = reactive<Record<LoginFormField, string>>({
  identifier: '',
  secret: '',
  deviceId: '',
  deviceName: '',
});

const generalError = ref('');
const submitting = computed(() => sessionStore.loading);

const clearFieldErrors = () => {
  (Object.keys(errors) as LoginFormField[]).forEach((key) => {
    errors[key] = '';
  });
};

const applyBackendErrors = () => {
  const backendErrors = sessionStore.fieldErrors;

  (Object.keys(errors) as LoginFormField[]).forEach((key) => {
    if (backendErrors[key]) {
      errors[key] = backendErrors[key] ?? '';
    }
  });
};

const validate = () => {
  clearFieldErrors();
  let valid = true;

  if (!form.identifier.trim()) {
    errors.identifier = 'Identifier is required.';
    valid = false;
  }

  if (form.secret.length < 8) {
    errors.secret = 'Secret must be at least 8 characters.';
    valid = false;
  }

  if (!form.deviceId.trim()) {
    errors.deviceId = 'Device ID is required.';
    valid = false;
  }

  if (form.deviceId && form.deviceId.trim().length < 3) {
    errors.deviceId = 'Device ID must be at least 3 characters.';
    valid = false;
  }

  if (form.deviceName && form.deviceName.trim().length < 3) {
    errors.deviceName = 'Device name must be at least 3 characters.';
    valid = false;
  }

  return valid;
};

const handleSubmit = async () => {
  generalError.value = '';

  if (!validate()) {
    return;
  }

  try {
    await sessionStore.login({
      identifier: form.identifier,
      secret: form.secret,
      deviceId: form.deviceId,
      deviceName: form.deviceName || undefined,
    });

    form.secret = '';
    await navigateTo('/');
  } catch (error) {
    applyBackendErrors();

    generalError.value =
      sessionStore.error ||
      (error instanceof Error
        ? error.message
        : 'Unable to complete sign in right now.');
  }
};

onMounted(() => {
  if (!form.deviceId) {
    form.deviceId = createDefaultDeviceId();
  }
});
</script>

<template>
  <div class="space-y-8">
    <div class="space-y-2 text-center">
      <p class="text-sm font-semibold uppercase tracking-[0.4em] text-sky-400">
        OpenGuild
      </p>
      <h1 class="text-2xl font-semibold text-white sm:text-3xl">
        Sign in to continue
      </h1>
      <p class="text-sm text-slate-400">
        Use your homeserver credentials to unlock the guild control room.
      </p>
    </div>

    <form class="space-y-5" @submit.prevent="handleSubmit">
      <UAlert
        v-if="generalError"
        color="error"
        variant="soft"
        icon="i-heroicons-shield-exclamation"
        :description="generalError"
      />

      <UFormGroup label="Identifier" :error="errors.identifier">
        <UInput
          v-model="form.identifier"
          placeholder="you@example.org"
          autocomplete="username"
        />
      </UFormGroup>

      <UFormGroup label="Secret" :error="errors.secret">
        <UInput
          v-model="form.secret"
          type="password"
          autocomplete="current-password"
          placeholder="••••••••"
        />
        <template #description>
          <p class="text-xs text-slate-500">
            Minimum eight characters. Matches backend password policy.
          </p>
        </template>
      </UFormGroup>

      <UFormGroup label="Device identifier" :error="errors.deviceId">
        <UInput
          v-model="form.deviceId"
          autocomplete="off"
          placeholder="browser-dev"
        />
        <template #description>
          <p class="text-xs text-slate-500">
            Used to bind your refresh token to this browser.
          </p>
        </template>
      </UFormGroup>

      <UFormGroup label="Device name" :error="errors.deviceName">
        <UInput
          v-model="form.deviceName"
          autocomplete="off"
          placeholder="MacBook Pro"
        />
        <template #description>
          <p class="text-xs text-slate-500">
            Optional label shown in the device management list.
          </p>
        </template>
      </UFormGroup>

      <div class="space-y-3 pt-2">
        <UButton
          type="submit"
          color="info"
          label="Sign in"
          class="w-full justify-center"
          :loading="submitting"
        />
        <UButton
          to="/"
          color="neutral"
          variant="ghost"
          label="Back to overview"
          class="w-full justify-center"
        />
      </div>
    </form>

    <div class="text-center text-xs text-slate-500">
      Need an account?
      <NuxtLink
        to="https://github.com/openguild"
        target="_blank"
        class="text-sky-400 transition hover:text-sky-300"
      >
        Contact the ops team
      </NuxtLink>
    </div>
  </div>
</template>
