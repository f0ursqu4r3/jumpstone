<!-- eslint-disable vue/multi-word-component-names -->
<script setup lang="ts">
import { useAttrs } from 'vue'

const props = withDefaults(
  defineProps<{
    color?: 'primary' | 'secondary' | 'success' | 'info' | 'warning' | 'error' | 'neutral'
    variant?: 'solid' | 'soft' | 'ghost' | 'outline' | 'link' | 'subtle'
    size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
    block?: boolean
    loading?: boolean
    icon?: string
    to?: string | Record<string, unknown>
    type?: 'button' | 'submit' | 'reset'
    label?: string
  }>(),
  {
    color: 'primary',
    variant: 'soft',
    size: 'md',
    type: 'button',
  },
)

const attrs = useAttrs()
</script>

<template>
  <UButton
    v-bind="{ ...attrs, ...props }"
    :class="['cursor-pointer disabled:cursor-not-allowed disabled:opacity-50', attrs.class]"
  >
    <slot />
  </UButton>
</template>
