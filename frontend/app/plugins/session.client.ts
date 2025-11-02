import { useSessionStore } from '~/stores/session';

export default defineNuxtPlugin(async () => {
  if (import.meta.server) {
    return;
  }

  const session = useSessionStore();
  session.hydrate();

  if (session.isAuthenticated) {
    try {
      await session.fetchProfile();
    } catch (err) {
      console.warn('Failed to hydrate profile', err);
    }
  }
});
