/* @vitest-environment jsdom */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { startNostrCacheSync } from './nostrSync';
import { storeEvents } from './nostrEventCache';

vi.mock('./nostrEventCache', () => ({
  storeEvents: vi.fn(),
}));

const mockStoreEvents = vi.mocked(storeEvents);

const SYNC_KEY = '__oaNostrSync';
const LAST_SYNC_KEY = 'openagents-sync-last';

function resetSyncState() {
  delete (globalThis as Record<string, unknown>)[SYNC_KEY];
}

describe('nostrSync', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
    resetSyncState();
    mockStoreEvents.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
    localStorage.clear();
    resetSyncState();
    vi.clearAllMocks();
    vi.unstubAllGlobals();
  });

  it('skips sync when offline', async () => {
    vi.stubGlobal('navigator', { onLine: false });
    const query = vi.fn().mockResolvedValue([]);

    startNostrCacheSync({ query });
    await Promise.resolve();

    expect(query).not.toHaveBeenCalled();
    expect(localStorage.getItem(LAST_SYNC_KEY)).toBeNull();
  });

  it('stores events and updates last sync', async () => {
    vi.stubGlobal('navigator', { onLine: true });
    const nowMs = 1_700_000_000_000;
    vi.spyOn(Date, 'now').mockReturnValue(nowMs);
    const since = Math.floor(nowMs / 1000) - 60 * 60;
    const events = [{ created_at: since + 100 } as { created_at: number }];
    const query = vi.fn().mockResolvedValue(events);

    startNostrCacheSync({ query });
    await Promise.resolve();
    await Promise.resolve();

    expect(query).toHaveBeenCalledTimes(1);
    expect(mockStoreEvents).toHaveBeenCalledWith(events);
    expect(localStorage.getItem(LAST_SYNC_KEY)).toBe(String(events[0].created_at + 1));
  });

  it('updates last sync when no events are found', async () => {
    vi.stubGlobal('navigator', { onLine: true });
    vi.spyOn(Date, 'now').mockReturnValue(1_700_000_050_000);
    const query = vi.fn().mockResolvedValue([]);

    startNostrCacheSync({ query });
    await Promise.resolve();
    await Promise.resolve();

    expect(query).toHaveBeenCalledTimes(1);
    expect(localStorage.getItem(LAST_SYNC_KEY)).toBe('1700000020');
  });
});
