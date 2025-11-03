import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { ref } from 'vue'

import Input from './Input.vue'

const meta = {
  title: 'UI/Input',
  component: Input,
  tags: ['autodocs'],
  argTypes: {
    size: { control: 'select', options: ['sm', 'md', 'lg'] },
    color: { control: 'select', options: ['neutral', 'info'] },
    variant: { control: 'select', options: ['soft', 'outline', 'ghost'] },
  },
  args: {
    label: 'Email',
    hint: 'We will never share your email.',
    error: null,
    size: 'md',
    color: 'neutral',
    variant: 'soft',
    modelValue: '',
  },
  render: (args) => ({
    components: { Input },
    setup() {
      const value = ref(args.modelValue ?? '')
      const handleUpdate = (next: string | number) => {
        value.value = typeof next === 'number' ? String(next) : next
      }
      return { args, value, handleUpdate }
    },
    template: `
      <div class="max-w-xs space-y-2">
        <Input v-bind="args" :model-value="value" @update:model-value="handleUpdate" />
        <p class="text-xs text-slate-500">Current value: {{ value }}</p>
      </div>
    `,
  }),
} satisfies Meta<typeof Input>

export default meta

type Story = StoryObj<typeof meta>

export const Playground: Story = {}

export const WithError: Story = {
  args: {
    error: 'Please provide a valid email address.',
  },
}

export const WithIcon: Story = {
  args: {
    icon: 'i-heroicons-envelope',
  },
}
