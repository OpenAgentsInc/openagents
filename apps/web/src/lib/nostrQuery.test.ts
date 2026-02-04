import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { queryWithFallback } from './nostrQuery';
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

type TestEvent = { created_at: number };

describe('queryWithFallback', () => {
  beforeEach(() => {
    mockQueryCachedEvents.mockResolvedValue([]);
    mockStoreEvents.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.clearAllMocks();
    delete (globalThis as { navigator?: { onLine?: boolean } }).navigator;
  });

  it('returns cached results when offline', async () => {
    (globalThis as { navigator?: { onLine?: boolean } }).navigator = { onLine: false };
    const cached: TestEvent[] = [{ created_at: 100 }];
    mockQueryCachedEvents.mockResolvedValue(cached);
    const nostr = { query: vi.fn().mockResolvedValue([]) };

    const result = await queryWithFallback(nostr, [{}]);
    expect(result).toEqual(cached);
    expect(nostr.query).not.toHaveBeenCalled();
  });

  it('returns primary results and stores them', async () => {
    const primary: TestEvent[] = [{ created_at: 200 }];
    const nostr = { query: vi.fn().mockResolvedValue(primary) };

    const result = await queryWithFallback(nostr, [{}]);
    expect(result).toEqual(primary);
    expect(mockStoreEvents).toHaveBeenCalledWith(primary);
  });

  it('returns cached when primary empty and fallback disabled', async () => {
    const cached: TestEvent[] = [{ created_at: 300 }];
    mockQueryCachedEvents.mockResolvedValue(cached);
    const nostr = { query: vi.fn().mockResolvedValue([]) };

    const result = await queryWithFallback(nostr, [{}], { fallbackOnEmpty: false });
    expect(result).toEqual(cached);
    expect(nostr.query).toHaveBeenCalledTimes(1);
  });

  it('falls back when primary results are below minResults', async () => {
    const primary: TestEvent[] = [{ created_at: 400 }];
    const fallback: TestEvent[] = [{ created_at: 500 }];
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
    const opts = secondCall?.[1] as { relays?: string[] } | undefined;
    expect(opts?.relays).toEqual([
      'wss://relay.one',
      ...DEFAULT_RELAYS,
    ]);
    expect(mockGetConfiguredRelays).toHaveBeenCalled();
  });
});
