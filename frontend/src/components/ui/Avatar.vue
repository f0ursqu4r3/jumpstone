<!-- eslint-disable vue/multi-word-component-names -->
<script setup lang="ts">
import { useAttrs, computed } from 'vue'

const props = withDefaults(
  defineProps<{
    name?: string
    src?: string | undefined
    size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
  }>(),
  {
    size: 'sm',
  },
)

const alt = computed(() => props.name)

const attrs = useAttrs()
</script>

<template>
  <UAvatar
    v-bind="{ ...attrs, ...{ ...props, alt } }"
    :class="['ring-2 ring-white/10', attrs.class]"
  >
    <slot />
  </UAvatar>
</template>
