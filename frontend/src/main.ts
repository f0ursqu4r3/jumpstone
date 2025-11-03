import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'
import router from './router'
import ui from '@nuxt/ui/vue-plugin'
import '@/assets/css/main.css'
import { useSessionStore } from '~/stores/session'

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(router)
app.use(ui)

if (typeof window !== 'undefined') {
  const sessionStore = useSessionStore(pinia)

  const initializeSession = async () => {
    sessionStore.hydrate()

    if (!sessionStore.isAuthenticated) {
      return
    }

    const refreshed = await sessionStore.ensureFreshAccessToken().catch((err) => {
      console.warn('Failed to refresh access token during hydration', err)
      return false
    })

    if (!refreshed && !sessionStore.isAuthenticated) {
      return
    }

    if (!sessionStore.isAuthenticated) {
      return
    }

    try {
      await sessionStore.fetchProfile()
    } catch (err) {
      console.warn('Failed to hydrate profile', err)
    }
  }

  router.isReady().then(() => {
    initializeSession()
  })
}

app.mount('#app')
