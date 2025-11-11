import { createRouter, createWebHistory } from 'vue-router'
import { storeToRefs } from 'pinia'

import DashboardView from '~/views/DashboardView.vue'
import LoginView from '~/views/LoginView.vue'
import MessagesView from '~/views/MessagesView.vue'
import RegisterView from '~/views/RegisterView.vue'
import RoadmapView from '~/views/RoadmapView.vue'
import StyleguideView from '~/views/StyleguideView.vue'
import { useSessionStore } from '~/stores/session'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'home',
      component: MessagesView,
      meta: {
        layout: 'messages',
        requiresAuth: true,
      },
    },
    {
      path: '/dashboard',
      name: 'dashboard',
      component: DashboardView,
      meta: {
        requiresAuth: true,
      },
    },
    {
      path: '/login',
      name: 'login',
      component: LoginView,
      meta: {
        layout: 'auth',
        requiresAuth: false,
      },
    },
    {
      path: '/register',
      name: 'register',
      component: RegisterView,
      meta: {
        layout: 'auth',
        requiresAuth: false,
      },
    },
    {
      path: '/roadmap',
      name: 'roadmap',
      component: RoadmapView,
      meta: {
        requiresAuth: true,
      },
    },
    {
      path: '/styleguide',
      name: 'styleguide',
      component: StyleguideView,
      meta: {
        requiresAuth: false,
      },
    },
  ],
})

const sanitizeRedirect = (value: unknown): string | null => {
  if (typeof value !== 'string') {
    return null
  }

  if (!value.startsWith('/')) {
    return null
  }

  if (value === '/login' || value === '/register') {
    return '/'
  }

  return value
}

router.beforeEach((to, from, next) => {
  const session = useSessionStore()
  const { hydrated, tokens, isAuthenticated } = storeToRefs(session)

  if (!hydrated.value) {
    session.hydrate()
  }

  if (!isAuthenticated.value && tokens.value) {
    session.logout()
  }

  const requiresAuth = to.meta.requiresAuth !== false

  if (requiresAuth && !isAuthenticated.value) {
    const redirect = to.path === '/login' ? null : to.fullPath
    return next(
      redirect
        ? { path: '/login', query: { redirect } }
        : { path: '/login' },
    )
  }

  if (
    !requiresAuth &&
    isAuthenticated.value &&
    (to.path === '/login' || to.path === '/register')
  ) {
    const candidate = sanitizeRedirect(to.query.redirect)
    return next(candidate ?? '/')
  }

  return next()
})

export default router
