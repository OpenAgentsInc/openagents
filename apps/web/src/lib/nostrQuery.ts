import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { getConfiguredRelays } from '@/lib/nostrPool';
import { DEFAULT_RELAYS } from '@/lib/relayConfig';
import { queryCachedEvents, storeEvents } from '@/lib/nostrEventCache';

type NostrQueryClient = {
  query(
    filters: NostrFilter[],
    opts?: { signal?: AbortSignal; relays?: string[] },
  ): Promise<NostrEvent[]>;
};

type QueryOptions = {
  signal?: AbortSignal;
  timeoutMs?: number;
  fallbackOnEmpty?: boolean;
  forceFallbackOnEmpty?: boolean;
  minResults?: number;
};

const DAY_SECONDS = 86400;
const DEFAULT_FALLBACK_WINDOW = 7 * DAY_SECONDS;
const VOTE_FALLBACK_WINDOW = 2 * DAY_SECONDS;
const PROFILE_FALLBACK_WINDOW = 30 * DAY_SECONDS;

function getFallbackWindowSeconds(filters: NostrFilter[]): number {
  const kinds = new Set<number>();
  for (const filter of filters) {
    if (filter.kinds) filter.kinds.forEach((kind) => kinds.add(kind));
  }
  if (kinds.has(7) || kinds.has(9735)) return VOTE_FALLBACK_WINDOW;
  if (kinds.has(0)) return PROFILE_FALLBACK_WINDOW;
  return DEFAULT_FALLBACK_WINDOW;
}

function getLatestSeen(events: NostrEvent[]): number | null {
  if (events.length === 0) return null;
  let latest = 0;
  for (const event of events) {
    if (event.created_at > latest) latest = event.created_at;
  }
  return latest > 0 ? latest : null;
}

function shouldEscalateFallback(events: NostrEvent[], filters: NostrFilter[]): boolean {
  const latest = getLatestSeen(events);
  if (!latest) return false;
  const windowSeconds = getFallbackWindowSeconds(filters);
  const now = Math.floor(Date.now() / 1000);
  return now - latest <= windowSeconds;
}

function combineSignals(signal?: AbortSignal, timeoutMs?: number): AbortSignal | undefined {
  const signals: AbortSignal[] = [];
  if (signal) signals.push(signal);
  if (typeof timeoutMs === 'number' && timeoutMs > 0) {
    signals.push(AbortSignal.timeout(timeoutMs));
  }
  if (signals.length === 0) return undefined;
  if (signals.length === 1) return signals[0];
  return AbortSignal.any(signals);
}

export async function queryWithFallback(
  nostr: NostrQueryClient,
  filters: NostrFilter[],
  options: QueryOptions = {},
): Promise<NostrEvent[]> {
  const signal = combineSignals(options.signal, options.timeoutMs);
  let cached: NostrEvent[] = [];
  try {
    cached = await queryCachedEvents(filters);
  } catch {
    cached = [];
  }

  const offline = typeof navigator !== 'undefined' && navigator.onLine === false;
  if (offline && cached.length > 0) return cached;

  let primary: NostrEvent[] = [];
  try {
    primary = await nostr.query(filters, { signal });
  } catch {
    return cached.length > 0 ? cached : [];
  }

  const minResults =
    typeof options.minResults === 'number' && options.minResults > 0
      ? options.minResults
      : undefined;
  const primaryTooSmall = minResults != null && primary.length < minResults;

  if (primary.length > 0) {
    void storeEvents(primary);
    if (!primaryTooSmall) {
      return primary;
    }
  }

  if (options.fallbackOnEmpty === false) {
    return cached.length > 0 ? cached : primary;
  }
  if (!options.forceFallbackOnEmpty && !primaryTooSmall && !shouldEscalateFallback(cached, filters)) {
    return cached.length > 0 ? cached : primary;
  }

  const configuredRelays = getConfiguredRelays(nostr);
  const allRelays =
    configuredRelays.length <= 1
      ? [...new Set([...configuredRelays, ...DEFAULT_RELAYS])]
      : configuredRelays;
  if (allRelays.length <= 1) return cached.length > 0 ? cached : primary;
  let fallback: NostrEvent[] = [];
  try {
    fallback = await nostr.query(filters, { signal, relays: allRelays });
  } catch {
    fallback = [];
  }
  if (fallback.length > 0) {
    void storeEvents(fallback);
    return fallback;
  }

  return cached.length > 0 ? cached : primary;
}
