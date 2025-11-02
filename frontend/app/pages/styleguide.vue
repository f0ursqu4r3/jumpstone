<script setup lang="ts">
const brandPalette = [
  {
    label: 'Sky 500',
    token: 'primary',
    class: 'bg-sky-500',
    hex: '#0EA5E9',
    usage: 'Primary actions, active outlines',
  },
  {
    label: 'Sky 200',
    token: 'primary-light',
    class: 'bg-sky-200 text-slate-900',
    hex: '#BAE6FD',
    usage: 'Soft fills, badges, hover states',
  },
  {
    label: 'Slate 950',
    token: 'surface-base',
    class: 'bg-slate-950',
    hex: '#020617',
    usage: 'App shell background',
  },
  {
    label: 'Slate 800',
    token: 'surface-elevated',
    class: 'bg-slate-800',
    hex: '#1F2937',
    usage: 'Cards, secondary rails',
  },
  {
    label: 'Slate 400',
    token: 'text-muted',
    class: 'bg-slate-400 text-slate-900',
    hex: '#94A3B8',
    usage: 'Captions, helper copy',
  },
  {
    label: 'Emerald 500',
    token: 'state-positive',
    class: 'bg-emerald-500',
    hex: '#10B981',
    usage: 'Success badges, online states',
  },
  {
    label: 'Rose 500',
    token: 'state-critical',
    class: 'bg-rose-500',
    hex: '#F43F5E',
    usage: 'Alerts, destructive actions',
  },
];

const typographyScale = [
  {
    token: 'display',
    sample: 'Display - 40px / 48px',
    class: 'text-4xl sm:text-5xl font-semibold tracking-tight',
    notes: 'Hero headlines and milestone banners.',
  },
  {
    token: 'title',
    sample: 'Title - 24px / 32px',
    class: 'text-2xl font-semibold',
    notes: 'Section titles, card headers.',
  },
  {
    token: 'body',
    sample: 'Body - 16px / 24px',
    class: 'text-base',
    notes: 'Primary copy across pages.',
  },
  {
    token: 'caption',
    sample: 'Caption - 12px / 16px',
    class: 'text-xs uppercase tracking-wide',
    notes: 'Metadata, subtle labels.',
  },
];

const spacingScale = [
  {
    token: 'space-1',
    value: '4px',
    usage: 'Button icon gutters, tight stack spacing.',
  },
  {
    token: 'space-2',
    value: '8px',
    usage: 'Inline badges, compact list padding.',
  },
  { token: 'space-3', value: '12px', usage: 'Card metadata separation.' },
  {
    token: 'space-4',
    value: '16px',
    usage: 'Default block spacing and grid gaps.',
  },
  {
    token: 'space-6',
    value: '24px',
    usage: 'Card inner padding, section separation.',
  },
  { token: 'space-8', value: '32px', usage: 'Page-level vertical rhythm.' },
];
</script>

<template>
  <div class="space-y-10">
    <section>
      <UCard class="border border-white/5 bg-slate-950/70">
        <template #header>
          <div class="flex flex-col gap-1">
            <h1 class="text-3xl font-semibold text-white">
              Frontend styleguide
            </h1>
            <p class="text-sm text-slate-400">
              Reference palette, typography, layout rhythm, and Nuxt UI
              primitives to keep new components aligned.
            </p>
          </div>
        </template>
        <div class="flex flex-wrap gap-3">
          <UBadge color="primary" variant="soft" label="Nuxt UI v4" />
          <UBadge color="secondary" variant="soft" label="Tailwind tokens" />
          <UBadge color="neutral" variant="soft" label="Dark theme baseline" />
        </div>
      </UCard>
    </section>

    <section class="grid gap-6 lg:grid-cols-2">
      <UCard class="border border-white/5 bg-slate-950/70">
        <template #header>
          <div class="flex items-baseline justify-between gap-4">
            <div>
              <h2 class="text-lg font-semibold text-white">Palette</h2>
              <p class="text-sm text-slate-400">
                Tokens used across the shell. Reference token names in Tailwind
                config overrides.
              </p>
            </div>
            <UBadge label="brand.ts" color="primary" variant="soft" />
          </div>
        </template>
        <div class="grid gap-4 sm:grid-cols-2">
          <div
            v-for="color in brandPalette"
            :key="color.token"
            class="rounded-xl border border-white/5 bg-slate-900/80 p-4"
          >
            <div
              :class="[
                'h-20 w-full rounded-lg border border-white/5',
                color.class,
              ]"
            />
            <div class="mt-4 space-y-1 text-sm">
              <p class="font-semibold text-white">
                {{ color.label }}
              </p>
              <p class="text-xs text-slate-400">
                {{ color.hex }} - token:
                <code class="text-slate-200">{{ color.token }}</code>
              </p>
              <p class="text-xs text-slate-500">
                {{ color.usage }}
              </p>
            </div>
          </div>
        </div>
      </UCard>

      <UCard class="border border-white/5 bg-slate-950/70">
        <template #header>
          <div class="flex items-baseline justify-between gap-4">
            <h2 class="text-lg font-semibold text-white">Typography</h2>
            <UBadge color="primary" variant="soft" label="Inter" />
          </div>
        </template>
        <div class="space-y-6">
          <div
            v-for="type in typographyScale"
            :key="type.token"
            class="space-y-2"
          >
            <p :class="[type.class, 'text-white']">
              {{ type.sample }}
            </p>
            <div
              class="flex items-baseline justify-between text-xs text-slate-500"
            >
              <span
                >token:
                <code class="text-slate-200">{{ type.token }}</code></span
              >
              <span>{{ type.notes }}</span>
            </div>
            <USeparator />
          </div>
        </div>
      </UCard>
    </section>

    <section class="grid gap-6 lg:grid-cols-[2fr_1fr]">
      <UCard class="border border-white/5 bg-slate-950/70">
        <template #header>
          <div class="flex items-baseline justify-between gap-4">
            <h2 class="text-lg font-semibold text-white">Component system</h2>
            <UBadge color="primary" variant="outline" label="Nuxt UI" />
          </div>
        </template>
        <div class="grid gap-6 md:grid-cols-2">
          <div class="space-y-3">
            <p class="text-xs uppercase tracking-wide text-slate-400">
              Buttons
            </p>
            <div class="flex flex-wrap gap-3">
              <UButton
                label="Primary"
                color="primary"
                icon="i-heroicons-rocket-launch"
              />
              <UButton label="Secondary" color="secondary" variant="soft" />
              <UButton
                icon="i-heroicons-plus"
                color="secondary"
                variant="ghost"
                aria-label="Add"
              />
              <UButton label="Destructive" color="error" variant="solid" />
            </div>
            <p class="text-xs text-slate-500">
              Use `color="primary"` for primary actions, `variant="ghost"` for
              toolbar affordances.
            </p>
          </div>

          <div class="space-y-3">
            <p class="text-xs uppercase tracking-wide text-slate-400">Inputs</p>
            <div class="space-y-3">
              <UInput label="Project name" placeholder="OpenGuild" />
              <UTextarea
                label="Description"
                placeholder="Short summary for the release note..."
              />
              <USelect
                label="Channel kind"
                :options="[
                  { label: 'Text channel', value: 'text' },
                  { label: 'Voice channel', value: 'voice' },
                  { label: 'Announcement', value: 'announcement' },
                ]"
                placeholder="Choose type"
              />
            </div>
            <p class="text-xs text-slate-500">
              Pair inline validation with UAlert set to variant="soft" for
              non-blocking errors.
            </p>
          </div>

          <div class="space-y-3">
            <p class="text-xs uppercase tracking-wide text-slate-400">Cards</p>
            <UCard class="border border-white/5 bg-slate-900/70">
              <template #header>
                <div class="flex items-center justify-between gap-3">
                  <span class="text-sm font-semibold text-white"
                    >Feature spec</span
                  >
                  <UBadge color="primary" variant="soft" label="Draft" />
                </div>
              </template>
              <p class="text-sm text-slate-300">
                Use cards to hold timeline entries, QA checklists, and release
                notes. Maintain `24px` padding for readability.
              </p>
              <template #footer>
                <div class="flex items-center gap-2 text-xs text-slate-500">
                  <UIcon name="i-heroicons-user-circle" class="h-4 w-4" />
                  <span>Owner - Lia Chen</span>
                </div>
              </template>
            </UCard>
          </div>

          <div class="space-y-3">
            <p class="text-xs uppercase tracking-wide text-slate-400">
              Feedback
            </p>
            <div class="flex flex-col gap-3">
              <UAlert
                title="Connected"
                description="All services responding within budget."
                color="primary"
                icon="i-heroicons-check-circle"
              />
              <UAlert
                title="Retry queued"
                description="We will retry in 30 seconds if the request keeps failing."
                color="primary"
                icon="i-heroicons-arrow-path"
                variant="soft"
              />
              <UAlert
                title="Action required"
                description="Missing guild permissions for this task."
                color="error"
                icon="i-heroicons-no-symbol"
              />
            </div>
            <p class="text-xs text-slate-500">
              Reserve solid alerts for blocking states; soften for informative
              callouts.
            </p>
          </div>
        </div>
      </UCard>

      <UCard class="border border-white/5 bg-slate-950/70">
        <template #header>
          <h2 class="text-lg font-semibold text-white">
            Spacing & layout rhythm
          </h2>
        </template>
        <table class="w-full table-fixed text-left text-sm text-slate-300">
          <thead class="text-xs uppercase tracking-wide text-slate-400">
            <tr>
              <th class="pb-2">Token</th>
              <th class="pb-2">Value</th>
              <th class="pb-2">Usage</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/5">
            <tr v-for="spacing in spacingScale" :key="spacing.token">
              <td class="py-3 font-semibold text-white">
                {{ spacing.token }}
              </td>
              <td class="py-3 text-slate-400">
                {{ spacing.value }}
              </td>
              <td class="py-3 text-slate-500">
                {{ spacing.usage }}
              </td>
            </tr>
          </tbody>
        </table>
        <p class="mt-4 text-xs text-slate-500">
          Use multiples of `space-4` (16px) for vertical rhythm; add `space-6`
          (24px) padding inside surface cards.
        </p>
      </UCard>
    </section>
  </div>
</template>
