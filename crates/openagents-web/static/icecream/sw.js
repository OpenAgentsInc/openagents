// The Ice Cream Man's service worker: the game installed to a home screen
// plays offline, and a new deploy still reaches players at once.
//
// - A build file under /games/the-ice-cream-man/v/VERSION/ never changes
//   (its version is in its address), so it comes from the cache first.
// - The page, the guide, the manifest and the icons come from the network
//   first, so the newest page (and with it the newest build's address)
//   always wins; the cache answers only when there's no network.
// - The page tells the worker the current build's files; the worker keeps
//   them and drops older builds.
const CACHE = "icecream-v1";
const GAME = "/games/the-ice-cream-man";
// The site's files the page uses besides the game's own.
const SHARED = ["/static/legacy-demo.css"];
const ours = (path) => path === GAME || path.startsWith(GAME + "/") || SHARED.includes(path);

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

self.addEventListener("message", (event) => {
  const urls = (event.data && Array.isArray(event.data.cache) ? event.data.cache : [])
    .map((url) => new URL(url, self.location.href))
    .filter((url) => url.origin === self.location.origin && ours(url.pathname));
  event.waitUntil(
    caches.open(CACHE).then(async (cache) => {
      for (const url of urls) {
        if (await cache.match(url.href)) continue;
        try {
          const response = await fetch(url.href, { cache: "no-cache" });
          if (response.ok) await cache.put(url.href, response);
        } catch (_) {
          // Offline now: the next visit fills it in.
        }
      }
      const keep = new Set(urls.map((url) => url.href));
      for (const request of await cache.keys()) {
        if (request.url.includes(GAME + "/v/") && !keep.has(request.url)) {
          await cache.delete(request);
        }
      }
    }),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin) return;
  if (!ours(url.pathname)) return;
  if (url.pathname === GAME + "/sw.js") return;
  if (url.pathname.startsWith(GAME + "/v/")) {
    event.respondWith(
      caches.match(request, { ignoreSearch: true }).then((hit) => hit || fetch(request)),
    );
    return;
  }
  event.respondWith(
    fetch(request)
      .then((response) => {
        if (response.ok && response.type === "basic") {
          const copy = response.clone();
          caches.open(CACHE).then((cache) => cache.put(request, copy));
        }
        return response;
      })
      .catch(() =>
        caches.match(request, { ignoreSearch: true }).then((hit) => hit || Response.error()),
      ),
  );
});
