<script setup lang="ts">
import { computed, useAttrs } from 'vue';

const props = withDefaults(
  defineProps<{
    label?: string;
    hint?: string;
    error?: string | null;
    size?: 'sm' | 'md' | 'lg';
    color?: string;
    variant?: 'soft' | 'outline' | 'ghost';
  }>(),
  {
    size: 'md',
    color: 'neutral',
    variant: 'soft',
    error: null,
  }
);

const attrs = useAttrs();

const inputProps = computed(() => ({
  ...attrs,
  size: props.size,
  color: props.color,
  variant: props.variant,
}));
</script>

<template>
  <div class="space-y-1">
    <label
      v-if="props.label"
      class="text-xs font-semibold uppercase tracking-wide text-slate-400"
    >
      {{ props.label }}
    </label>
    <UInput
      v-bind="inputProps"
      :class="[
        'rounded-lg border-white/10 bg-surface-900/60 text-sm text-white shadow-none focus:ring-0 focus:outline-none focus-visible:shadow-focus',
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
