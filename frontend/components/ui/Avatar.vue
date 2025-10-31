<template>
  <div
    class="relative inline-flex items-center justify-center rounded-full border border-surface-muted bg-surface-subtle text-slate-200"
    :class="sizeClasses[size]"
  >
    <img
      v-if="src"
      :src="src"
      :alt="name"
      class="h-full w-full rounded-full object-cover"
    />
    <span
      v-else
      class="font-semibold uppercase"
    >
      {{ initials }}
    </span>
    <span
      v-if="status"
      class="absolute bottom-0 right-0 inline-flex rounded-full border-2 border-background"
      :class="statusClasses[status]"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, withDefaults } from 'vue';

type AvatarSize = 'sm' | 'md' | 'lg';
type PresenceStatus = 'online' | 'idle' | 'dnd' | 'offline';

const props = withDefaults(
  defineProps<{
    name: string;
    src?: string;
    size?: AvatarSize;
    status?: PresenceStatus;
  }>(),
  {
    src: '',
    size: 'md',
    status: undefined,
  },
);

const initials = computed(() => {
  if (!props.name) {
    return '?';
  }
  const words = props.name.trim().split(/\s+/);
  if (words.length === 1) {
    return words[0].charAt(0).toUpperCase();
  }
  return `${words[0].charAt(0)}${words[words.length - 1].charAt(0)}`.toUpperCase();
});

const sizeClasses: Record<AvatarSize, string> = {
  sm: 'h-8 w-8 text-xs',
  md: 'h-10 w-10 text-sm',
  lg: 'h-14 w-14 text-base',
};

const statusClasses: Record<PresenceStatus, string> = {
  online: 'h-2.5 w-2.5 bg-green-400',
  idle: 'h-2.5 w-2.5 bg-yellow-400',
  dnd: 'h-2.5 w-2.5 bg-red-500',
  offline: 'h-2.5 w-2.5 bg-slate-500',
};
</script>
