import { useSessionStore } from '~/stores/session';

export default defineNuxtPlugin((nuxtApp) => {
  if (import.meta.server) {
    return;
  }

  const session = useSessionStore();

  const initializeSession = async () => {
    session.hydrate();

    if (!session.isAuthenticated) {
      return;
    }

    const refreshed = await session.ensureFreshAccessToken().catch((err) => {
      console.warn('Failed to refresh access token during hydration', err);
      return false;
    });

    if (!refreshed && !session.isAuthenticated) {
      return;
    }

    if (!session.isAuthenticated) {
      return;
    }

    try {
      await session.fetchProfile();
    } catch (err) {
      console.warn('Failed to hydrate profile', err);
    }
  };

  if (nuxtApp.payload.serverRendered) {
    nuxtApp.hook('app:mounted', () => {
      initializeSession();
    });
  } else {
    initializeSession();
  }
});
