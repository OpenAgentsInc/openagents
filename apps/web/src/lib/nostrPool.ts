import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import { NPool, NRelay1 } from '@nostrify/nostrify';
import { DEFAULT_RELAYS } from '@/lib/relayConfig';
import {
  pickReadRelays,
  recordRelayClose,
  recordRelayError,
  recordRelayOpen,
} from '@/lib/relayHealth';

const RELAY_LIST_KEY = '__oaRelays';

const POOL_CACHE_KEY = '__OA_NOSTR_POOL_CACHE__';

type PoolCache = Map<string, NPool>;

function getPoolCache(): PoolCache {
  const scope = globalThis as typeof globalThis & { [POOL_CACHE_KEY]?: PoolCache };
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
  const key = relays.join('|');
  const cache = getPoolCache();
  const existing = cache.get(key);
  if (existing) return existing;

  const pool = new NPool({
    open(url: string) {
      const createdAt = Date.now();
      return new NRelay1(url, {
        log: (log) => {
          if (log.ns === 'relay.ws.state') {
            const state = (log as { state?: string }).state;
            if (state === 'open') {
              recordRelayOpen(url, Date.now() - createdAt);
            } else if (state === 'close') {
              recordRelayClose(url);
            }
          } else if (log.ns === 'relay.ws.error') {
            recordRelayError(url);
          } else if (log.ns === 'relay.ws.retry') {
            recordRelayError(url);
          }
        },
      });
    },
    reqRouter(_filters: NostrFilter[]) {
      const routes = new Map<string, NostrFilter[]>();
      const readRelays = pickReadRelays(relays);
      for (const url of readRelays) {
        routes.set(url, _filters);
      }
      return routes;
    },
    eventRouter(_event: NostrEvent) {
      return [...relays];
    },
  });

  cache.set(key, pool);
  (pool as unknown as { [RELAY_LIST_KEY]?: string[] })[RELAY_LIST_KEY] = relays;
  return pool;
}

export function getConfiguredRelays(nostr: unknown): string[] {
  if (!nostr || typeof nostr !== 'object') return DEFAULT_RELAYS;
  const relays = (nostr as { [RELAY_LIST_KEY]?: string[] })[RELAY_LIST_KEY];
  return Array.isArray(relays) && relays.length > 0 ? relays : DEFAULT_RELAYS;
}
