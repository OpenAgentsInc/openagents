// Khala Sync LiveHub HTTP client (CFG-5, #8520; epic #8515).
//
// The owned Google Cloud Run `khala-live-hub` service (apps/khala-live-hub)
// replaces the `KhalaSyncHubDO` Durable Object as the per-scope live hub.
// This module adapts that HTTP/WS service to the EXISTING
// `KhalaSyncHubNamespaceLike` seam every hub consumer already uses (the
// /api/sync/connect WS proxy, the /api/sync/log hub-first read, the four
// internal hub routes, and the access-changed notifier), so the cutover is
// a CONFIG CHANGE, not a route rewrite:
//
//   env.KHALA_SYNC_LIVE_HUB_URL    — LiveHub base URL (e.g. the Cloud Run
//                                    service URL). Set ⇒ hub traffic goes
//                                    to LiveHub over HTTPS.
//   env.KHALA_SYNC_LIVE_HUB_TOKEN  — shared service bearer (Worker secret /
//                                    Secret Manager `khala-live-hub-token`).
//
// `resolveKhalaSyncHubNamespace(env)` prefers LiveHub when BOTH are set and
// falls back to the DO binding otherwise, so a deployment missing either
// half keeps its previous behavior (and after the DO deletion, missing
// config degrades to the routes' existing honest 503/absent-hub paths).
//
// HOW THE WS PROXY STILL WORKS: consumers forward
// `new Request(internalUrl, request)` into `stub.fetch`. This adapter
// rewrites the internal `khala-sync-hub.openagents.internal` URL onto the
// LiveHub base (path + query preserved), REPLACES the Authorization header
// with the shared service bearer (the client's promoted `?token=` bearer
// from `withBearerFromQueryToken` — commit b45071b9b6 — is for the ROUTE's
// own end-user auth, which has already run; it must never travel to
// LiveHub), and fetches. workerd supports WebSocket upgrades through plain
// `fetch` to external origins: the 101 response carries `webSocket` back
// through the route exactly like a DO stub's upgrade response did. This
// module is runtime-agnostic (fetch + Headers only), so the same seam
// serves the CFG-9 Cloud Run monolith unchanged.

import type {
  KhalaSyncHubNamespaceLike,
  KhalaSyncHubStubLike,
} from './khala-sync-hub-do'

type HttpResponse = globalThis.Response

export type KhalaSyncLiveHubConfig = Readonly<{
  /** LiveHub base URL, e.g. https://khala-live-hub-….run.app */
  baseUrl: string
  /** Shared service bearer (KHALA_LIVE_HUB_TOKEN on the service side). */
  token: string
  /** Injectable for tests; defaults to global fetch. */
  fetchImpl?: ((request: Request) => Promise<HttpResponse>) | undefined
}>

/**
 * Adapt the LiveHub HTTP service to the `KhalaSyncHubNamespaceLike` seam.
 * `idFromName(scope)` is the scope itself — LiveHub owns the scope→hub map
 * (and the documented sharding extension point) service-side.
 */
export const makeKhalaSyncLiveHubNamespace = (
  config: KhalaSyncLiveHubConfig,
): KhalaSyncHubNamespaceLike => {
  const base = config.baseUrl.replace(/\/+$/, '')
  const fetchImpl =
    config.fetchImpl ?? ((request: Request) => fetch(request))

  const stub: KhalaSyncHubStubLike = {
    fetch: (request: Request): Promise<HttpResponse> => {
      const inbound = new URL(request.url)
      const target = new URL(`${base}${inbound.pathname}`)
      // Preserve the consumer-set query (scope/cursor/limit). The internal
      // hub URL never carries end-user tokens (the connect route re-encodes
      // only scope + cursor), so nothing sensitive is forwarded here.
      inbound.searchParams.forEach((value, key) => {
        target.searchParams.set(key, value)
      })
      // `new Request(url, request)` preserves method, body, and the
      // WebSocket Upgrade header for /connect proxying.
      const outbound = new Request(target.toString(), request)
      // The service bearer ALWAYS wins: any inbound Authorization (the
      // route-promoted end-user bearer) authenticated the ROUTE, not the
      // hub hop.
      outbound.headers.set('authorization', `Bearer ${config.token}`)
      return fetchImpl(outbound)
    },
  }

  return {
    idFromName: (name: string) => name,
    get: () => stub,
  }
}

/**
 * The minimal env slice this resolver reads. Structural so route wiring and
 * tests can pass small fakes.
 */
export type KhalaSyncLiveHubEnvSlice = Readonly<{
  KHALA_SYNC_LIVE_HUB_URL?: string | undefined
  KHALA_SYNC_LIVE_HUB_TOKEN?: string | undefined
  KHALA_SYNC_HUB?: unknown
}>

/**
 * Resolve the hub namespace for a deployment: the LiveHub HTTP adapter when
 * `KHALA_SYNC_LIVE_HUB_URL` + `KHALA_SYNC_LIVE_HUB_TOKEN` are BOTH set,
 * else the `KHALA_SYNC_HUB` DO binding (absent ⇒ undefined, which every
 * consumer already maps to its honest hub-unconfigured behavior).
 */
export const resolveKhalaSyncHubNamespace = (
  env: KhalaSyncLiveHubEnvSlice,
): KhalaSyncHubNamespaceLike | undefined => {
  const url = env.KHALA_SYNC_LIVE_HUB_URL?.trim()
  const token = env.KHALA_SYNC_LIVE_HUB_TOKEN?.trim()
  if (url !== undefined && url !== '' && token !== undefined && token !== '') {
    return makeKhalaSyncLiveHubNamespace({ baseUrl: url, token })
  }
  return env.KHALA_SYNC_HUB as KhalaSyncHubNamespaceLike | undefined
}
