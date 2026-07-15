// Khala Sync LiveHub HTTP client (CFG-5, #8520; epic #8515).
//
// The owned Google Cloud Run `khala-live-hub` service (apps/khala-live-hub)
// owns the per-scope live hub on Google Cloud.
// This module adapts that HTTP/WS service to the EXISTING
// `KhalaSyncHubNamespaceLike` seam every hub consumer already uses (the
// /api/sync/connect WS proxy, the /api/sync/log hub-first read, the four
// internal hub routes, and the access-changed notifier), so the cutover is
// a CONFIG CHANGE, not a route rewrite:
//
//   env.KHALA_SYNC_LIVE_HUB_URL    — LiveHub base URL (e.g. the Cloud Run
//                                    service URL). Set ⇒ hub traffic goes
//                                    to LiveHub over HTTPS.
//   env.KHALA_SYNC_LIVE_HUB_TOKEN  — shared service bearer sourced from
//                                    Secret Manager `khala-live-hub-token`.
//
// `resolveKhalaSyncHubNamespace(env)` returns LiveHub when both values are set;
// missing config degrades to the routes' honest 503/absent-hub paths.
//
// HOW THE WS PROXY STILL WORKS: consumers forward
// `new Request(internalUrl, request)` into `stub.fetch`. This adapter
// rewrites the internal `khala-sync-hub.openagents.internal` URL onto the
// LiveHub base (path + query preserved), REPLACES the Authorization header
// with the shared service bearer (the client's promoted `?token=` bearer
// from `withBearerFromQueryToken` — commit b45071b9b6 — is for the ROUTE's
// own end-user auth, which has already run; it must never travel to
// LiveHub), and fetches. The Cloud Run service carries the WebSocket upgrade
// over its HTTP transport. This module is runtime-agnostic (fetch + Headers
// only) and is used directly by the Google Cloud monolith.

import type {
  KhalaSyncHubNamespaceLike,
  KhalaSyncHubStubLike,
} from './khala-sync-hub-routes'

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
  KHALA_SYNC_LIVE_HUB_FETCH?: ((request: Request) => Promise<HttpResponse>) | undefined
  KHALA_SYNC_LIVE_HUB_URL?: string | undefined
  KHALA_SYNC_LIVE_HUB_TOKEN?: string | undefined
}>

/**
 * Resolve the hub namespace for a deployment: the LiveHub HTTP adapter when
 * `KHALA_SYNC_LIVE_HUB_URL` + `KHALA_SYNC_LIVE_HUB_TOKEN` are both set.
 */
export const resolveKhalaSyncHubNamespace = (
  env: KhalaSyncLiveHubEnvSlice,
): KhalaSyncHubNamespaceLike | undefined => {
  const url = env.KHALA_SYNC_LIVE_HUB_URL?.trim()
  const token = env.KHALA_SYNC_LIVE_HUB_TOKEN?.trim()
  if (url !== undefined && url !== '' && token !== undefined && token !== '') {
    return makeKhalaSyncLiveHubNamespace({
      baseUrl: url,
      token,
      ...(env.KHALA_SYNC_LIVE_HUB_FETCH === undefined
        ? {}
        : { fetchImpl: env.KHALA_SYNC_LIVE_HUB_FETCH }),
    })
  }
  return undefined
}
