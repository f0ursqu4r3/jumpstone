import type { Meta, StoryObj } from '@storybook/vue3-vite'

import Tooltip from './Tooltip.vue'

const meta = {
  title: 'UI/Tooltip',
  component: Tooltip,
  tags: ['autodocs'],
  argTypes: {
    placement: {
      control: 'select',
      options: [
        'top',
        'bottom',
        'left',
        'right',
        'top-start',
        'top-end',
        'bottom-start',
        'bottom-end',
        'left-start',
        'left-end',
        'right-start',
        'right-end',
      ],
    },
  },
  args: {
    text: 'Helpful hint',
    placement: 'top',
    openDelay: 0,
    closeDelay: 0,
  },
  render: (args) => ({
    components: { Tooltip },
    setup() {
      return { args }
    },
    template: `
      <Tooltip v-bind="args">
        <button class="rounded-lg bg-surface-700 px-4 py-2 text-sm font-medium text-white">
          Hover me
        </button>
      </Tooltip>
    `,
  }),
} satisfies Meta<typeof Tooltip>

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {}

export const Delayed: Story = {
  args: {
    openDelay: 300,
    closeDelay: 150,
  },
}

export const CustomContent: Story = {
  render: (args) => ({
    components: { Tooltip },
    setup() {
      return { args }
    },
    template: `
      <Tooltip v-bind="args">
        <button class="rounded-lg bg-surface-700 px-4 py-2 text-sm font-medium text-white">
          Hover me
        </button>
        <template #content>
          <div class="max-w-xs space-y-1">
            <p class="text-sm font-semibold">Keyboard shortcut</p>
            <p class="text-xs text-slate-400">Press <kbd class="rounded border border-slate-500 px-1">Cmd+K</kbd> to launch search.</p>
          </div>
        </template>
      </Tooltip>
    `,
  }),
  args: {
    text: undefined,
  },
}
