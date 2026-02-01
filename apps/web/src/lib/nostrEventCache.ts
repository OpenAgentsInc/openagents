import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { matchFilters } from "nostr-tools";

const DB_NAME = "clawstr-events-v1";
const STORE_NAME = "events";
const DB_VERSION = 1;
const DEFAULT_LIMIT = 200;

type CachedEvent = NostrEvent & {
  identifier?: string;
  parent_id?: string;
};

let dbPromise: Promise<IDBDatabase | null> | null = null;

function openDb(): Promise<IDBDatabase | null> {
  if (typeof indexedDB === "undefined") return Promise.resolve(null);
  if (dbPromise) return dbPromise;
  dbPromise = new Promise((resolve) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        const store = db.createObjectStore(STORE_NAME, { keyPath: "id" });
        store.createIndex("kind", "kind", { unique: false });
        store.createIndex("created_at", "created_at", { unique: false });
        store.createIndex("pubkey", "pubkey", { unique: false });
        store.createIndex("identifier", "identifier", { unique: false });
        store.createIndex("parent_id", "parent_id", { unique: false });
        store.createIndex("kind_identifier", ["kind", "identifier"], { unique: false });
        store.createIndex("kind_pubkey", ["kind", "pubkey"], { unique: false });
        store.createIndex("kind_parent", ["kind", "parent_id"], { unique: false });
        store.createIndex("kind_created_at", ["kind", "created_at"], { unique: false });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => resolve(null);
  });
  return dbPromise;
}

function waitForTx(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

function getIdentifier(tags: string[][]): string | undefined {
  const tag = tags.find(([name]) => name === "I") ?? tags.find(([name]) => name === "i");
  return tag?.[1];
}

function getParentId(tags: string[][]): string | undefined {
  const tag = tags.find(([name]) => name === "e");
  return tag?.[1];
}

function toCachedEvent(event: NostrEvent): CachedEvent {
  return {
    ...event,
    identifier: getIdentifier(event.tags),
    parent_id: getParentId(event.tags),
  };
}

function toLimit(filters: NostrFilter[]): number {
  let limit = DEFAULT_LIMIT;
  for (const filter of filters) {
    if (typeof filter.limit === "number") {
      limit = Math.max(limit, filter.limit);
    }
  }
  return limit;
}

export async function storeEvents(events: NostrEvent[]): Promise<void> {
  const db = await openDb();
  if (!db || events.length === 0) return;
  const tx = db.transaction(STORE_NAME, "readwrite");
  const store = tx.objectStore(STORE_NAME);
  for (const event of events) {
    store.put(toCachedEvent(event));
  }
  await waitForTx(tx);
}

async function collectFromIndex(
  store: IDBObjectStore,
  indexName: string,
  range: IDBKeyRange,
  limit: number,
  direction: IDBCursorDirection = "next"
): Promise<CachedEvent[]> {
  return new Promise((resolve) => {
    const result: CachedEvent[] = [];
    const index = store.index(indexName);
    const request = index.openCursor(range, direction);
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor || result.length >= limit) {
        resolve(result);
        return;
      }
      result.push(cursor.value as CachedEvent);
      cursor.continue();
    };
    request.onerror = () => resolve(result);
  });
}

async function collectByIds(store: IDBObjectStore, ids: string[]): Promise<CachedEvent[]> {
  const results: CachedEvent[] = [];
  for (const id of ids) {
    const request = store.get(id);
    const value = await new Promise<CachedEvent | undefined>((resolve) => {
      request.onsuccess = () => resolve(request.result as CachedEvent | undefined);
      request.onerror = () => resolve(undefined);
    });
    if (value) results.push(value);
  }
  return results;
}

async function candidatesForFilter(
  store: IDBObjectStore,
  filter: NostrFilter,
  limit: number
): Promise<CachedEvent[]> {
  if (filter.ids?.length) {
    return collectByIds(store, filter.ids);
  }

  const singleKind = filter.kinds && filter.kinds.length === 1 ? filter.kinds[0] : null;
  const singleAuthor = filter.authors && filter.authors.length === 1 ? filter.authors[0] : null;
  const identifier =
    filter["#I"]?.[0] ?? filter["#i"]?.[0] ?? filter["#I"]?.[1] ?? filter["#i"]?.[1];
  const parentId = filter["#e"]?.[0];

  if (singleKind != null && singleAuthor) {
    const range = IDBKeyRange.only([singleKind, singleAuthor]);
    return collectFromIndex(store, "kind_pubkey", range, limit, "prev");
  }

  if (singleKind != null && identifier) {
    const range = IDBKeyRange.only([singleKind, identifier]);
    return collectFromIndex(store, "kind_identifier", range, limit, "prev");
  }

  if (singleKind != null && parentId) {
    const range = IDBKeyRange.only([singleKind, parentId]);
    return collectFromIndex(store, "kind_parent", range, limit, "next");
  }

  if (singleKind != null) {
    const since = typeof filter.since === "number" ? filter.since : 0;
    const until =
      typeof filter.until === "number" ? filter.until : Number.MAX_SAFE_INTEGER;
    const range = IDBKeyRange.bound([singleKind, since], [singleKind, until]);
    return collectFromIndex(store, "kind_created_at", range, limit, "prev");
  }

  return [];
}

export async function queryCachedEvents(filters: NostrFilter[]): Promise<NostrEvent[]> {
  const db = await openDb();
  if (!db) return [];
  const tx = db.transaction(STORE_NAME, "readonly");
  const store = tx.objectStore(STORE_NAME);
  const limit = toLimit(filters);
  const results = new Map<string, CachedEvent>();

  for (const filter of filters) {
    const candidates = await candidatesForFilter(store, filter, limit);
    for (const event of candidates) {
      if (matchFilters([filter], event)) {
        results.set(event.id, event);
        if (results.size >= limit) break;
      }
    }
    if (results.size >= limit) break;
  }

  await waitForTx(tx);
  return [...results.values()];
}
