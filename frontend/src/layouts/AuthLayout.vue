<template>
  <div
    class="flex min-h-screen flex-col items-center justify-center bg-slate-950 px-4 py-10"
  >
    <div
      class="relative w-full max-w-md rounded-3xl border border-slate-800/70 bg-slate-950/80 p-8 shadow-xl shadow-slate-950/50 backdrop-blur"
    >
      <div
        class="pointer-events-none absolute -top-24 right-12 h-40 w-40 rounded-full bg-sky-500/10 blur-3xl"
      />
      <slot />
    </div>
  </div>
</template>
