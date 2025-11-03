<!-- eslint-disable vue/multi-word-component-names -->
<script setup lang="ts">
import { useAttrs } from 'vue'

const props = withDefaults(
  defineProps<{
    name?: string
    src?: string | undefined
    size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
    alt?: string
  }>(),
  {
    size: 'sm',
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
