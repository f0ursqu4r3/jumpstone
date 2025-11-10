<script setup lang="ts">
withDefaults(
  defineProps<{
    message?: string
  }>(),
  {
    message: 'Preparing your workspace…',
  },
)
</script>

<template>
  <div
    class="fixed inset-0 z-1000 flex flex-col items-center justify-center bg-slate-950 text-white"
  >
    <div class="mb-6 flex flex-col items-center gap-3 text-center">
      <p class="text-lg font-semibold tracking-wide text-white">OpenGuild</p>
      <p class="text-sm text-slate-400">{{ message }}</p>
    </div>
    <div class="flex items-center gap-3 text-slate-300">
      <span class="relative flex h-3 w-3">
        <span
          class="absolute inline-flex h-full w-full animate-ping rounded-full bg-sky-400 opacity-75"
        />
        <span class="relative inline-flex h-3 w-3 rounded-full bg-sky-500" />
      </span>
      <span>Loading…</span>
    </div>
  </div>
</template>
