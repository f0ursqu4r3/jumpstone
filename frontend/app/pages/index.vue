<script setup lang="ts">
const timelineEntries = [
  {
    id: 'kickoff',
    title: 'Frontend kickoff',
    author: 'Lia Chen',
    time: 'Today - 10:21 AM',
    summary:
      'Scaffolded Nuxt UI shell, navigation rails, and responsive layout baseline. Connected to roadmap tasks in docs/FRONTEND_TIMELINE.md.',
    tag: 'Milestone F0',
  },
  {
    id: 'design-sync',
    title: 'Design tokens imported',
    author: 'Ben Flores',
    time: 'Yesterday - 5:08 PM',
    summary:
      'Brand palette and typography landed in Tailwind config export. Dark mode defaults match app frame preview in Figma.',
    tag: 'Design',
  },
  {
    id: 'api-handshake',
    title: 'Session API handshake',
    author: 'Maya Singh',
    time: 'Yesterday - 11:32 AM',
    summary:
      'Login and refresh endpoints connected in sandbox. Captured QA steps in docs/TESTING.md for replay.',
    tag: 'Platform',
  },
] as const;

const upcomingTasks = [
  {
    id: 'storybook',
    label: 'Wire Storybook with Nuxt UI tokens',
    owner: 'lia',
    status: 'In review',
  },
  {
    id: 'pinia-stores',
    label: 'Scaffold session and guild stores',
    owner: 'maya',
    status: 'Unstarted',
  },
  {
    id: 'api-client',
    label: 'HTTP client with retries and telemetry',
    owner: 'kai',
    status: 'Blocked',
  },
] as const;

const quickMetrics = [
  { label: 'Open guilds', value: '6', trend: '+2 this week' },
  { label: 'Active channels', value: '28', trend: 'Guild sync focus' },
  { label: 'Pending invites', value: '14', trend: 'Awaiting approval' },
] as const;
</script>

<template>
  <div class="space-y-10">
    <section
      class="relative overflow-hidden rounded-3xl border border-slate-800/50 bg-linear-to-br from-slate-900 via-slate-950 to-slate-950/60 px-8 py-10 shadow-xl shadow-slate-950/40"
    >
      <div class="relative z-10 max-w-3xl space-y-4">
        <UBadge variant="soft" color="info" label="Milestone F0" />
        <h1 class="text-3xl font-semibold text-white sm:text-4xl">
          Welcome to the OpenGuild frontend workspace
        </h1>
        <p class="text-base text-slate-300 sm:text-lg">
          The navigation shell is ready. Next up: component stories, Pinia
          stores, and API wiring. Use this dashboard to track progress and jump
          into the developer docs.
        </p>
        <div class="flex flex-wrap gap-3 pt-2">
          <UButton
            icon="i-heroicons-rocket-launch"
            color="info"
            label="Open roadmap"
            to="/roadmap"
            variant="solid"
          />
          <UButton
            icon="i-heroicons-academic-cap"
            color="neutral"
            label="Developer setup"
            to="https://github.com/openguild"
            target="_blank"
            variant="ghost"
          />
          <UButton
            icon="i-heroicons-swatch"
            color="neutral"
            label="View styleguide"
            to="/styleguide"
            variant="ghost"
          />
        </div>
      </div>
      <div
        class="pointer-events-none absolute -right-20 -top-20 h-96 w-96 rounded-full bg-sky-500/10 blur-3xl"
      />
    </section>

    <section class="grid gap-6 lg:grid-cols-[2fr_1fr]">
      <UCard class="border border-white/5 bg-slate-950/60">
        <template #header>
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-lg font-semibold text-white">Release timeline</h2>
              <p class="text-sm text-slate-400">
                Snapshot of the workstreams landing this week.
              </p>
            </div>
            <UButton
              icon="i-heroicons-arrow-path"
              color="neutral"
              variant="ghost"
              aria-label="Refresh feed"
            />
          </div>
        </template>

        <div class="space-y-8">
          <div
            v-for="item in timelineEntries"
            :key="item.id"
            class="relative pl-8"
          >
            <span
              class="absolute left-0 top-1 h-2.5 w-2.5 rounded-full bg-sky-400 ring-4 ring-sky-500/20"
            />
            <div class="flex flex-wrap items-center gap-3">
              <p class="text-sm font-semibold text-white">
                {{ item.title }}
              </p>
              <UBadge variant="soft" color="neutral" :label="item.tag" />
              <span class="text-xs text-slate-500">
                {{ item.time }}
              </span>
            </div>
            <p class="mt-3 text-sm leading-relaxed text-slate-300">
              {{ item.summary }}
            </p>
            <p class="mt-2 text-xs font-medium text-slate-500">
              Posted by {{ item.author }}
            </p>
          </div>
        </div>
      </UCard>

      <div class="space-y-6">
        <UCard class="border border-white/5 bg-slate-950/60">
          <template #header>
            <h2 class="text-lg font-semibold text-white">Quick metrics</h2>
          </template>
          <dl class="space-y-4">
            <div v-for="metric in quickMetrics" :key="metric.label">
              <dt class="text-xs uppercase tracking-wide text-slate-400">
                {{ metric.label }}
              </dt>
              <dd class="mt-1 text-2xl font-semibold text-white">
                {{ metric.value }}
              </dd>
              <p class="text-xs text-slate-500">
                {{ metric.trend }}
              </p>
            </div>
          </dl>
        </UCard>

        <UCard class="border border-white/5 bg-slate-950/60">
          <template #header>
            <h2 class="text-lg font-semibold text-white">Upcoming tasks</h2>
          </template>
          <div class="space-y-4">
            <div
              v-for="task in upcomingTasks"
              :key="task.id"
              class="flex items-start justify-between gap-4"
            >
              <div>
                <p class="text-sm font-medium text-white">
                  {{ task.label }}
                </p>
                <p class="text-xs text-slate-500">Owner - {{ task.owner }}</p>
              </div>
              <UBadge color="info" variant="soft" :label="task.status" />
            </div>
          </div>
        </UCard>
      </div>
    </section>
  </div>
</template>
