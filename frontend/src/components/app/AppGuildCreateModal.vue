<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'

withDefaults(
  defineProps<{
    loading?: boolean
    error?: string | null
  }>(),
  {
    loading: false,
    error: null,
  },
)

const emit = defineEmits<{
  (event: 'submit', name: string): void
  (event: 'reset-error'): void
}>()

const open = defineModel<boolean>('open', { default: false })

const form = reactive({
  name: '',
})

const touched = ref(false)

watch(
  () => open.value,
  (isOpen) => {
    if (isOpen) {
      return
    }
    form.name = ''
    touched.value = false
    emit('reset-error')
  },
)

const nameError = computed(() => {
  if (!touched.value) {
    return ''
  }
  if (!form.name.trim()) {
    return 'Guild name is required.'
  }
  if (form.name.trim().length < 3) {
    return 'Guild name must be at least 3 characters.'
  }
  if (form.name.trim().length > 64) {
    return 'Guild name cannot exceed 64 characters.'
  }
  return ''
})

const close = () => {
  open.value = false
}

const handleSubmit = () => {
  touched.value = true
  if (nameError.value) {
    return
  }
  emit('submit', form.name.trim())
}
</script>

<template>
  <UModal
    v-model:open="open"
    size="md"
    title="Create a new guild"
    description="Guild names should be unique and between 3 and 64 characters. You can configure roles
            and invites after creation."
  >
    <template #content>
      <div class="space-y-6 p-6">
        <div>
          <h2 class="text-xl font-semibold text-white">Create a new guild</h2>
          <p class="text-sm text-slate-400">
            Guild names should be unique and between 3 and 64 characters. You can configure roles
            and invites after creation.
          </p>
        </div>
        <form class="space-y-4" @submit.prevent="handleSubmit">
          <UFormField label="Guild name" :error="nameError || undefined" required>
            <UInput
              v-model="form.name"
              placeholder="Frontend Core"
              class="w-full"
              autofocus
              :disabled="loading"
              @blur="touched = true"
            />
          </UFormField>

          <UAlert
            v-if="error"
            color="warning"
            variant="soft"
            icon="i-heroicons-exclamation-triangle"
            :description="error"
          />

          <div class="flex justify-end gap-2">
            <UButton
              color="neutral"
              variant="ghost"
              type="button"
              :disabled="loading"
              @click="close"
            >
              Cancel
            </UButton>
            <UButton color="info" type="submit" :loading="loading"> Create guild </UButton>
          </div>
        </form>
      </div>
    </template>
  </UModal>
</template>
