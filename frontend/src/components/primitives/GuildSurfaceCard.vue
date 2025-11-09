<script setup lang="ts">
import { computed } from 'vue'

type Tone = 'default' | 'info' | 'success' | 'warning'

const props = withDefaults(
  defineProps<{
    tone?: Tone
    padded?: boolean
  }>(),
  {
    tone: 'default',
    padded: true,
  },
)

const toneClasses: Record<Tone, string> = {
  default: 'border-white/5 bg-slate-950/60 shadow-slate-950/40',
  info: 'border-sky-500/20 bg-sky-500/5 shadow-sky-500/10',
  success: 'border-emerald-500/20 bg-emerald-500/5 shadow-emerald-500/10',
  warning: 'border-amber-500/20 bg-amber-500/5 shadow-amber-500/10',
}

const containerClasses = computed(() => [
  'rounded-3xl border transition-colors duration-200',
  'shadow-inner',
  toneClasses[props.tone] ?? toneClasses.default,
])

const bodyClasses = computed(() => (props.padded ? 'p-6' : 'p-0'))
</script>

<template>
  <section :class="containerClasses">
    <div v-if="$slots.header" class="border-b border-white/5 px-6 py-4">
      <slot name="header" />
    </div>
    <div :class="bodyClasses">
      <slot />
    </div>
    <div v-if="$slots.footer" class="border-t border-white/5 px-6 py-4">
      <slot name="footer" />
    </div>
  </section>
</template>
