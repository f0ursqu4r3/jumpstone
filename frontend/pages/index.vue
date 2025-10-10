<template>
  <div class="min-h-screen bg-slate-950 text-slate-100">
    <main class="mx-auto flex max-w-3xl flex-col gap-6 px-6 py-16">
      <header class="space-y-2">
        <p class="text-sm uppercase tracking-[0.3em] text-indigo-400">
          OpenGuild
        </p>
        <h1 class="text-4xl font-semibold">
          Federated Discord-style communication.
        </h1>
        <p class="max-w-xl text-slate-300">
          Explore the upcoming homeserver and client stack that blends the
          familiarity of Discord with Matrix-inspired federation and modern
          end-to-end encryption.
        </p>
      </header>

      <section
        class="grid gap-4 rounded-xl border border-slate-800 bg-slate-900/60 p-6"
      >
        <h2 class="text-2xl font-medium">Milestones</h2>
        <ul class="space-y-2 text-sm text-slate-300">
          <li>
            <span class="font-semibold text-indigo-300">M0</span> —
            Single-server text MVP
          </li>
          <li>
            <span class="font-semibold text-indigo-300">M1</span> — E2EE for DMs
            and private channels
          </li>
          <li>
            <span class="font-semibold text-indigo-300">M2</span> — Text
            federation & state resolution
          </li>
        </ul>
      </section>

      <section
        class="grid gap-4 rounded-xl border border-slate-800 bg-slate-900/60 p-6"
      >
        <h2 class="text-2xl font-medium">Getting involved</h2>
        <p class="text-sm text-slate-300">
          The backend is a Rust workspace targeting Postgres, NATS, and
          S3-compatible storage. The frontend ships as a Nuxt 3 app with
          Tailwind design system.
        </p>
      </section>
    </main>
  </div>
</template>
