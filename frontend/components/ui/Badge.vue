<template>
  <span
    :class="[
      'inline-flex items-center gap-1 rounded-full border text-xs font-medium uppercase tracking-wider',
      sizeClasses[size],
      variantClasses[variant],
    ]"
  >
    <slot />
  </span>
</template>

<script setup lang="ts">
import { withDefaults } from 'vue';

type BadgeVariant = 'solid' | 'outline' | 'ghost';
type BadgeSize = 'sm' | 'md';

withDefaults(
  defineProps<{
    variant?: BadgeVariant;
    size?: BadgeSize;
  }>(),
  {
    variant: 'solid',
    size: 'md',
  },
);

const variantClasses: Record<BadgeVariant, string> = {
  solid:
    'border-brand-primary bg-brand-primary/20 text-brand-accent px-2 text-[0.6rem]',
  outline:
    'border-brand-primary/60 text-brand-accent px-2 text-[0.6rem] bg-transparent',
  ghost: 'border-transparent text-slate-400 px-2 bg-surface-subtle/40',
};

const sizeClasses: Record<BadgeSize, string> = {
  sm: 'px-2 py-0.5 text-[0.6rem]',
  md: 'px-3 py-1 text-[0.65rem]',
};
</script>
