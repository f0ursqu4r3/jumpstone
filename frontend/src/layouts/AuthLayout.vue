<template>
  <div class="relative flex min-h-screen items-center justify-center bg-slate-950 px-4 py-12">
    <div
      class="relative w-full max-w-5xl overflow-hidden rounded-3xl border border-white/5 bg-slate-950/75 p-8 shadow-[0_45px_80px_-30px_rgba(8,47,73,0.65)] backdrop-blur-xl sm:p-10 lg:p-12"
    >
      <div
        class="pointer-events-none absolute -top-32 right-10 h-56 w-56 rounded-full bg-sky-500/15 blur-3xl"
      />
      <div
        class="pointer-events-none absolute -bottom-24 left-6 h-44 w-44 rounded-full bg-indigo-500/10 blur-3xl"
      />
      <slot />
    </div>
  </div>
</template>
