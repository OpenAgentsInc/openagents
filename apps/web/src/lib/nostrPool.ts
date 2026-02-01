import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { NPool, NRelay1 } from "@nostrify/nostrify";
import { DEFAULT_RELAYS } from "@/lib/relayConfig";

const POOL_CACHE_KEY = "__OA_NOSTR_POOL_CACHE__";

type PoolCache = Map<string, NPool>;

function getPoolCache(): PoolCache {
  const scope = globalThis as typeof globalThis & {
    [POOL_CACHE_KEY]?: PoolCache;
  };
  if (!scope[POOL_CACHE_KEY]) {
    scope[POOL_CACHE_KEY] = new Map();
  }
  return scope[POOL_CACHE_KEY];
}

function normalizeRelays(relayUrls: string[]): string[] {
  const relays = relayUrls.length > 0 ? relayUrls : DEFAULT_RELAYS;
  const unique = [...new Set(relays)];
  unique.sort();
  return unique;
}

export function getNostrPool(relayUrls: string[]): NPool {
  const relays = normalizeRelays(relayUrls);
  const key = relays.join("|");
  const cache = getPoolCache();
  const existing = cache.get(key);
  if (existing) return existing;

  const pool = new NPool({
    open(url: string) {
      return new NRelay1(url);
    },
    reqRouter(_filters: NostrFilter[]) {
      const routes = new Map<string, NostrFilter[]>();
      for (const url of relays) {
        routes.set(url, _filters);
      }
      return routes;
    },
    eventRouter(_event: NostrEvent) {
      return [...relays];
    },
  });

  cache.set(key, pool);
  return pool;
}
