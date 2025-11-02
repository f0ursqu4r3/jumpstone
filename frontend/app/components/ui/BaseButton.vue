<script setup lang="ts">
import { useAttrs } from 'vue';

const props = withDefaults(
  defineProps<{
    color?: string;
    variant?: 'solid' | 'soft' | 'ghost' | 'outline' | 'link';
    size?: 'xs' | 'sm' | 'md' | 'lg';
    block?: boolean;
    loading?: boolean;
    icon?: string;
    to?: string | Record<string, unknown>;
    type?: 'button' | 'submit' | 'reset';
  }>(),
  {
    color: 'primary',
    variant: 'soft',
    size: 'md',
    type: 'button',
  }
);

const attrs = useAttrs();
</script>

<template>
  <UButton
    v-bind="{ ...attrs, ...props }"
    :class="['font-semibold tracking-tight rounded-lg', attrs.class]"
  >
    <slot />
  </UButton>
</template>
