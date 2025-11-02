import { useSessionStore } from '~/stores/session';

export default defineNuxtPlugin(() => {
  if (import.meta.server) {
    return;
  }

  const session = useSessionStore();
  session.hydrate();
});
