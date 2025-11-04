<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import type { RadioGroupItem } from '@nuxt/ui'

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

const title: string = 'Create a new channel'
const description: string = `Text channels work best for discussions, while voice channels are perfect for huddles and stand-ups.`

const form = reactive({
  name: '',
  kind: 'text' as 'text' | 'voice',
})

const items: RadioGroupItem[] = [
  { value: 'text', label: 'Text channel', description: 'Messages, threads, and updates' },
  { value: 'voice', label: 'Voice channel', description: 'Stand-ups and huddles' },
] as const

const touched = ref(false)

watch(
  () => open.value,
  (isOpen) => {
    if (isOpen) {
      return
    }
    form.name = ''
    form.kind = 'text'
    touched.value = false
    emit('reset-error')
  },
)

const nameError = computed(() => {
  if (!touched.value) {
    return ''
  }
  const trimmed = form.name.trim()
  if (!trimmed) {
    return 'Channel name is required.'
  }
  if (trimmed.length < 2) {
    return 'Channel name must be at least 2 characters.'
  }
  if (trimmed.length > 64) {
    return 'Channel name cannot exceed 64 characters.'
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
  <UModal v-model:open="open" size="md" :title="title" :description="description">
    <template #content>
      <div class="space-y-6 p-6">
        <div>
          <h2 class="text-xl font-semibold text-white">{{ title }}</h2>
          <p class="text-sm text-slate-400">{{ description }}</p>
        </div>

        <form class="space-y-4" @submit.prevent="handleSubmit">
          <UFormField label="Channel name" :error="nameError || undefined" required>
            <UInput
              v-model="form.name"
              placeholder="frontend-team"
              class="w-full"
              icon="i-heroicons-hashtag-20-solid"
              :autofocus="true"
              :disabled="loading"
              @blur="touched = true"
            />
          </UFormField>

          <UFormField label="Channel type">
            <URadioGroup v-model="form.kind" :items="items" :disabled="loading" />
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
            <UButton color="info" type="submit" :loading="loading"> Create channel </UButton>
          </div>
        </form>
      </div>
    </template>
  </UModal>
</template>
