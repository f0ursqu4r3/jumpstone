import { useSessionStore } from '~/stores/session';

export default defineNuxtPlugin(async () => {
  if (import.meta.server) {
    return;
  }

  const session = useSessionStore();
  session.hydrate();

  if (session.isAuthenticated) {
    const refreshed = await session.ensureFreshAccessToken().catch((err) => {
      console.warn('Failed to refresh access token during hydration', err);
      return false;
    });

    if (!refreshed && !session.isAuthenticated) {
      return;
    }

    if (session.isAuthenticated) {
      try {
        await session.fetchProfile();
      } catch (err) {
        console.warn('Failed to hydrate profile', err);
      }
    }
  }
});
