<script setup lang="ts">
import { useAttrs } from 'vue'

const props = withDefaults(
  defineProps<{
    name?: string
    src?: string | null
    size?: 'xs' | 'sm' | 'md' | 'lg'
    alt?: string
  }>(),
  {
    size: 'sm',
    src: null,
    alt: '',
  },
)

const attrs = useAttrs()
</script>

<template>
  <UAvatar v-bind="{ ...attrs, ...props }" :class="['ring-2 ring-white/10', attrs.class]">
    <slot />
  </UAvatar>
</template>
