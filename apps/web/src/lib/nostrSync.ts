import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { storeEvents } from '@/lib/nostrEventCache';
import { WEB_KIND } from '@/lib/clawstr';

const SYNC_KEY = '__oaNostrSync';
const LAST_SYNC_KEY = 'openagents-sync-last';
const SYNC_INTERVAL_MS = 2 * 60 * 1000;
const DEFAULT_LOOKBACK_SECONDS = 60 * 60;

type NostrQueryClient = {
  query: (
    filters: Array<NostrFilter>,
    opts?: { signal?: AbortSignal; relays?: Array<string> },
  ) => Promise<Array<NostrEvent>>;
};

type SyncState = {
  intervalId?: number;
  running?: boolean;
  nostr?: NostrQueryClient;
};

function getState(): SyncState {
  const scope = globalThis as typeof globalThis & { [SYNC_KEY]?: SyncState };
  if (!scope[SYNC_KEY]) scope[SYNC_KEY] = {};
  return scope[SYNC_KEY];
}

function loadLastSync(): number | null {
  if (typeof window === 'undefined') return null;
  try {
    const raw = window.localStorage.getItem(LAST_SYNC_KEY);
    if (!raw) return null;
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function saveLastSync(value: number) {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(LAST_SYNC_KEY, String(value));
  } catch {
    // ignore
  }
}

async function syncOnce(state: SyncState) {
  if (state.running) return;
  if (!state.nostr) return;
  if (typeof navigator !== 'undefined' && navigator.onLine === false) return;

  state.running = true;
  try {
    const now = Math.floor(Date.now() / 1000);
    const lastSync = loadLastSync();
    const since = lastSync ?? now - DEFAULT_LOOKBACK_SECONDS;
    const filter: NostrFilter = {
      kinds: [1111],
      '#K': [WEB_KIND],
      since,
      limit: 200,
    };
    const signal = AbortSignal.timeout(8000);
    const events = await state.nostr.query([filter], { signal });
    if (events.length > 0) {
      await storeEvents(events);
      const latest = events.reduce(
        (max, ev) => (ev.created_at > max ? ev.created_at : max),
        since,
      );
      saveLastSync(latest + 1);
    } else {
      saveLastSync(now - 30);
    }
  } catch {
    // ignore sync errors
  } finally {
    state.running = false;
  }
}

export function startNostrCacheSync(nostr: NostrQueryClient) {
  if (typeof window === 'undefined') return;
  const state = getState();
  state.nostr = nostr;
  if (state.intervalId) return;
  state.intervalId = window.setInterval(() => {
    void syncOnce(state);
  }, SYNC_INTERVAL_MS);
  void syncOnce(state);
}
