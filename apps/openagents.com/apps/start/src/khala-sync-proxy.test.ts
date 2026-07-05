import { describe, expect, test } from 'vitest'

import {
  KHALA_SYNC_BOOTSTRAP_PROXY_PATH,
  KHALA_SYNC_CONNECT_PROXY_PATH,
  KHALA_SYNC_OWNER_COOKIE,
  KHALA_SYNC_PUSH_PROXY_PATH,
  KHALA_SYNC_SESSION_PATH,
  KHALA_SYNC_TOKEN_COOKIE,
  readKhalaSyncCredentials,
  routeKhalaSyncProxyRequestWithDeps,
  type KhalaSyncProxyDeps,
} from './khala-sync-proxy'

const ORIGIN = 'https://openagents-com-start-staging.workers.dev'

const cookieHeader = (ownerUserId: string, token: string): string =>
  `${KHALA_SYNC_OWNER_COOKIE}=${encodeURIComponent(ownerUserId)}; ${KHALA_SYNC_TOKEN_COOKIE}=${encodeURIComponent(token)}`

/**
 * `cookie` and `upgrade` are both spec "forbidden request-header names" —
 * `happy-dom`'s `Headers`/`Request` polyfill enforces that (matching real
 * browsers), so `new Request(url, { headers: { cookie: ... } })` silently
 * drops them, exactly like a real browser would for a `fetch()` call. A REAL
 * incoming WebSocket-upgrade or cookie-bearing request never goes through
 * that constructor path at all — the Workers runtime hands the route a
 * `Request` built from the actual TCP/TLS handshake, headers intact. This
 * minimal stand-in reproduces just the interface `khala-sync-proxy.ts`
 * reads (`url`, `method`, `headers.get`, `json`, `text`) so these tests can
 * still exercise both headers.
 */
const fakeRequest = (
  url: string,
  init: Readonly<{ method?: string; headers?: Record<string, string>; body?: string }> = {},
): Request => {
  const headerMap = new Map(Object.entries(init.headers ?? {}).map(([key, value]) => [key.toLowerCase(), value]))
  const bodyText = init.body
  return {
    url,
    method: init.method ?? 'GET',
    headers: { get: (name: string) => headerMap.get(name.toLowerCase()) ?? null },
    json: async () => (bodyText === undefined ? null : JSON.parse(bodyText)),
    text: async () => bodyText ?? '',
  } as unknown as Request
}

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })

type FakeCall = Readonly<{ url: string; init?: RequestInit }>

const fakeDeps = (
  handler: (call: FakeCall) => Response | Promise<Response>,
): KhalaSyncProxyDeps & { calls: Array<FakeCall> } => {
  const calls: Array<FakeCall> = []
  return {
    calls,
    fetch: async (input, init) => {
      const call: FakeCall = init === undefined ? { url: input } : { url: input, init }
      calls.push(call)
      return handler(call)
    },
    upstreamBaseUrl: 'https://openagents.com',
  }
}

describe('routeKhalaSyncProxyRequestWithDeps (#8413)', () => {
  test('falls through to undefined for unrelated paths', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(fakeRequest(`${ORIGIN}/khala/chat-sync`), deps)
    expect(response).toBeUndefined()
  })

  test('GET session reports signed_out with no cookies', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_SESSION_PATH}`),
      deps,
    )
    expect(response).toBeDefined()
    const body = await response!.json()
    expect(body).toEqual({ signedIn: false, ownerUserId: null })
  })

  test('GET session reports signed_in when both cookies are present', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_SESSION_PATH}`, {
        headers: { cookie: cookieHeader('user.owner1', 'tok-abc') },
      }),
      deps,
    )
    const body = await response!.json()
    expect(body).toEqual({ signedIn: true, ownerUserId: 'user.owner1' })
  })

  test('POST session rejects a blank owner/token pair without calling upstream', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_SESSION_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ ownerUserId: '', token: '' }),
      }),
      deps,
    )
    const body = await response!.json()
    expect(body).toEqual({
      ok: false,
      messageSafe: 'Owner user id and token are both required.',
    })
    expect(deps.calls).toHaveLength(0)
  })

  test('POST session validates against a real upstream bootstrap call and sets httpOnly cookies on success', async () => {
    const deps = fakeDeps(call => {
      expect(call.url).toBe('https://openagents.com/api/sync/bootstrap')
      expect((call.init?.headers as Record<string, string>).authorization).toBe('Bearer tok-abc')
      const parsedBody: unknown = JSON.parse(String(call.init?.body))
      expect(parsedBody).toMatchObject({ scope: 'scope.user.user.owner1' })
      return jsonResponse({ protocolVersion: 1, scope: 'scope.user.user.owner1', entities: [], cursor: 0 })
    })
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_SESSION_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ ownerUserId: 'user.owner1', token: 'tok-abc' }),
      }),
      deps,
    )
    expect(await response!.json()).toEqual({ ok: true })
    const setCookies = response!.headers.getSetCookie()
    expect(setCookies.some(cookie => cookie.startsWith(`${KHALA_SYNC_OWNER_COOKIE}=user.owner1`))).toBe(true)
    expect(setCookies.some(cookie => cookie.startsWith(`${KHALA_SYNC_TOKEN_COOKIE}=tok-abc`))).toBe(true)
    expect(setCookies.every(cookie => cookie.includes('HttpOnly'))).toBe(true)
  })

  test('POST session surfaces the upstream messageSafe on a failed validation without setting cookies', async () => {
    const deps = fakeDeps(() =>
      jsonResponse({ _tag: 'SyncError', code: 'unauthenticated', messageSafe: 'bad token', retryable: false }, 401),
    )
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_SESSION_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ ownerUserId: 'user.owner1', token: 'bad-token' }),
      }),
      deps,
    )
    expect(await response!.json()).toEqual({ ok: false, messageSafe: 'bad token' })
    expect(response!.headers.get('set-cookie')).toBeNull()
  })

  test('DELETE session clears both cookies', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_SESSION_PATH}`, { method: 'DELETE' }),
      deps,
    )
    expect(await response!.json()).toEqual({ ok: true })
    const setCookies = response!.headers.getSetCookie()
    expect(setCookies.some(cookie => cookie.startsWith(`${KHALA_SYNC_OWNER_COOKIE}=;`))).toBe(true)
    expect(setCookies.some(cookie => cookie.startsWith(`${KHALA_SYNC_TOKEN_COOKIE}=;`))).toBe(true)
  })

  test('bootstrap proxy rejects an unauthenticated request without calling upstream', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_BOOTSTRAP_PROXY_PATH}`, { method: 'POST', body: '{}' }),
      deps,
    )
    expect(response!.status).toBe(401)
    expect(deps.calls).toHaveLength(0)
  })

  test('bootstrap proxy forwards the body and attaches the cookie-sourced bearer token', async () => {
    const upstreamBody = { protocolVersion: 1, scope: 'scope.thread.t1', entities: [], cursor: 5 }
    const deps = fakeDeps(call => {
      expect((call.init?.headers as Record<string, string>).authorization).toBe('Bearer tok-abc')
      return jsonResponse(upstreamBody)
    })
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_BOOTSTRAP_PROXY_PATH}`, {
        method: 'POST',
        headers: { cookie: cookieHeader('user.owner1', 'tok-abc') },
        body: JSON.stringify({ scope: 'scope.thread.t1' }),
      }),
      deps,
    )
    expect(response!.status).toBe(200)
    expect(await response!.json()).toEqual(upstreamBody)
    expect(response!.headers.get('cache-control')).toBe('no-store')
  })

  test('push proxy mirrors an upstream mutation-rejected body verbatim', async () => {
    const rejectedBody = {
      protocolVersion: 1,
      results: [{ mutationId: 1, status: 'rejected', errorCode: 'thread_exists', errorMessageSafe: 'already exists' }],
      lastMutationId: 1,
    }
    const deps = fakeDeps(() => jsonResponse(rejectedBody))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_PUSH_PROXY_PATH}`, {
        method: 'POST',
        headers: { cookie: cookieHeader('user.owner1', 'tok-abc') },
        body: '{}',
      }),
      deps,
    )
    expect(await response!.json()).toEqual(rejectedBody)
  })

  test('connect proxy requires an Upgrade: websocket header', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_CONNECT_PROXY_PATH}?scope=scope.thread.t1`, {
        headers: { cookie: cookieHeader('user.owner1', 'tok-abc') },
      }),
      deps,
    )
    expect(response!.status).toBe(426)
  })

  test('connect proxy requires a signed-in session before attempting the upstream upgrade', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_CONNECT_PROXY_PATH}?scope=scope.thread.t1`, {
        headers: { upgrade: 'websocket' },
      }),
      deps,
    )
    expect(response!.status).toBe(401)
    expect(deps.calls).toHaveLength(0)
  })

  test('connect proxy requires a scope query parameter', async () => {
    const deps = fakeDeps(() => jsonResponse({}))
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_CONNECT_PROXY_PATH}`, {
        headers: { upgrade: 'websocket', cookie: cookieHeader('user.owner1', 'tok-abc') },
      }),
      deps,
    )
    expect(response!.status).toBe(400)
  })

  test('connect proxy attaches the bearer token to the upstream upgrade fetch and reports a clean 500 when WebSocketPair is unavailable in this test runtime', async () => {
    let closed = false
    const fakeUpstreamSocket = {
      accept: () => undefined,
      addEventListener: () => undefined,
      close: () => {
        closed = true
      },
    }
    const deps = fakeDeps(call => {
      expect(call.url).toBe('https://openagents.com/api/sync/connect?scope=scope.thread.t1&cursor=7')
      expect((call.init?.headers as Record<string, string>).authorization).toBe('Bearer tok-abc')
      expect((call.init?.headers as Record<string, string>).upgrade).toBe('websocket')
      const response = new Response(null, { status: 101 })
      Object.assign(response, { webSocket: fakeUpstreamSocket })
      return response
    })
    const response = await routeKhalaSyncProxyRequestWithDeps(
      fakeRequest(`${ORIGIN}${KHALA_SYNC_CONNECT_PROXY_PATH}?scope=scope.thread.t1&cursor=7`, {
        headers: { upgrade: 'websocket', cookie: cookieHeader('user.owner1', 'tok-abc') },
      }),
      deps,
    )
    // happy-dom/vitest has no Workers-runtime `WebSocketPair` global — this
    // confirms the proxy attached the token and reached that final pairing
    // step (rather than erroring earlier) and fails closed with a typed
    // 500 instead of hanging or throwing, closing the upstream socket it
    // opened. Real `WebSocketPair` bridging is exercised via a deployed
    // Worker + manual smoke, not this unit test — see
    // docs/khala-code/2026-07-04-mobile-tailnet-handshake.md.
    expect(response!.status).toBe(500)
    expect(closed).toBe(true)
  })
})

describe('readKhalaSyncCredentials', () => {
  test('returns undefined when either cookie is missing', () => {
    expect(readKhalaSyncCredentials(fakeRequest(ORIGIN))).toBeUndefined()
    expect(
      readKhalaSyncCredentials(
        fakeRequest(ORIGIN, { headers: { cookie: `${KHALA_SYNC_OWNER_COOKIE}=user.owner1` } }),
      ),
    ).toBeUndefined()
  })

  test('returns both fields when both cookies are present', () => {
    expect(
      readKhalaSyncCredentials(fakeRequest(ORIGIN, { headers: { cookie: cookieHeader('user.owner1', 'tok-abc') } })),
    ).toEqual({ ownerUserId: 'user.owner1', token: 'tok-abc' })
  })
})
