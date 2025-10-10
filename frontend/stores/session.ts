import { defineStore } from 'pinia';

type SessionState = {
  userId: string | null;
  accessToken: string | null;
};

export const useSessionStore = defineStore('session', {
  state: (): SessionState => ({
    userId: null,
    accessToken: null,
  }),
  actions: {
    setSession(userId: string, accessToken: string) {
      this.userId = userId;
      this.accessToken = accessToken;
    },
    clearSession() {
      this.userId = null;
      this.accessToken = null;
    },
  },
});
