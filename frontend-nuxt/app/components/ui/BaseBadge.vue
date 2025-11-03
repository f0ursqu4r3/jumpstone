<script setup lang="ts">
import { useAttrs } from 'vue';

const props = withDefaults(
  defineProps<{
    label?: string;
    color?: string;
    variant?: 'solid' | 'soft' | 'outline';
    size?: 'xs' | 'sm' | 'md';
  }>(),
  {
    color: 'primary',
    variant: 'soft',
    size: 'sm',
  }
);

const attrs = useAttrs();
</script>

<template>
  <UBadge
    v-bind="{ ...attrs, ...props }"
    :class="['font-medium tracking-tight', attrs.class]"
  >
    <slot>{{ props.label }}</slot>
  </UBadge>
</template>
