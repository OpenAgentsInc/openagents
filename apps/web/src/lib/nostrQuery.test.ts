import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { queryWithFallback } from './nostrQuery';
import type { NostrEvent } from '@nostrify/nostrify';
import { queryCachedEvents, storeEvents } from '@/lib/nostrEventCache';
import { getConfiguredRelays } from '@/lib/nostrPool';
import { DEFAULT_RELAYS } from '@/lib/relayConfig';

vi.mock('@/lib/nostrEventCache', () => ({
  queryCachedEvents: vi.fn(),
  storeEvents: vi.fn(),
}));

vi.mock('@/lib/nostrPool', () => ({
  getConfiguredRelays: vi.fn(() => ['wss://relay.one']),
}));

vi.mock('@/lib/relayConfig', () => ({
  DEFAULT_RELAYS: ['wss://relay.default1', 'wss://relay.default2'],
}));

const mockQueryCachedEvents = vi.mocked(queryCachedEvents);
const mockStoreEvents = vi.mocked(storeEvents);
const mockGetConfiguredRelays = vi.mocked(getConfiguredRelays);

const makeEvent = (created_at: number): NostrEvent => ({
  id: `id-${created_at}`,
  pubkey: 'pubkey',
  created_at,
  kind: 1,
  tags: [],
  content: '',
  sig: 'sig',
});

describe('queryWithFallback', () => {
  beforeEach(() => {
    mockQueryCachedEvents.mockResolvedValue([]);
    mockStoreEvents.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
  });

  it('returns cached results when offline', async () => {
    vi.stubGlobal('navigator', { onLine: false });
    const cached: Array<NostrEvent> = [makeEvent(100)];
    mockQueryCachedEvents.mockResolvedValue(cached);
    const nostr = { query: vi.fn().mockResolvedValue([]) };

    const result = await queryWithFallback(nostr, [{}]);
    expect(result).toEqual(cached);
    expect(nostr.query).not.toHaveBeenCalled();
  });

  it('returns primary results and stores them', async () => {
    const primary: Array<NostrEvent> = [makeEvent(200)];
    const nostr = { query: vi.fn().mockResolvedValue(primary) };

    const result = await queryWithFallback(nostr, [{}]);
    expect(result).toEqual(primary);
    expect(mockStoreEvents).toHaveBeenCalledWith(primary);
  });

  it('returns cached when primary empty and fallback disabled', async () => {
    const cached: Array<NostrEvent> = [makeEvent(300)];
    mockQueryCachedEvents.mockResolvedValue(cached);
    const nostr = { query: vi.fn().mockResolvedValue([]) };

    const result = await queryWithFallback(nostr, [{}], { fallbackOnEmpty: false });
    expect(result).toEqual(cached);
    expect(nostr.query).toHaveBeenCalledTimes(1);
  });

  it('falls back when primary results are below minResults', async () => {
    const primary: Array<NostrEvent> = [makeEvent(400)];
    const fallback: Array<NostrEvent> = [makeEvent(500)];
    const nostr = {
      query: vi
        .fn()
        .mockResolvedValueOnce(primary)
        .mockResolvedValueOnce(fallback),
    };

    const result = await queryWithFallback(nostr, [{}], { minResults: 2 });
    expect(result).toEqual(fallback);
    expect(nostr.query).toHaveBeenCalledTimes(2);

    const secondCall = nostr.query.mock.calls[1];
    const opts = (secondCall[1] ?? {}) as { relays?: Array<string> };
    expect(opts.relays).toEqual([
      'wss://relay.one',
      ...DEFAULT_RELAYS,
    ]);
    expect(mockGetConfiguredRelays).toHaveBeenCalled();
  });
});
