<template>
  <div class="flex flex-col gap-6">
    <section
      class="rounded-xl border border-surface-muted bg-background-elevated/70 p-6 shadow-elevated-sm"
    >
      <header class="flex items-center justify-between">
        <div>
          <p class="text-xs uppercase tracking-widest text-slate-500">
            OpenGuild Frontend
          </p>
          <h1 class="text-2xl font-semibold text-white">
            Week 1-2 delivery checkpoints
          </h1>
        </div>
        <UiBadge variant="outline">
          F0
        </UiBadge>
      </header>
      <ul class="mt-6 space-y-3 text-sm text-slate-300">
        <li
          v-for="milestone in roadmap"
          :key="milestone.id"
          class="flex items-start gap-3 rounded-lg border border-surface-muted/60 bg-background/60 px-4 py-3"
        >
          <div class="mt-1 h-2 w-2 rounded-full bg-brand-accent" />
          <div>
            <p class="font-medium text-white">
              {{ milestone.title }}
            </p>
            <p class="text-xs uppercase tracking-widest text-slate-500">
              {{ milestone.category }}
            </p>
            <p class="mt-2 text-slate-400">
              {{ milestone.summary }}
            </p>
          </div>
        </li>
      </ul>
    </section>

    <section
      class="rounded-xl border border-surface-muted bg-background-elevated/80 p-6 shadow-elevated-sm"
    >
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-xl font-semibold text-white">
            Timeline prototype
          </h2>
          <p class="text-sm text-slate-400">
            Virtualized view will land once channel events API scaffolding is wired.
          </p>
        </div>
        <UiBadge variant="ghost" size="sm">
          demo
        </UiBadge>
      </header>

      <div class="mt-5 space-y-5">
        <article
          v-for="message in messages"
          :key="message.id"
          class="group flex gap-4 rounded-lg border border-transparent px-3 py-2 transition hover:border-surface-muted/60 hover:bg-background-elevated/60"
        >
          <UiAvatar
            :name="message.author.name"
            :status="message.author.status"
            size="sm"
          />
          <div class="flex-1 space-y-2">
            <div class="flex flex-wrap items-center gap-2 text-sm text-slate-400">
              <span class="font-semibold text-slate-100">
                {{ message.author.name }}
              </span>
              <span class="text-xs text-slate-500">
                {{ message.timestamp }}
              </span>
              <UiBadge
                v-if="message.author.role"
                variant="outline"
                size="sm"
              >
                {{ message.author.role }}
              </UiBadge>
            </div>
            <p class="whitespace-pre-line text-slate-200">
              {{ message.content }}
            </p>
            <div
              v-if="message.reactions.length"
              class="flex flex-wrap gap-2"
            >
              <UiTooltip
                v-for="reaction in message.reactions"
                :key="reaction.emoji"
                :text="reaction.users.join(', ')"
              >
                <span
                  class="inline-flex items-center gap-2 rounded-full border border-surface-muted bg-background/80 px-3 py-1 text-xs text-slate-300"
                >
                  {{ reaction.emoji }}
                  <span class="font-medium text-white">
                    {{ reaction.count }}
                  </span>
                </span>
              </UiTooltip>
            </div>
          </div>
        </article>
      </div>
    </section>

    <section
      class="rounded-xl border border-surface-muted bg-background-elevated/80 p-6 shadow-elevated-sm"
    >
      <h2 class="text-xl font-semibold text-white">
        Composer stub
      </h2>
      <p class="mt-1 text-sm text-slate-400">
        Hook this form up to the messaging API client once the HTTP wrapper lands.
      </p>
      <form class="mt-4 space-y-4">
        <UiInput
          v-model="composer.deviceName"
          label="Device label"
          placeholder="My laptop"
          hint="Appears in session security settings"
        />
        <UiInput
          v-model="composer.message"
          label="Message"
          placeholder="Share progress..."
        />
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3 text-xs text-slate-500">
            <UiBadge variant="ghost">
              Markdown
            </UiBadge>
            <span>Emoji picker + uploads arrive in Week 5</span>
          </div>
          <UiButton
            type="button"
            :disabled="composer.message.length === 0"
          >
            Send preview
          </UiButton>
        </div>
      </form>
    </section>
  </div>
</template>

<script setup lang="ts">
import { reactive } from 'vue';

type Message = {
  id: string;
  timestamp: string;
  content: string;
  author: {
    name: string;
    role?: string;
    status: 'online' | 'idle' | 'dnd' | 'offline';
  };
  reactions: Array<{
    emoji: string;
    count: number;
    users: string[];
  }>;
};

const roadmap = [
  {
    id: 'dev-workflow',
    title: 'Developer workflow parity',
    category: 'F0',
    summary:
      'Document Bun/npm parity, surface lint/test/build commands, and wire CI scaffolding.',
  },
  {
    id: 'design-shell',
    title: 'Design system shell',
    category: 'F0',
    summary:
      'Baseline Tailwind tokens, layout frame, and core UI components (button, input, badge, avatar, tooltip).',
  },
  {
    id: 'stores-api',
    title: 'State + API foundation',
    category: 'F0',
    summary:
      'Stub Pinia session/guild/channel stores and begin the typed HTTP client with retry/backoff.',
  },
];

const messages: Message[] = [
  {
    id: '1',
    timestamp: 'Today at 09:14',
    content:
      'Frontend skeleton is live. Next up: hook the HTTP client and session flow to the backend login endpoint.',
    author: {
      name: 'Alex Carter',
      role: 'frontend',
      status: 'online',
    },
    reactions: [
      { emoji: ':rocket:', count: 3, users: ['Kai', 'Morgan', 'Jules'] },
      { emoji: ':white_check_mark:', count: 1, users: ['Priya'] },
    ],
  },
  {
    id: '2',
    timestamp: 'Today at 09:32',
    content:
      'Reminder: add vitest coverage for the session store mutations when the testing scaffold lands.',
    author: {
      name: 'Priya Shah',
      role: 'qa',
      status: 'idle',
    },
    reactions: [{ emoji: ':memo:', count: 2, users: ['Kai', 'Alex'] }],
  },
];

const composer = reactive({
  deviceName: 'Alice Laptop',
  message: '',
});
</script>
