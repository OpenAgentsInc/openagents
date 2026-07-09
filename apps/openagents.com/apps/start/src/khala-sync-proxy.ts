/**
 * Server-side Khala Sync proxy for the Start web app (issue #8413).
 *
 * `apps/openagents.com/apps/start` deploys to its own isolated Worker
 * (`openagents-com-start-staging.workers.dev`), a DIFFERENT origin from the
 * production `openagents.com` Worker that owns the real
 * `/api/sync/bootstrap` `/api/sync/connect` `/api/sync/push` routes. Two
 * problems fall out of that:
 *
 * 1. Plain cross-origin `fetch` calls from the browser to those routes would
 *    need CORS headers the production Worker does not (and, per
 *    docs/khala-sync/SPEC.md, should not) grant to arbitrary web origins.
 * 2. The standard browser `WebSocket` constructor cannot set an
 *    `Authorization` header on the upgrade request (unlike React Native's
 *    3-arg constructor, see `clients/khala-mobile/src/sync/
 *    use-khala-sync-collection.ts`), so a bearer token can never reach
 *    `/api/sync/connect` directly from browser JS.
 *
 * This module solves both by keeping the bearer token OUT of the browser
 * entirely: it lives only in an httpOnly cookie set by `POST
 * /api/khala-sync/session` after a real bootstrap-backed credential check
 * (mirrors `clients/khala-mobile/src/auth/khala-auth-validate.ts`). Every
 * other route here is a same-origin server-to-server proxy: the browser
 * talks to THIS Worker's own origin (no CORS, cookies attach automatically),
 * and THIS Worker attaches the real `Authorization: Bearer <token>` header
 * when it forwards to the production Khala Sync API — including the
 * WebSocket upgrade, using the Workers-runtime "outbound fetch upgrade"
 * pattern already used for the Nostr relay bridge in
 * `apps/openagents.com/workers/api/src/forum-work-request-live-publisher.ts`
 * (`workersFetchRelayConnector`), just proxied back out to a browser client
 * via `WebSocketPair` instead of consumed in-process.
 */

import { getStartRequestContext } from '@openagentsinc/effect-start'
import { personalScope } from '@openagentsinc/khala-sync'

import {
  buildBootstrapRequestBody,
  KHALA_SYNC_WEB_BOOTSTRAP_PATH,
  KHALA_SYNC_WEB_CONNECT_PATH,
  KHALA_SYNC_WEB_PUSH_PATH,
  KHALA_SYNC_WEB_SESSION_PATH,
  type BootstrapRequestBody,
} from './routes/-chat-sync-web-core'
import {
  expiredCookie,
  parseCookies,
  serializeCookie,
} from '../../../workers/api/src/auth-cookies'

export const KHALA_SYNC_SESSION_PATH = KHALA_SYNC_WEB_SESSION_PATH
export const KHALA_SYNC_BOOTSTRAP_PROXY_PATH = KHALA_SYNC_WEB_BOOTSTRAP_PATH
export const KHALA_SYNC_PUSH_PROXY_PATH = KHALA_SYNC_WEB_PUSH_PATH
export const KHALA_SYNC_CONNECT_PROXY_PATH = KHALA_SYNC_WEB_CONNECT_PATH

export const KHALA_SYNC_OWNER_COOKIE = 'khala_sync_owner'
export const KHALA_SYNC_TOKEN_COOKIE = 'khala_sync_token'

const SESSION_COOKIE_MAX_AGE_SECONDS = 60 * 60 * 24 * 30

export const DEFAULT_KHALA_SYNC_UPSTREAM_BASE_URL = 'https://openagents.com'

export type KhalaSyncProxyFetch = (
  input: string,
  init?: RequestInit,
) => Promise<Response>

export type KhalaSyncProxyDeps = Readonly<{
  fetch: KhalaSyncProxyFetch
  upstreamBaseUrl: string
}>

const upstreamUrl = (deps: KhalaSyncProxyDeps, path: string): string =>
  `${deps.upstreamBaseUrl.replace(/\/$/, '')}${path}`

const noStoreJson = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })

export type KhalaSyncCredentials = Readonly<{
  ownerUserId: string
  token: string
}>

export const readKhalaSyncCredentials = (
  request: Request,
): KhalaSyncCredentials | undefined => {
  const cookies = parseCookies(request)
  const ownerUserId = cookies.get(KHALA_SYNC_OWNER_COOKIE)
  const token = cookies.get(KHALA_SYNC_TOKEN_COOKIE)

  if (
    ownerUserId === undefined ||
    ownerUserId === '' ||
    token === undefined ||
    token === ''
  ) {
    return undefined
  }

  return { ownerUserId, token }
}

type SignInValidation = Readonly<{ ok: true }> | Readonly<{ ok: false; messageSafe: string }>

/**
 * Confirms a token/ownerUserId pair actually authenticates against Khala
 * Sync before saving it — a real `POST /api/sync/bootstrap` call against the
 * owner's own personal scope, mirroring
 * `clients/khala-mobile/src/auth/khala-auth-validate.ts`.
 */
export const validateKhalaSyncCredentials = async (
  deps: KhalaSyncProxyDeps,
  ownerUserId: string,
  token: string,
): Promise<SignInValidation> => {
  try {
    const body: BootstrapRequestBody = buildBootstrapRequestBody(
      String(personalScope(ownerUserId)),
      'khala-web-sign-in-check',
    )
    const response = await deps.fetch(upstreamUrl(deps, '/api/sync/bootstrap'), {
      method: 'POST',
      headers: {
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
      },
      body: JSON.stringify(body),
    })
    if (response.ok) return { ok: true }
    const parsed: unknown = await response.json().catch(() => null)
    const messageSafe =
      typeof parsed === 'object' && parsed !== null && 'messageSafe' in parsed
        ? String((parsed as { messageSafe: unknown }).messageSafe)
        : `sign-in check failed (${response.status})`
    return { ok: false, messageSafe }
  } catch (error) {
    return {
      ok: false,
      messageSafe: error instanceof Error ? error.message : 'sign-in check failed',
    }
  }
}

const handleSessionStatus = (request: Request): Response => {
  const credentials = readKhalaSyncCredentials(request)
  return noStoreJson({
    signedIn: credentials !== undefined,
    ownerUserId: credentials?.ownerUserId ?? null,
  })
}

const readJsonBody = async (request: Request): Promise<Record<string, unknown> | undefined> => {
  try {
    const parsed: unknown = await request.json()
    return typeof parsed === 'object' && parsed !== null
      ? (parsed as Record<string, unknown>)
      : undefined
  } catch {
    return undefined
  }
}

const stringField = (body: Record<string, unknown> | undefined, key: string): string => {
  const value = body?.[key]
  return typeof value === 'string' ? value.trim() : ''
}

const handleSessionSignIn = async (
  request: Request,
  deps: KhalaSyncProxyDeps,
): Promise<Response> => {
  const body = await readJsonBody(request)
  const ownerUserId = stringField(body, 'ownerUserId')
  const token = stringField(body, 'token')

  if (ownerUserId === '' || token === '') {
    return noStoreJson({
      ok: false,
      messageSafe: 'Owner user id and token are both required.',
    })
  }

  const validation = await validateKhalaSyncCredentials(deps, ownerUserId, token)
  if (!validation.ok) {
    return noStoreJson({ ok: false, messageSafe: validation.messageSafe })
  }

  // Cookies are appended to the RESPONSE's own `headers` object after
  // construction, not built on a separate `Headers` instance passed into
  // `ResponseInit` — some Fetch-spec implementations (observed in this
  // app's `happy-dom` unit-test environment) don't reliably preserve
  // multiple same-name header values when copying an externally-built
  // `Headers` object into a new `Response`.
  const response = noStoreJson({ ok: true })
  response.headers.append(
    'set-cookie',
    serializeCookie(KHALA_SYNC_OWNER_COOKIE, ownerUserId, SESSION_COOKIE_MAX_AGE_SECONDS),
  )
  response.headers.append(
    'set-cookie',
    serializeCookie(KHALA_SYNC_TOKEN_COOKIE, token, SESSION_COOKIE_MAX_AGE_SECONDS),
  )
  return response
}

const handleSessionSignOut = (): Response => {
  const response = noStoreJson({ ok: true })
  response.headers.append('set-cookie', expiredCookie(KHALA_SYNC_OWNER_COOKIE))
  response.headers.append('set-cookie', expiredCookie(KHALA_SYNC_TOKEN_COOKIE))
  return response
}

const unauthenticated = (): Response =>
  noStoreJson(
    {
      code: 'unauthenticated',
      messageSafe: 'Sign in to Khala Sync before bootstrapping or pushing mutations.',
    },
    401,
  )

/** Forwards a POST body verbatim to the named upstream Khala Sync route with
 * the caller's bearer token attached, and mirrors back the upstream status +
 * body byte-for-byte (typed `SyncError` bodies included). */
const proxyJsonPost = async (
  request: Request,
  deps: KhalaSyncProxyDeps,
  upstreamPath: string,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return noStoreJson({ code: 'invalid_request', messageSafe: 'POST required' }, 405)
  }
  const credentials = readKhalaSyncCredentials(request)
  if (credentials === undefined) return unauthenticated()

  const bodyText = await request.text()
  const upstream = await deps.fetch(upstreamUrl(deps, upstreamPath), {
    method: 'POST',
    headers: {
      authorization: `Bearer ${credentials.token}`,
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

/** Cloudflare Workers extends `fetch`'s `Response` with an optional
 * `webSocket` field on a successful upgrade, and `ResponseInit` with the
 * matching constructor argument — neither is in `lib.dom.d.ts`, so both ends
 * are narrowly typed here rather than widening the ambient DOM types (same
 * convention as `forum-work-request-live-publisher.ts` and
 * `khala-sync-hub-do.ts`). */
type WorkersUpgradeResponse = Response & { webSocket?: WebSocket | null }
type WorkersUpgradeResponseInit = ResponseInit & { webSocket?: WebSocket }

const parseNonNegativeInt = (raw: string | null): number => {
  if (raw === null || !/^\d+$/.test(raw)) return 0
  const value = Number(raw)
  return Number.isSafeInteger(value) ? value : 0
}

/**
 * Proxies the WebSocket upgrade itself: fetches the upstream connect route
 * WITH the bearer token attached (server-to-server, so the header restriction
 * that blocks browser `WebSocket` does not apply), accepts the resulting
 * upstream socket, then bridges it to a fresh `WebSocketPair` handed back to
 * the browser — the browser only ever sees THIS Worker's own origin.
 */
const proxyConnectUpgrade = async (
  request: Request,
  deps: KhalaSyncProxyDeps,
): Promise<Response> => {
  if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') {
    return noStoreJson(
      {
        code: 'invalid_request',
        messageSafe: `GET ${KHALA_SYNC_CONNECT_PROXY_PATH} requires a WebSocket upgrade.`,
      },
      426,
    )
  }
  const credentials = readKhalaSyncCredentials(request)
  if (credentials === undefined) return unauthenticated()

  const requestUrl = new URL(request.url)
  const scope = requestUrl.searchParams.get('scope')
  if (scope === null || scope === '') {
    return noStoreJson(
      { code: 'invalid_request', messageSafe: 'scope query parameter is required.' },
      400,
    )
  }
  const cursor = parseNonNegativeInt(requestUrl.searchParams.get('cursor'))

  const target = new URL(upstreamUrl(deps, '/api/sync/connect'))
  target.searchParams.set('scope', scope)
  target.searchParams.set('cursor', String(cursor))

  let upstreamResponse: WorkersUpgradeResponse
  try {
    upstreamResponse = (await deps.fetch(target.toString(), {
      headers: {
        authorization: `Bearer ${credentials.token}`,
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
    // Upstream declined the upgrade (typed SyncError body) — mirror it back.
    const text = await upstreamResponse.text().catch(() => '')
    return new Response(text || JSON.stringify({
      code: 'internal',
      messageSafe: 'Khala Sync connect upstream refused the upgrade.',
    }), {
      status: upstreamResponse.status === 101 ? 502 : upstreamResponse.status,
      headers: {
        'content-type': 'application/json; charset=utf-8',
        'cache-control': 'no-store',
      },
    })
  }

  ;(upstreamSocket as unknown as { accept: () => void }).accept()

  const PairCtor = (globalThis as { WebSocketPair?: new () => [WebSocket, WebSocket] })
    .WebSocketPair
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
  ;(server as unknown as { accept: () => void }).accept()

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
 * Routes one `/api/khala-sync/*` request, or returns `undefined` for
 * anything else so the caller can fall through to the normal Start router.
 * Pure w.r.t. its `deps` — tests inject a fake `fetch` so no real network or
 * Workers runtime is required.
 */
export const routeKhalaSyncProxyRequestWithDeps = async (
  request: Request,
  deps: KhalaSyncProxyDeps,
): Promise<Response | undefined> => {
  const path = new URL(request.url).pathname

  if (path === KHALA_SYNC_SESSION_PATH) {
    if (request.method === 'GET') return handleSessionStatus(request)
    if (request.method === 'POST') return handleSessionSignIn(request, deps)
    if (request.method === 'DELETE') return handleSessionSignOut()
    return noStoreJson({ code: 'invalid_request', messageSafe: 'unsupported method' }, 405)
  }

  if (path === KHALA_SYNC_BOOTSTRAP_PROXY_PATH) {
    return proxyJsonPost(request, deps, '/api/sync/bootstrap')
  }

  if (path === KHALA_SYNC_PUSH_PROXY_PATH) {
    return proxyJsonPost(request, deps, '/api/sync/push')
  }

  if (path === KHALA_SYNC_CONNECT_PROXY_PATH) {
    return proxyConnectUpgrade(request, deps)
  }

  return undefined
}

type StartEnvLike = Readonly<{ KHALA_SYNC_UPSTREAM_BASE_URL?: unknown }>

/** Reads an optional upstream-base-url override from the Start Worker's own
 * `env` (Wrangler var / secret) — defaults to production `openagents.com` so
 * a normal deploy needs zero configuration. Read via `effect-start`'s
 * AsyncLocalStorage request context (populated by `withStartRequestContext`
 * in `server.ts` before this ever runs) rather than threaded as a parameter,
 * so this stays a drop-in `(request) => Promise<Response | undefined>`
 * alongside `routeSharedAgentSurface` at the call site in `server.ts`. */
const resolveUpstreamBaseUrl = (): string => {
  const context = getStartRequestContext<StartEnvLike>()
  const fromEnv = context?.env?.KHALA_SYNC_UPSTREAM_BASE_URL
  return typeof fromEnv === 'string' && fromEnv.trim() !== ''
    ? fromEnv.trim()
    : DEFAULT_KHALA_SYNC_UPSTREAM_BASE_URL
}

/** Real-`fetch`-backed entry point for `server.ts`. */
export const routeKhalaSyncProxyRequest = (
  request: Request,
): Promise<Response | undefined> =>
  routeKhalaSyncProxyRequestWithDeps(request, {
    fetch: globalThis.fetch.bind(globalThis),
    upstreamBaseUrl: resolveUpstreamBaseUrl(),
  })
