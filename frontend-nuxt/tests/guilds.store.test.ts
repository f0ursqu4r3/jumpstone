declare const beforeEach: any;
declare const describe: any;
declare const expect: any;
declare const it: any;

import { createPinia, setActivePinia } from 'pinia';
import { useGuildStore } from '../app/stores/guilds';

describe('useGuildStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('hydrates guilds with a default active selection', () => {
    const store = useGuildStore();

    store.hydrate();

    expect(store.guilds.length).toBeGreaterThan(0);
    expect(store.hydrated).toBe(true);
    expect(store.activeGuild?.id).toBe(store.guilds[0]?.id);
  });

  it('updates the active guild when requested', () => {
    const store = useGuildStore();
    store.hydrate();

    const nextGuild = store.guilds[1]?.id;
    expect(nextGuild).toBeTruthy();

    store.setActiveGuild(nextGuild!);

    expect(store.activeGuild?.id).toBe(nextGuild);
    expect(store.error).toBeNull();
  });

  it('guards against selecting unknown guilds', () => {
    const store = useGuildStore();
    store.hydrate();

    store.setActiveGuild('unknown');

    expect(store.error).toBe('Unknown guild: unknown');
    expect(store.activeGuild?.id).not.toBe('unknown');
  });
});
