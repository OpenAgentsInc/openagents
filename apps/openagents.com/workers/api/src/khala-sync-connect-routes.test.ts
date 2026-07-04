// Route tests for GET /api/sync/connect (KS-4.4, #8297): auth BEFORE the
// upgrade, param validation, v1 scope gate, 426 for non-WebSocket requests,
// 503 while the hub binding is absent, and the DO WebSocket-proxy forward
// (per-scope idFromName, /connect target with scope + cursor params, Upgrade
// header preserved, hub response passed through untouched). All seams are
// injected — no network, no Durable Objects.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { personalScope, publicScope } from '@openagentsinc/khala-sync'

import type {
  KhalaSyncHubNamespaceLike,
  KhalaSyncHubStubLike,
} from './khala-sync-hub-do'
import {
  handleKhalaSyncConnect,
  KHALA_SYNC_CONNECT_PATH,
  type KhalaSyncConnectDependencies,
} from './khala-sync-connect-routes'

const USER_ID = 'user-1'
const OWN_SCOPE = personalScope(USER_ID)
const PUBLIC_SCOPE = publicScope('artanis.global')

const get = (
  params: Readonly<Record<string, string>> = { scope: OWN_SCOPE },
  headers: Readonly<Record<string, string>> = { upgrade: 'websocket' },
) => {
  const url = new URL(`https://openagents.com${KHALA_SYNC_CONNECT_PATH}`)
  for (const [key, value] of Object.entries(params)) {
    url.searchParams.set(key, value)
  }
  return new Request(url.toString(), { headers, method: 'GET' })
}

/** A hub namespace whose per-scope stub answers with `respond`. */
const fakeHub = (
  respond: (request: Request) => Response | Promise<Response>,
) => {
  const requests: Array<Request> = []
  const idsRequested: Array<string> = []
  const stub: KhalaSyncHubStubLike = {
    fetch: async request => {
      requests.push(request)
      return respond(request)
    },
  }
  const namespace: KhalaSyncHubNamespaceLike = {
    get: () => stub,
    idFromName: name => {
      idsRequested.push(name)
      return name
    },
  }
  return { idsRequested, namespace, requests }
}

const run = (
  input: Readonly<{
    request?: Request
    userId?: string | undefined
    hubNamespace?: KhalaSyncHubNamespaceLike | undefined
  }> = {},
) => {
  const deps: KhalaSyncConnectDependencies = {
    authenticate: async () =>
      'userId' in input
        ? input.userId === undefined
          ? undefined
          : { userId: input.userId }
        : { userId: USER_ID },
    hubNamespace: 'hubNamespace' in input ? input.hubNamespace : undefined,
  }
  return Effect.runPromise(handleKhalaSyncConnect(input.request ?? get(), deps))
}

const syncErrorBody = async (response: Response) =>
  (await response.json()) as {
    code: string
    messageSafe: string
    retryable: boolean
  }

describe('handleKhalaSyncConnect', () => {
  test('non-GET methods are 405', async () => {
    const response = await run({
      request: new Request(`https://openagents.com${KHALA_SYNC_CONNECT_PATH}`, {
        method: 'POST',
      }),
    })
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })

  test('unauthenticated requests are 401 BEFORE any hub contact', async () => {
    const hub = fakeHub(() => {
      throw new Error('hub must not be reached unauthenticated')
    })
    const response = await run({
      hubNamespace: hub.namespace,
      userId: undefined,
    })
    expect(response.status).toBe(401)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('unauthenticated')
    expect(hub.requests).toHaveLength(0)
    expect(hub.idsRequested).toHaveLength(0)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test.each<Record<string, string>>([
    { scope: 'not-a-scope' },
    { cursor: '-1', scope: OWN_SCOPE },
    { cursor: 'abc', scope: OWN_SCOPE },
    { cursor: '1.5', scope: OWN_SCOPE },
  ])('invalid query %j is 400 invalid_request', async params => {
    const response = await run({ request: get(params) })
    expect(response.status).toBe(400)
    expect((await syncErrorBody(response)).code).toBe('invalid_request')
  })

  test('missing scope is 400 invalid_request', async () => {
    const response = await run({ request: get({}) })
    expect(response.status).toBe(400)
  })

  test("another user's personal scope is 403 BEFORE any hub contact", async () => {
    const hub = fakeHub(() => {
      throw new Error('hub must not be reached for an unauthorized scope')
    })
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: personalScope('user-2') }),
    })
    expect(response.status).toBe(403)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('unauthorized_scope')
    expect(hub.requests).toHaveLength(0)
  })

  test('membership scopes (scope.team.*) are denied by the v1 gate', async () => {
    const response = await run({ request: get({ scope: 'scope.team.t-1' }) })
    expect(response.status).toBe(403)
  })

  test('a plain HTTP request (no Upgrade header) is 426 invalid_request', async () => {
    const response = await run({ request: get({ scope: OWN_SCOPE }, {}) })
    expect(response.status).toBe(426)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('invalid_request')
    expect(body.messageSafe).toContain('/api/sync/log')
  })

  test('missing KHALA_SYNC_HUB binding is 503 storage_unavailable', async () => {
    const response = await run({ hubNamespace: undefined })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('forwards the upgrade to the per-scope hub /connect and passes the response through', async () => {
    const sentinel = new Response('sentinel', {
      headers: { 'x-hub-upgrade': 'yes' },
      status: 200,
    })
    const hub = fakeHub(() => sentinel)
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ cursor: '17', scope: OWN_SCOPE }),
    })
    // The route must not rewrap the hub's upgrade response: the webSocket
    // end rides the Response object itself in the Workers runtime.
    expect(response).toBe(sentinel)

    expect(hub.idsRequested).toEqual([OWN_SCOPE])
    expect(hub.requests).toHaveLength(1)
    const forwarded = hub.requests[0]!
    const forwardedUrl = new URL(forwarded.url)
    expect(forwardedUrl.pathname).toBe('/connect')
    expect(forwardedUrl.searchParams.get('scope')).toBe(OWN_SCOPE)
    expect(forwardedUrl.searchParams.get('cursor')).toBe('17')
    expect(forwarded.method).toBe('GET')
    // The WebSocket Upgrade header must survive the proxy.
    expect(forwarded.headers.get('upgrade')).toBe('websocket')
  })

  test('cursor defaults to 0 when absent', async () => {
    const hub = fakeHub(() => new Response(null, { status: 204 }))
    await run({
      hubNamespace: hub.namespace,
      request: get({ scope: OWN_SCOPE }),
    })
    const forwardedUrl = new URL(hub.requests[0]!.url)
    expect(forwardedUrl.searchParams.get('cursor')).toBe('0')
  })

  test('public scopes are connectable by any authenticated user', async () => {
    const hub = fakeHub(() => new Response(null, { status: 204 }))
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: PUBLIC_SCOPE }),
    })
    expect(response.status).toBe(204)
  })

  test('a throwing hub stub is 500 internal (never an unhandled rejection)', async () => {
    const hub = fakeHub(() => {
      throw new Error('hub exploded')
    })
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: OWN_SCOPE }),
    })
    expect(response.status).toBe(500)
    expect((await syncErrorBody(response)).code).toBe('internal')
  })
})
