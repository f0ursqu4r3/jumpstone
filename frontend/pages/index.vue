<template>
  <div class="flex flex-col gap-6">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between">
          <div>
            <p class="text-xs uppercase tracking-widest text-slate-500">
              OpenGuild Frontend
            </p>
            <h1 class="text-2xl font-semibold text-white">
              Week 1-2 delivery checkpoints
            </h1>
          </div>
          <UBadge color="primary" variant="outline">
            F0
          </UBadge>
        </div>
      </template>
      <ul class="space-y-3 text-sm text-slate-300">
        <li
          v-for="milestone in roadmap"
          :key="milestone.id"
          class="flex items-start gap-3 rounded-lg border border-surface-muted/60 bg-background/60 px-4 py-3"
        >
          <span class="mt-1 h-2 w-2 rounded-full bg-brand-accent" />
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
    </UCard>

    <UCard>
      <template #header>
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-xl font-semibold text-white">
              Timeline prototype
            </h2>
            <p class="text-sm text-slate-400">
              Virtualized view will land once channel events API scaffolding is wired.
            </p>
          </div>
          <UBadge variant="ghost" color="gray" size="sm">
            demo
          </UBadge>
        </div>
      </template>
      <div class="space-y-5">
        <article
          v-for="message in messages"
          :key="message.id"
          class="group flex gap-4 rounded-lg border border-transparent px-3 py-2 transition hover:border-surface-muted/60 hover:bg-background-elevated/60"
        >
          <UAvatar
            :text="initials(message.author.name)"
            size="sm"
            class="bg-surface-subtle text-slate-100"
          />
          <div class="flex-1 space-y-2">
            <div class="flex flex-wrap items-center gap-2 text-sm text-slate-400">
              <span class="font-semibold text-slate-100">
                {{ message.author.name }}
              </span>
              <span class="text-xs text-slate-500">
                {{ message.timestamp }}
              </span>
              <UBadge
                v-if="message.author.role"
                color="primary"
                variant="outline"
                size="sm"
              >
                {{ message.author.role }}
              </UBadge>
            </div>
            <p class="whitespace-pre-line text-slate-200">
              {{ message.content }}
            </p>
            <div
              v-if="message.reactions.length"
              class="flex flex-wrap gap-2"
            >
              <UTooltip
                v-for="reaction in message.reactions"
                :key="reaction.emoji"
                :text="reaction.users.join(', ')"
                :open-delay="100"
              >
                <span
                  class="inline-flex items-center gap-2 rounded-full border border-surface-muted bg-background/80 px-3 py-1 text-xs text-slate-300"
                >
                  {{ resolveEmoji(reaction.emoji) }}
                  <span class="font-medium text-white">
                    {{ reaction.count }}
                  </span>
                </span>
              </UTooltip>
            </div>
          </div>
        </article>
      </div>
    </UCard>

    <UCard>
      <template #header>
        <div>
          <h2 class="text-xl font-semibold text-white">
            Composer stub
          </h2>
          <p class="mt-1 text-sm text-slate-400">
            Hook this form up to the messaging API client once the HTTP wrapper lands.
          </p>
        </div>
      </template>
      <form class="space-y-4">
        <UFormGroup
          label="Device label"
          description="Appears in session security settings"
        >
          <UInput
            v-model="composer.deviceName"
            placeholder="My laptop"
          />
        </UFormGroup>
        <UFormGroup label="Message">
          <UTextarea
            v-model="composer.message"
            placeholder="Share progress..."
            autoresize
            :rows="3"
          />
        </UFormGroup>
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex items-center gap-3 text-xs text-slate-500">
            <UBadge variant="ghost" color="gray">
              Markdown
            </UBadge>
            <span>Emoji picker + uploads arrive in Week 5</span>
          </div>
          <UButton
            type="button"
            color="primary"
            :disabled="composer.message.length === 0"
            icon="i-heroicons-paper-airplane"
          >
            Send preview
          </UButton>
        </div>
      </form>
    </UCard>
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

const emojiMap: Record<string, string> = {
  ':rocket:': '\u{1F680}',
  ':white_check_mark:': '\u{2705}',
  ':memo:': '\u{1F4DD}',
};

const resolveEmoji = (value: string) => emojiMap[value] ?? value;

const initials = (value: string) => {
  const name = value.trim();
  if (!name) {
    return '?';
  }
  const parts = name.split(/\s+/);
  if (parts.length === 1) {
    return parts[0][0]?.toUpperCase() ?? '?';
  }
  const first = parts[0][0] ?? '';
  const last = parts[parts.length - 1][0] ?? '';
  return `${first}${last}`.toUpperCase();
};
</script>
