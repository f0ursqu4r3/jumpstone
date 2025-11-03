import { useSessionStore } from '~/stores/session';

const sanitizeRedirect = (value: unknown): string | null => {
  if (typeof value !== 'string') {
    return null;
  }

  if (!value.startsWith('/')) {
    return null;
  }

  return value;
};

export default defineNuxtRouteMiddleware((to) => {
  if (import.meta.server) {
    return;
  }

  const session = useSessionStore();

  if (!session.hydrated) {
    session.hydrate();
  }

  if (!session.isAuthenticated && session.tokens) {
    session.logout();
  }

  const requiresAuth = to.meta.auth !== false;

  if (requiresAuth && !session.isAuthenticated) {
    const redirect = to.path === '/login' ? null : to.fullPath;
    return navigateTo(
      redirect
        ? { path: '/login', query: { redirect } }
        : '/login'
    );
  }

  if (!requiresAuth && session.isAuthenticated && to.path === '/login') {
    const candidate = sanitizeRedirect(to.query.redirect);
    return navigateTo(candidate ?? '/');
  }
});
