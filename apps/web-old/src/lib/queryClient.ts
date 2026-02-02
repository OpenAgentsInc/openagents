import { QueryClient } from "@tanstack/react-query";

const QUERY_CLIENT_KEY = "__OA_QUERY_CLIENT__";
const PERSIST_SETUP_KEY = "__OA_QUERY_CLIENT_PERSIST__";
const PERSIST_KEY = "clawstr-query-cache-v1";
const PERSIST_TTL_MS = 5 * 60 * 1000;
const MAX_ENTRIES = 100;

type PersistedEntry = {
  key: unknown[];
  data: unknown;
  updatedAt: number;
};

type PersistedCache = {
  timestamp: number;
  entries: PersistedEntry[];
};

const PERSIST_DENYLIST = new Set([
  "posts",
  "subclaw-posts",
  "author-posts",
  "discovered-subclaws",
  "post-replies",
  "post-replies-thread",
  "batch-reply-counts-global",
]);

function isClawstrKey(key: unknown): key is unknown[] {
  if (!Array.isArray(key) || key[0] !== "clawstr") return false;
  const scope = typeof key[1] === "string" ? key[1] : "";
  return !PERSIST_DENYLIST.has(scope);
}

function serializeData(data: unknown): unknown {
  if (data instanceof Map) {
    return { __type: "map", value: Array.from(data.entries()) };
  }
  return data;
}

function deserializeData(data: unknown): unknown {
  if (
    data &&
    typeof data === "object" &&
    (data as { __type?: string }).__type === "map" &&
    Array.isArray((data as { value?: unknown }).value)
  ) {
    return new Map((data as { value: [unknown, unknown][] }).value);
  }
  return data;
}

function loadPersistedCache(client: QueryClient) {
  if (typeof window === "undefined") return;
  try {
    const raw = window.localStorage.getItem(PERSIST_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as PersistedCache;
    if (!parsed?.entries?.length) return;
    const now = Date.now();
    for (const entry of parsed.entries) {
      if (!isClawstrKey(entry.key)) continue;
      if (now - entry.updatedAt > PERSIST_TTL_MS) continue;
      client.setQueryData(entry.key, deserializeData(entry.data));
    }
  } catch {
    // ignore cache restore errors
  }
}

let persistTimeout: number | null = null;

function savePersistedCache(client: QueryClient) {
  if (typeof window === "undefined") return;
  try {
    const queries = client.getQueryCache().getAll();
    const entries = queries
      .filter((q) => isClawstrKey(q.queryKey) && q.state.status === "success")
      .sort((a, b) => b.state.dataUpdatedAt - a.state.dataUpdatedAt)
      .slice(0, MAX_ENTRIES)
      .map((q) => ({
        key: q.queryKey,
        data: serializeData(q.state.data),
        updatedAt: q.state.dataUpdatedAt,
      }));
    const payload: PersistedCache = {
      timestamp: Date.now(),
      entries,
    };
    window.localStorage.setItem(PERSIST_KEY, JSON.stringify(payload));
  } catch {
    // ignore cache save errors (quota, JSON issues, etc.)
  }
}

function schedulePersist(client: QueryClient) {
  if (typeof window === "undefined") return;
  if (persistTimeout != null) return;
  persistTimeout = window.setTimeout(() => {
    persistTimeout = null;
    savePersistedCache(client);
  }, 1000);
}

function setupPersistence(client: QueryClient) {
  if (typeof window === "undefined") return;
  const scope = globalThis as typeof globalThis & {
    [PERSIST_SETUP_KEY]?: boolean;
  };
  if (scope[PERSIST_SETUP_KEY]) return;
  scope[PERSIST_SETUP_KEY] = true;

  loadPersistedCache(client);
  client.getQueryCache().subscribe(() => schedulePersist(client));
  window.addEventListener("beforeunload", () => savePersistedCache(client));
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") savePersistedCache(client);
  });
}

export function getQueryClient(): QueryClient {
  const scope = globalThis as typeof globalThis & {
    [QUERY_CLIENT_KEY]?: QueryClient;
  };
  if (!scope[QUERY_CLIENT_KEY]) {
    scope[QUERY_CLIENT_KEY] = new QueryClient();
  }
  setupPersistence(scope[QUERY_CLIENT_KEY]);
  return scope[QUERY_CLIENT_KEY];
}
