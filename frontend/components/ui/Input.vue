<template>
  <component
    :is="label ? 'label' : 'div'"
    class="flex w-full flex-col gap-1 text-sm text-slate-200"
  >
    <span v-if="label" class="font-medium text-slate-300">
      {{ label }}
    </span>
    <div class="relative">
      <input
        :id="id"
        :type="type"
        :value="modelValue"
        :placeholder="placeholder"
        :disabled="disabled"
        :class="[
          'w-full rounded-md border bg-background-elevated/80 px-3 py-2 text-slate-100 shadow-sm transition focus:border-brand-primary focus:outline-none focus-visible:shadow-focus disabled:cursor-not-allowed disabled:opacity-50',
          hasError
            ? 'border-intent-danger focus:border-intent-danger'
            : 'border-surface-muted',
        ]"
        @input="onInput"
      />
      <div
        v-if="$slots.suffix"
        class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-slate-500"
      >
        <slot name="suffix" />
      </div>
    </div>
    <p
      v-if="message"
      :class="[
        'text-xs',
        hasError ? 'text-intent-danger' : 'text-slate-500',
      ]"
    >
      {{ message }}
    </p>
  </component>
</template>

<script setup lang="ts">
import { computed, withDefaults } from 'vue';

const props = withDefaults(
  defineProps<{
    modelValue?: string;
    label?: string;
    placeholder?: string;
    type?: string;
    hint?: string;
    error?: string;
    disabled?: boolean;
    id?: string;
  }>(),
  {
    modelValue: '',
    placeholder: '',
    type: 'text',
    hint: '',
    error: '',
    disabled: false,
    id: undefined,
  },
);

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void;
}>();

const hasError = computed(() => Boolean(props.error));
const message = computed(() => props.error || props.hint);

const onInput = (event: Event) => {
  const target = event.target as HTMLInputElement;
  emit('update:modelValue', target.value);
};
</script>
