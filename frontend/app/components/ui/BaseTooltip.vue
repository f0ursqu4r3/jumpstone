<script setup lang="ts">
import { useAttrs } from 'vue';

const props = withDefaults(
  defineProps<{
    text?: string;
    color?: string;
    placement?:
      | 'top'
      | 'bottom'
      | 'left'
      | 'right'
      | 'top-start'
      | 'top-end'
      | 'bottom-start'
      | 'bottom-end'
      | 'left-start'
      | 'left-end'
      | 'right-start'
      | 'right-end';
    openDelay?: number;
    closeDelay?: number;
  }>(),
  {
    color: 'neutral',
    placement: 'top',
    text: '',
    openDelay: 0,
    closeDelay: 0,
  }
);

const attrs = useAttrs();
</script>

<template>
  <UTooltip v-bind="{ ...attrs, ...props }">
    <template #trigger>
      <slot />
    </template>
    <template v-if="$slots.content" #default>
      <slot name="content" />
    </template>
  </UTooltip>
</template>
