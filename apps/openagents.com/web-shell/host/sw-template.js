const CACHE_PREFIX = "openagents-web-shell::";
const BUILD_ID = "__OA_BUILD_ID__";
const CACHE_NAME = `${CACHE_PREFIX}${BUILD_ID}`;
const ROLLBACK_CACHE_NAMES = __OA_ROLLBACK_CACHE_NAMES__;
const PINNED_ASSETS = __OA_PINNED_ASSETS__;
const INDEX_PATH = "/index.html";
const MANIFEST_PATH = "/manifest.json";

const PINNED_PATH_SET = new Set(
  PINNED_ASSETS.map((asset) => new URL(asset, self.location.origin).pathname),
);
const PRECACHE_ASSETS = Array.from(new Set([...PINNED_ASSETS, MANIFEST_PATH]));

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE_NAME);
      await cache.addAll(PRECACHE_ASSETS);
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keep = new Set([CACHE_NAME, ...ROLLBACK_CACHE_NAMES]);
      const keys = await caches.keys();
      await Promise.all(
        keys
          .filter((key) => key.startsWith(CACHE_PREFIX) && !keep.has(key))
          .map((key) => caches.delete(key)),
      );
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("message", (event) => {
  if (event.data?.type === "SKIP_WAITING") {
    self.skipWaiting();
  }
});

function isPinnedRequest(request) {
  const url = new URL(request.url);
  return PINNED_PATH_SET.has(url.pathname);
}

async function cacheFirst(request) {
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(request, { ignoreSearch: true });
  if (cached) {
    return cached;
  }

  const response = await fetch(request);
  if (response.ok) {
    cache.put(request, response.clone());
  }
  return response;
}

async function networkFirstNavigation(request) {
  const cache = await caches.open(CACHE_NAME);
  try {
    const response = await fetch(request);
    if (response.ok) {
      cache.put(INDEX_PATH, response.clone());
    }
    return response;
  } catch (_error) {
    const fallback = await cache.match(INDEX_PATH, { ignoreSearch: true });
    if (fallback) {
      return fallback;
    }
    throw _error;
  }
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") {
    return;
  }

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) {
    return;
  }

  if (isPinnedRequest(request)) {
    event.respondWith(cacheFirst(request));
    return;
  }

  if (request.mode === "navigate") {
    event.respondWith(networkFirstNavigation(request));
  }
});
