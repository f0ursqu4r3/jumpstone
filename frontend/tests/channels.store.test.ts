import { beforeEach, describe, expect, it } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useChannelStore } from '../app/stores/channels';

describe('useChannelStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('hydrates channels and assigns default for the active guild', () => {
    const store = useChannelStore();

    store.hydrate();
    store.setActiveGuild('openguild');

    expect(store.channelsForGuild.length).toBeGreaterThan(0);
    expect(store.activeChannel?.id).toBe(store.channelsForGuild[0]?.id);
  });

  it('allows selecting a specific channel within the active guild', () => {
    const store = useChannelStore();
    store.hydrate();
    store.setActiveGuild('openguild');

    const targetChannel = store.channelsForGuild[1]?.id;
    expect(targetChannel).toBeTruthy();

    store.setActiveChannel(targetChannel!);

    expect(store.activeChannel?.id).toBe(targetChannel);
    expect(store.error).toBeNull();
  });

  it('guards against selecting channels that do not exist', () => {
    const store = useChannelStore();
    store.hydrate();
    store.setActiveGuild('openguild');

    store.setActiveChannel('missing-channel');

    expect(store.error).toBe('Unknown channel: missing-channel');
    expect(store.activeChannel?.id).not.toBe('missing-channel');
  });
});
