import type { Meta, StoryObj } from '@storybook/vue3-vite'

import Badge from './Badge.vue'

const meta = {
  title: 'UI/Badge',
  component: Badge,
  tags: ['autodocs'],
  argTypes: {
    color: {
      control: 'select',
      options: ['primary', 'secondary', 'info', 'success', 'warning', 'error', 'neutral'],
    },
    variant: {
      control: 'select',
      options: ['solid', 'soft', 'outline', 'subtle'],
    },
    size: { control: 'select', options: ['xs', 'sm', 'md', 'lg', 'xl'] },
  },
  args: {
    label: 'Active',
    color: 'primary',
    variant: 'soft',
    size: 'sm',
  },
} satisfies Meta<typeof Badge>

export default meta

type Story = StoryObj<typeof meta>

export const Playground: Story = {}

export const Variants: Story = {
  render: (args) => ({
    components: { Badge },
    setup() {
      const variants = ['solid', 'soft', 'outline', 'subtle'] as const
      return { args, variants }
    },
    template: `
      <div class="flex flex-wrap gap-3">
        <Badge
          v-for="variant in variants"
          :key="variant"
          v-bind="args"
          :variant="variant"
        >
          {{ variant }}
        </Badge>
      </div>
    `,
  }),
}

export const Palette: Story = {
  render: (args) => ({
    components: { Badge },
    setup() {
      const colors = ['primary', 'secondary', 'info', 'success', 'warning', 'error', 'neutral'] as const
      return { args, colors }
    },
    template: `
      <div class="flex flex-wrap gap-3">
        <Badge
          v-for="color in colors"
          :key="color"
          v-bind="args"
          :color="color"
        >
          {{ color }}
        </Badge>
      </div>
    `,
  }),
}
