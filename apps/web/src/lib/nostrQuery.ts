import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { getConfiguredRelays } from "@/lib/nostrPool";

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
  const primary = await nostr.query(filters, { signal });
  if (primary.length > 0 || options.fallbackOnEmpty === false) return primary;

  const allRelays = getConfiguredRelays(nostr);
  if (allRelays.length <= 1) return primary;
  const fallback = await nostr.query(filters, { signal, relays: allRelays });
  return fallback.length > 0 ? fallback : primary;
}
