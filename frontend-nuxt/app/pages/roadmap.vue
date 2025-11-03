<template>
  <div class="space-y-6">
    <UCard class="border border-white/5 bg-slate-950/70">
      <template #header>
        <h1 class="text-2xl font-semibold text-white">
          Roadmap hub
        </h1>
      </template>
      <p class="text-sm leading-relaxed text-slate-300">
        Detailed milestone planning lives in the Markdown docs shipped with the repository. Start with
        <code class="rounded bg-slate-900 px-2 py-1 text-xs text-slate-200">docs/FRONTEND_TIMELINE.md</code>
        and keep it in sync with weekly sync notes.
      </p>
      <UAlert
        class="mt-6"
        color="sky"
        variant="soft"
        title="Coming soon"
        description="This view will evolve into a structured roadmap with filters and doc embeds."
      />
      <UButton to="/" label="Back to overview" color="gray" variant="ghost" class="mt-6 w-fit" />
    </UCard>
  </div>
</template>
