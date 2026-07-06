/**
 * Server-side Khala Sync proxy for Aiur (AIUR-1, #8499).
 *
 * Aiur is a separate Cloudflare Worker (`aiur.openagents.com`) from the
 * production `openagents.com` Worker that owns the real
 * `/api/sync/bootstrap` / `/api/sync/push` / `/api/sync/connect` routes.
 * Same two problems as `apps/openagents.com/apps/start/src/khala-sync-proxy.ts`
 * (cross-origin fetch needs CORS the upstream does not grant; a browser
 * `WebSocket` cannot set an `Authorization` header) — same fix: the browser
 * only ever talks to THIS Worker's own origin, and THIS Worker attaches the
 * real bearer server-side before forwarding upstream.
 *
 * Unlike the Start app's proxy, the bearer here is never a separately
 * pasted/typed token — it is the SAME OpenAuth access token that already
 * authenticated the caller as the allow-listed Aiur owner (`resolveAiurAccess`),
 * exactly mirroring the mobile bridge's `syncToken = the OpenAuth access
 * token` pattern (#8469). Every route below re-checks the owner gate itself
 * (FAIL CLOSED) rather than trusting an caller-supplied header/cookie.
 */

import { resolveAiurAccess, type ResolveAiurAccessDeps } from './auth/access'
import { AIUR_ACCESS_COOKIE, parseCookies } from './auth/cookies'
import type { AiurEnv } from './auth/config'

export const AIUR_SYNC_BOOTSTRAP_PATH = '/api/sync/bootstrap'
export const AIUR_SYNC_PUSH_PATH = '/api/sync/push'
export const AIUR_SYNC_LOG_PATH = '/api/sync/log'
export const AIUR_SYNC_CONNECT_PATH = '/api/sync/connect'

const noStoreJson = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })

const unauthenticated = (): Response =>
  noStoreJson(
    {
      code: 'unauthenticated',
      messageSafe: 'Sign in as the Aiur owner before using Khala Sync.',
    },
    401,
  )

export type KhalaSyncProxyFetch = (
  input: string,
  init?: RequestInit,
) => Promise<Response>

export type AiurKhalaSyncProxyDeps = ResolveAiurAccessDeps &
  Readonly<{
    fetch?: KhalaSyncProxyFetch
    upstreamBaseUrl?: string
  }>

const upstreamUrl = (
  env: AiurEnv,
  deps: AiurKhalaSyncProxyDeps,
  path: string,
): string => {
  const base = (
    deps.upstreamBaseUrl ??
    env.KHALA_SYNC_UPSTREAM_BASE_URL ??
    'https://openagents.com'
  ).replace(/\/$/, '')

  return `${base}${path}`
}

/** Resolves owner access AND the raw bearer to forward, or `undefined`. */
const requireOwnerBearer = async (
  request: Request,
  env: AiurEnv,
  deps: AiurKhalaSyncProxyDeps,
): Promise<string | undefined> => {
  const access = await resolveAiurAccess(request, env, deps)

  if (access.kind !== 'owner') {
    return undefined
  }

  return parseCookies(request).get(AIUR_ACCESS_COOKIE)
}

const proxyJsonPost = async (
  request: Request,
  env: AiurEnv,
  deps: AiurKhalaSyncProxyDeps,
  upstreamPath: string,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return noStoreJson(
      { code: 'invalid_request', messageSafe: 'POST required' },
      405,
    )
  }

  const bearer = await requireOwnerBearer(request, env, deps)
  if (bearer === undefined) return unauthenticated()

  const fetchImpl = deps.fetch ?? globalThis.fetch.bind(globalThis)
  const bodyText = await request.text()
  const upstream = await fetchImpl(upstreamUrl(env, deps, upstreamPath), {
    method: 'POST',
    headers: {
      authorization: `Bearer ${bearer}`,
      'content-type': 'application/json',
    },
    body: bodyText,
  })
  const responseText = await upstream.text()

  return new Response(responseText, {
    status: upstream.status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })
}

const proxyLogGet = async (
  request: Request,
  env: AiurEnv,
  deps: AiurKhalaSyncProxyDeps,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return noStoreJson(
      { code: 'invalid_request', messageSafe: 'GET required' },
      405,
    )
  }

  const bearer = await requireOwnerBearer(request, env, deps)
  if (bearer === undefined) return unauthenticated()

  const fetchImpl = deps.fetch ?? globalThis.fetch.bind(globalThis)
  const requestUrl = new URL(request.url)
  const target = new URL(upstreamUrl(env, deps, AIUR_SYNC_LOG_PATH))
  target.search = requestUrl.search
  const upstream = await fetchImpl(target.toString(), {
    method: 'GET',
    headers: { authorization: `Bearer ${bearer}` },
  })
  const responseText = await upstream.text()

  return new Response(responseText, {
    status: upstream.status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })
}

/** Cloudflare Workers extends `fetch`'s `Response`/`ResponseInit` with an
 * optional `webSocket` field on a successful upgrade — not in
 * `lib.dom.d.ts`, so both ends are narrowly typed here (same convention as
 * `apps/openagents.com/apps/start/src/khala-sync-proxy.ts`). */
type WorkersUpgradeResponse = Response & { webSocket?: WebSocket | null }
type WorkersUpgradeResponseInit = ResponseInit & { webSocket?: WebSocket }

export type AiurSyncConnectTarget =
  | Readonly<{ kind: 'response'; response: Response }>
  | Readonly<{ kind: 'connect'; bearer: string; targetUrl: string }>

/**
 * The runtime-agnostic gate + target resolution for `/api/sync/connect`,
 * shared by the Workers `WebSocketPair` bridge below and the Cloud Run Bun
 * bridge (`cloudrun/server.ts`). FAIL CLOSED: anything other than a
 * verified allow-listed owner session yields an error `response`, never a
 * `connect`.
 */
export const resolveAiurSyncConnectTarget = async (
  request: Request,
  env: AiurEnv,
  deps: AiurKhalaSyncProxyDeps = {},
): Promise<AiurSyncConnectTarget> => {
  if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') {
    return {
      kind: 'response',
      response: noStoreJson(
        {
          code: 'invalid_request',
          messageSafe: `GET ${AIUR_SYNC_CONNECT_PATH} requires a WebSocket upgrade.`,
        },
        426,
      ),
    }
  }

  const bearer = await requireOwnerBearer(request, env, deps)
  if (bearer === undefined) {
    return { kind: 'response', response: unauthenticated() }
  }

  const requestUrl = new URL(request.url)
  const scope = requestUrl.searchParams.get('scope')
  if (scope === null || scope === '') {
    return {
      kind: 'response',
      response: noStoreJson(
        { code: 'invalid_request', messageSafe: 'scope query parameter is required.' },
        400,
      ),
    }
  }
  const cursor = requestUrl.searchParams.get('cursor') ?? '0'

  const target = new URL(upstreamUrl(env, deps, AIUR_SYNC_CONNECT_PATH))
  target.searchParams.set('scope', scope)
  target.searchParams.set('cursor', cursor)

  return { kind: 'connect', bearer, targetUrl: target.toString() }
}

const proxyConnectUpgrade = async (
  request: Request,
  env: AiurEnv,
  deps: AiurKhalaSyncProxyDeps,
): Promise<Response> => {
  const connectTarget = await resolveAiurSyncConnectTarget(request, env, deps)
  if (connectTarget.kind === 'response') {
    return connectTarget.response
  }

  const { bearer, targetUrl } = connectTarget

  const fetchImpl = deps.fetch ?? globalThis.fetch.bind(globalThis)
  let upstreamResponse: WorkersUpgradeResponse
  try {
    upstreamResponse = (await fetchImpl(targetUrl, {
      headers: {
        authorization: `Bearer ${bearer}`,
        upgrade: 'websocket',
        connection: 'Upgrade',
      },
    })) as WorkersUpgradeResponse
  } catch {
    return noStoreJson(
      {
        code: 'internal',
        messageSafe: 'Khala Sync live-tail upgrade failed unexpectedly; reconnect.',
      },
      500,
    )
  }

  const upstreamSocket = upstreamResponse.webSocket
  if (upstreamSocket === undefined || upstreamSocket === null) {
    const text = await upstreamResponse.text().catch(() => '')
    return new Response(
      text ||
        JSON.stringify({
          code: 'internal',
          messageSafe: 'Khala Sync connect upstream refused the upgrade.',
        }),
      {
        status: upstreamResponse.status === 101 ? 502 : upstreamResponse.status,
        headers: {
          'content-type': 'application/json; charset=utf-8',
          'cache-control': 'no-store',
        },
      },
    )
  }

  ;(upstreamSocket as unknown as { accept: () => void }).accept()

  const PairCtor = (
    globalThis as { WebSocketPair?: new () => [WebSocket, WebSocket] }
  ).WebSocketPair
  if (PairCtor === undefined) {
    try {
      upstreamSocket.close()
    } catch {
      // already closed
    }
    return noStoreJson(
      { code: 'internal', messageSafe: 'WebSocketPair is unavailable in this runtime.' },
      500,
    )
  }

  const [client, server] = new PairCtor()
  server.accept()

  server.addEventListener('message', event => {
    try {
      upstreamSocket.send(event.data as never)
    } catch {
      // upstream already closed
    }
  })
  server.addEventListener('close', event => {
    try {
      upstreamSocket.close(event.code, event.reason)
    } catch {
      // already closed
    }
  })
  upstreamSocket.addEventListener('message', event => {
    try {
      server.send((event as MessageEvent).data)
    } catch {
      // client already closed
    }
  })
  upstreamSocket.addEventListener('close', () => {
    try {
      server.close()
    } catch {
      // already closed
    }
  })
  upstreamSocket.addEventListener('error', () => {
    try {
      server.close(1011, 'upstream error')
    } catch {
      // already closed
    }
  })

  return new Response(null, {
    status: 101,
    webSocket: client,
  } as WorkersUpgradeResponseInit) as Response
}

/**
 * Routes one `/api/sync/*` request, or returns `undefined` for anything
 * else so the caller falls through to the normal app router. Pure w.r.t.
 * its `deps` — tests inject a fake `resolveAiurAccess`-backing client and
 * fake `fetch`, no real network or Workers runtime required.
 */
export const routeAiurKhalaSyncProxyRequest = (
  request: Request,
  env: AiurEnv,
  deps: AiurKhalaSyncProxyDeps = {},
): Promise<Response> | undefined => {
  const path = new URL(request.url).pathname

  if (path === AIUR_SYNC_BOOTSTRAP_PATH) {
    return proxyJsonPost(request, env, deps, AIUR_SYNC_BOOTSTRAP_PATH)
  }

  if (path === AIUR_SYNC_PUSH_PATH) {
    return proxyJsonPost(request, env, deps, AIUR_SYNC_PUSH_PATH)
  }

  if (path === AIUR_SYNC_LOG_PATH) {
    return proxyLogGet(request, env, deps)
  }

  if (path === AIUR_SYNC_CONNECT_PATH) {
    return proxyConnectUpgrade(request, env, deps)
  }

  return undefined
}
