import type { Preview } from '@storybook/vue3'
import { setup } from '@storybook/vue3'

import ui from '@nuxt/ui/vue-plugin'
import { TooltipProvider } from 'reka-ui'

import '@/assets/css/main.css'

setup((app) => {
  app.use(ui)
})

const preview: Preview = {
  decorators: [
    (story) => ({
      components: { story, TooltipProvider },
      template: '<TooltipProvider><UApp><story /></UApp></TooltipProvider>',
    }),
  ],
  controls: {
    matchers: {
      color: /(background|color)$/i,
      date: /Date$/i,
    },
  },

  a11y: {
    // 'todo' - show a11y violations in the test UI only
    // 'error' - fail CI on a11y violations
    // 'off' - skip a11y checks entirely
    test: 'todo',
  },
}

export default preview
