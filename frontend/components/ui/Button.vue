<template>
  <button
    :type="type"
    :disabled="disabled"
    :class="[
      'inline-flex items-center justify-center gap-2 rounded-md border font-medium transition focus:outline-none focus:ring-0 focus-visible:shadow-focus disabled:opacity-60 disabled:cursor-not-allowed',
      block ? 'w-full' : 'w-auto',
      sizeClasses[size],
      variantClasses[variant],
    ]"
  >
    <span v-if="$slots.icon" class="flex items-center">
      <slot name="icon" />
    </span>
    <span :class="[$slots.icon ? 'whitespace-nowrap' : '']">
      <slot />
    </span>
  </button>
</template>

<script setup lang="ts">
import { withDefaults } from 'vue';

type ButtonVariant = 'primary' | 'secondary' | 'ghost';
type ButtonSize = 'xs' | 'sm' | 'md' | 'lg';

withDefaults(
  defineProps<{
    variant?: ButtonVariant;
    size?: ButtonSize;
    disabled?: boolean;
    block?: boolean;
    type?: 'button' | 'submit' | 'reset';
  }>(),
  {
    variant: 'primary',
    size: 'md',
    disabled: false,
    block: false,
    type: 'button',
  },
);

const variantClasses: Record<ButtonVariant, string> = {
  primary:
    'bg-brand-primary border-brand-primary text-white hover:bg-brand-primary/90',
  secondary:
    'bg-surface-subtle border-surface-muted text-slate-100 hover:bg-surface-muted/70',
  ghost:
    'bg-transparent border-transparent text-slate-300 hover:bg-surface-muted/40',
};

const sizeClasses: Record<ButtonSize, string> = {
  xs: 'h-8 px-3 text-xs',
  sm: 'h-9 px-4 text-sm',
  md: 'h-10 px-5 text-sm',
  lg: 'h-11 px-6 text-base',
};
</script>
