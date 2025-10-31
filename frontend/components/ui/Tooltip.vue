<template>
  <div
    class="group relative inline-flex"
    :aria-describedby="isOpen ? tooltipId : undefined"
    @mouseenter="show"
    @mouseleave="hide"
    @focusin="show"
    @focusout="hide"
    @keydown.esc="hide"
  >
    <slot />
    <transition
      enter-active-class="transition duration-100 ease-out"
      enter-from-class="opacity-0 translate-y-1"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition duration-75 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 translate-y-1"
    >
      <div
        v-if="isOpen"
        :id="tooltipId"
        role="tooltip"
        class="pointer-events-none absolute left-1/2 top-full z-30 mt-2 w-max -translate-x-1/2 rounded-md border border-surface-muted bg-background-elevated/95 px-2.5 py-1.5 text-xs text-slate-100 shadow-lg"
      >
        <slot name="content">
          {{ text }}
        </slot>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, withDefaults } from 'vue';

const props = withDefaults(
  defineProps<{
    text?: string;
    delay?: number;
  }>(),
  {
    text: '',
    delay: 100,
  },
);

const isOpen = ref(false);
const timeoutHandle = ref<number | null>(null);

const tooltipId = `tooltip-${Math.random().toString(36).slice(2, 9)}`;

const clearTimer = () => {
  if (timeoutHandle.value !== null) {
    window.clearTimeout(timeoutHandle.value);
    timeoutHandle.value = null;
  }
};

const show = () => {
  clearTimer();
  timeoutHandle.value = window.setTimeout(() => {
    isOpen.value = true;
  }, props.delay);
};

const hide = () => {
  clearTimer();
  isOpen.value = false;
};

onBeforeUnmount(() => {
  clearTimer();
});
</script>
