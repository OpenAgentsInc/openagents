import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { matchFilters } from "nostr-tools";

const DB_NAME = "clawstr-events-v1";
const STORE_NAME = "events";
const METRICS_STORE = "metrics";
const DB_VERSION = 2;
const DEFAULT_LIMIT = 200;
const MAX_EVENT_COUNT = 5000;
const MAX_METRIC_COUNT = 4000;

type CachedEvent = NostrEvent & {
  identifier?: string;
  parent_id?: string;
};

type MetricEntry = {
  key: string;
  event_id: string;
  type: "votes" | "zaps" | "replies" | "replies-ai" | "replies-all";
  data: unknown;
  updated_at: number;
};

let pruneInFlight = false;

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
      if (!db.objectStoreNames.contains(METRICS_STORE)) {
        const metrics = db.createObjectStore(METRICS_STORE, { keyPath: "key" });
        metrics.createIndex("updated_at", "updated_at", { unique: false });
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
  void pruneEvents();
}

async function pruneEvents() {
  if (pruneInFlight) return;
  pruneInFlight = true;
  try {
    const db = await openDb();
    if (!db) return;
    const tx = db.transaction(STORE_NAME, "readwrite");
    const store = tx.objectStore(STORE_NAME);
    const count = await new Promise<number>((resolve) => {
      const req = store.count();
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => resolve(0);
    });
    if (count <= MAX_EVENT_COUNT) {
      await waitForTx(tx);
      return;
    }
    let toDelete = count - MAX_EVENT_COUNT;
    const index = store.index("created_at");
    const cursorReq = index.openCursor(null, "next");
    cursorReq.onsuccess = () => {
      const cursor = cursorReq.result;
      if (!cursor || toDelete <= 0) return;
      cursor.delete();
      toDelete -= 1;
      cursor.continue();
    };
    await waitForTx(tx);
  } finally {
    pruneInFlight = false;
  }
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

export async function getCachedMetrics<T>(
  eventIds: string[],
  type: MetricEntry["type"],
  maxAgeMs: number
): Promise<{ data: Map<string, T>; missing: string[] }> {
  const db = await openDb();
  if (!db || eventIds.length === 0) {
    return { data: new Map(), missing: eventIds };
  }
  const tx = db.transaction(METRICS_STORE, "readonly");
  const store = tx.objectStore(METRICS_STORE);
  const now = Date.now();
  const data = new Map<string, T>();
  const missing: string[] = [];

  for (const id of eventIds) {
    const key = `${type}:${id}`;
    const entry = await new Promise<MetricEntry | undefined>((resolve) => {
      const req = store.get(key);
      req.onsuccess = () => resolve(req.result as MetricEntry | undefined);
      req.onerror = () => resolve(undefined);
    });
    if (entry && now - entry.updated_at <= maxAgeMs) {
      data.set(id, entry.data as T);
    } else {
      missing.push(id);
    }
  }

  await waitForTx(tx);
  return { data, missing };
}

export async function storeMetrics<T>(
  type: MetricEntry["type"],
  entries: Map<string, T>
): Promise<void> {
  const db = await openDb();
  if (!db || entries.size === 0) return;
  const tx = db.transaction(METRICS_STORE, "readwrite");
  const store = tx.objectStore(METRICS_STORE);
  const now = Date.now();
  for (const [id, value] of entries) {
    const entry: MetricEntry = {
      key: `${type}:${id}`,
      event_id: id,
      type,
      data: value,
      updated_at: now,
    };
    store.put(entry);
  }
  await waitForTx(tx);
  void pruneMetrics();
}

async function pruneMetrics() {
  const db = await openDb();
  if (!db) return;
  const tx = db.transaction(METRICS_STORE, "readwrite");
  const store = tx.objectStore(METRICS_STORE);
  const count = await new Promise<number>((resolve) => {
    const req = store.count();
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => resolve(0);
  });
  if (count <= MAX_METRIC_COUNT) {
    await waitForTx(tx);
    return;
  }
  let toDelete = count - MAX_METRIC_COUNT;
  const index = store.index("updated_at");
  const cursorReq = index.openCursor(null, "next");
  cursorReq.onsuccess = () => {
    const cursor = cursorReq.result;
    if (!cursor || toDelete <= 0) return;
    cursor.delete();
    toDelete -= 1;
    cursor.continue();
  };
  await waitForTx(tx);
}
