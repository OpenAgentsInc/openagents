import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { getConfiguredRelays } from "@/lib/nostrPool";
import { queryCachedEvents, storeEvents } from "@/lib/nostrEventCache";

type NostrQueryClient = {
  query(
    filters: NostrFilter[],
    opts?: { signal?: AbortSignal; relays?: string[] }
  ): Promise<NostrEvent[]>;
};

type QueryOptions = {
  signal?: AbortSignal;
  timeoutMs?: number;
  fallbackOnEmpty?: boolean;
};

function combineSignals(signal?: AbortSignal, timeoutMs?: number): AbortSignal | undefined {
  const signals: AbortSignal[] = [];
  if (signal) signals.push(signal);
  if (typeof timeoutMs === "number" && timeoutMs > 0) {
    signals.push(AbortSignal.timeout(timeoutMs));
  }
  if (signals.length === 0) return undefined;
  if (signals.length === 1) return signals[0];
  return AbortSignal.any(signals);
}

export async function queryWithFallback(
  nostr: NostrQueryClient,
  filters: NostrFilter[],
  options: QueryOptions = {}
): Promise<NostrEvent[]> {
  const signal = combineSignals(options.signal, options.timeoutMs);
  let cached: NostrEvent[] = [];
  try {
    cached = await queryCachedEvents(filters);
  } catch {
    cached = [];
  }

  const offline = typeof navigator !== "undefined" && navigator.onLine === false;
  if (offline && cached.length > 0) return cached;

  let primary: NostrEvent[] = [];
  try {
    primary = await nostr.query(filters, { signal });
  } catch {
    return cached.length > 0 ? cached : [];
  }

  if (primary.length > 0) {
    void storeEvents(primary);
    return primary;
  }

  if (options.fallbackOnEmpty === false) {
    return cached.length > 0 ? cached : primary;
  }

  const allRelays = getConfiguredRelays(nostr);
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
