import type { Meta, StoryObj } from '@storybook/vue3-vite'

import Avatar from './Avatar.vue'

const meta = {
  title: 'UI/Avatar',
  component: Avatar,
  tags: ['autodocs'],
  argTypes: {
    size: { control: 'select', options: ['xs', 'sm', 'md', 'lg', 'xl'] },
  },
  args: {
    name: 'Alex Johnson',
    src: undefined,
    size: 'sm',
  },
} satisfies Meta<typeof Avatar>

export default meta

type Story = StoryObj<typeof meta>

export const Initials: Story = {
  args: {
    src: undefined,
  },
}

export const WithImage: Story = {
  args: {
    src: 'https://i.pravatar.cc/300?img=5',
  },
}

export const Sizes: Story = {
  render: (args) => ({
    components: { Avatar },
    setup() {
      const sizes = ['xs', 'sm', 'md', 'lg', 'xl'] as const
      return { args, sizes }
    },
    template: `
      <div class="flex items-center gap-4">
        <Avatar v-for="size in sizes" :key="size" v-bind="args" :size="size" />
      </div>
    `,
  }),
  args: {
    src: 'https://i.pravatar.cc/300?img=8',
  },
}
