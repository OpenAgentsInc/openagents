import { describe, expect, it } from 'vitest';
import { DEFAULT_RELAYS } from '@/lib/relayConfig';
import { getConfiguredRelays } from '@/lib/nostrPool';

describe('nostrPool.getConfiguredRelays', () => {
  it('returns defaults for invalid input', () => {
    expect(getConfiguredRelays(null)).toEqual(DEFAULT_RELAYS);
    expect(getConfiguredRelays(undefined)).toEqual(DEFAULT_RELAYS);
    expect(getConfiguredRelays('not-an-object')).toEqual(DEFAULT_RELAYS);
  });

  it('returns configured relays when present', () => {
    const relays = ['wss://relay.one', 'wss://relay.two'];
    const nostr = { __oaRelays: relays };
    expect(getConfiguredRelays(nostr)).toEqual(relays);
  });

  it('falls back to defaults when configured relays are empty', () => {
    const nostr = { __oaRelays: [] as string[] };
    expect(getConfiguredRelays(nostr)).toEqual(DEFAULT_RELAYS);
  });
});
