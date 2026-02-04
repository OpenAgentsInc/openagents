import { NPool, NRelay1 } from '@nostrify/nostrify';
import type { NostrEvent, NostrFilter } from '@nostrify/nostrify';
import type { RelayEntry } from '@/lib/relayConfig';
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

type RelayInput = Array<RelayEntry> | Array<string>;

function isRelayEntry(value: RelayEntry | string): value is RelayEntry {
  return typeof value !== 'string';
}

function normalizeRelayInput(relayInput: RelayInput): {
  readRelays: Array<string>;
  writeRelays: Array<string>;
  allRelays: Array<string>;
  key: string;
} {
  const entries: Array<RelayEntry> = relayInput.length
    ? relayInput.map((entry) =>
        isRelayEntry(entry)
          ? entry
          : { url: entry, read: true, write: true },
      )
    : DEFAULT_RELAYS.map((url) => ({ url, read: true, write: true }));

  const readRelays = [
    ...new Set(
      entries.filter((entry) => entry.read).map((entry) => entry.url),
    ),
  ];
  const writeRelays = [
    ...new Set(
      entries.filter((entry) => entry.write).map((entry) => entry.url),
    ),
  ];
  const allRelays = [...new Set([...readRelays, ...writeRelays])];
  const fallbackRelays = allRelays.length > 0 ? allRelays : DEFAULT_RELAYS;
  const normalizedRead = readRelays.length > 0 ? readRelays : fallbackRelays;
  const normalizedWrite = writeRelays.length > 0 ? writeRelays : fallbackRelays;
  const key = [
    `r:${[...normalizedRead].sort().join(',')}`,
    `w:${[...normalizedWrite].sort().join(',')}`,
  ].join('|');

  return {
    readRelays: normalizedRead,
    writeRelays: normalizedWrite,
    allRelays: fallbackRelays,
    key,
  };
}

export function getNostrPool(relayInput: RelayInput): NPool {
  const { readRelays, writeRelays, key } = normalizeRelayInput(relayInput);
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
    reqRouter(_filters: Array<NostrFilter>) {
      const routes = new Map<string, Array<NostrFilter>>();
      const selected = pickReadRelays(readRelays);
      for (const url of selected) {
        routes.set(url, _filters);
      }
      return routes;
    },
    eventRouter(_event: NostrEvent) {
      return [...writeRelays];
    },
  });

  cache.set(key, pool);
  (pool as unknown as { [RELAY_LIST_KEY]?: Array<string> })[RELAY_LIST_KEY] =
    readRelays;
  return pool;
}

export function getConfiguredRelays(nostr: unknown): Array<string> {
  if (!nostr || typeof nostr !== 'object') return DEFAULT_RELAYS;
  const relays = (nostr as { [RELAY_LIST_KEY]?: Array<string> })[RELAY_LIST_KEY];
  return Array.isArray(relays) && relays.length > 0 ? relays : DEFAULT_RELAYS;
}
