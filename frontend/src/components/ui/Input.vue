<!-- eslint-disable vue/multi-word-component-names -->
<script setup lang="ts">
import { computed, useAttrs } from 'vue'

const props = withDefaults(
  defineProps<{
    label?: string
    hint?: string
    error?: string | null
    size?: 'sm' | 'md' | 'lg'
    color?: 'neutral' | 'info'
    variant?: 'soft' | 'outline' | 'ghost'
    icon?: string
    modelValue?: string | number
  }>(),
  {
    size: 'md',
    color: 'neutral',
    variant: 'soft',
    error: null,
    label: '',
    hint: '',
    icon: undefined,
  },
)

const attrs = useAttrs()

const inputProps = computed(() => ({
  ...attrs,
  size: props.size,
  color: props.color,
  variant: props.variant,
  icon: props.icon,
}))
</script>

<template>
  <div class="space-y-1">
    <label v-if="props.label" class="text-xs font-semibold uppercase tracking-wide text-slate-400">
      {{ props.label }}
    </label>
    <UInput
      v-bind="inputProps"
      :model-value="props.modelValue"
      :class="[
        'rounded-lg bg-surface-900/60 text-sm text-white focus-visible:ring-2 focus-visible:ring-sky-500/40',
        attrs.class,
      ]"
    />
    <p v-if="props.hint && !props.error" class="text-xs text-slate-500">
      {{ props.hint }}
    </p>
    <p v-if="props.error" class="text-xs text-rose-400">
      {{ props.error }}
    </p>
  </div>
</template>
