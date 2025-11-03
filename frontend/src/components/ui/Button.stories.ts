import type { Meta, StoryObj } from '@storybook/vue3-vite'

import Button from './Button.vue'

const meta = {
  title: 'UI/Button',
  component: Button,
  tags: ['autodocs'],
  argTypes: {
    color: {
      control: 'select',
      options: ['primary', 'secondary', 'success', 'info', 'warning', 'error', 'neutral'],
    },
    variant: {
      control: 'select',
      options: ['solid', 'soft', 'ghost', 'outline', 'link', 'subtle'],
    },
    size: { control: 'select', options: ['xs', 'sm', 'md', 'lg', 'xl'] },
    block: { control: 'boolean' },
    loading: { control: 'boolean' },
  },
  args: {
    color: 'primary',
    variant: 'soft',
    size: 'md',
    label: 'Click me',
  block: false,
  loading: false,
  },
  render: (args) => ({
    components: { Button },
    setup() {
      return { args }
    },
    template: `<Button v-bind="args">{{ args.label }}</Button>`,
  }),
} satisfies Meta<typeof Button>

export default meta

type Story = StoryObj<typeof meta>

export const Playground: Story = {}

export const Solid: Story = {
  args: {
    variant: 'solid',
  },
}

export const Ghost: Story = {
  args: {
    variant: 'ghost',
  },
}

export const Loading: Story = {
  args: {
    loading: true,
    label: 'Saving...',
  },
}

export const Block: Story = {
  args: {
    block: true,
    label: 'Full width action',
  },
}
