// Route tests for GET /api/sync/connect (KS-4.4, #8297): auth BEFORE the
// upgrade, param validation, the KS-7.1 scope gate, 426 for non-WebSocket requests,
// 503 while the hub binding is absent, and the DO WebSocket-proxy forward
// (per-scope idFromName, /connect target with scope + cursor params, Upgrade
// header preserved, hub response passed through untouched). All seams are
// injected — no network, no Durable Objects.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { personalScope, publicScope } from '@openagentsinc/khala-sync'
import { resolveScopeRead } from '@openagentsinc/khala-sync-server'

import type {
  KhalaSyncHubNamespaceLike,
  KhalaSyncHubStubLike,
} from './khala-sync-hub-do'
import {
  handleKhalaSyncConnect,
  KHALA_SYNC_CONNECT_PATH,
  type KhalaSyncConnectDependencies,
} from './khala-sync-connect-routes'
import type { KhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

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

/**
 * The REAL package resolver (KS-7.1) over deterministic in-test
 * capabilities: own personal scope + public scopes, plus any teams in
 * `memberOfTeams`. Same shape as the log/bootstrap suites.
 */
const testResolver = (
  input: Readonly<{ memberOfTeams?: ReadonlySet<string> }> = {},
): KhalaSyncScopeReadResolver =>
  (userId, scope) =>
    resolveScopeRead(
      {
        canReadAgentRun: async () => false,
        canReadThread: async () => false,
        isTeamMember: async (uid, teamId) =>
          uid === userId && (input.memberOfTeams?.has(teamId) ?? false),
        readFleetScopeOwner: async () => null,
      },
      userId,
      scope,
    )

const defaultResolveScopeRead = testResolver()

const run = (
  input: Readonly<{
    request?: Request
    userId?: string | undefined
    hubNamespace?: KhalaSyncHubNamespaceLike | undefined
    resolveScopeRead?: KhalaSyncScopeReadResolver
    anonymousRateLimit?: (request: Request) => boolean
  }> = {},
) => {
  const deps: KhalaSyncConnectDependencies = {
    anonymousRateLimit: input.anonymousRateLimit,
    authenticate: async () =>
      'userId' in input
        ? input.userId === undefined
          ? undefined
          : { userId: input.userId }
        : { userId: USER_ID },
    hubNamespace: 'hubNamespace' in input ? input.hubNamespace : undefined,
    resolveScopeRead: input.resolveScopeRead ?? defaultResolveScopeRead,
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

  test('a NON-MEMBER is denied a team scope BEFORE any hub contact', async () => {
    const hub = fakeHub(() => {
      throw new Error('hub must not be reached for an unauthorized scope')
    })
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: 'scope.team.t-1' }),
    })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unauthorized_scope')
    expect(hub.requests).toHaveLength(0)
  })

  test('a LIVE team member connects to the team scope (upgrade forwarded to the hub)', async () => {
    const hub = fakeHub(() => new Response(null, { status: 204 }))
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: 'scope.team.t-1' }),
      resolveScopeRead: testResolver({ memberOfTeams: new Set(['t-1']) }),
    })
    expect(response.status).toBe(204)
    expect(hub.requests).toHaveLength(1)
  })

  test('an unknown scope taxonomy member is gated CLOSED (403 unknown_scope)', async () => {
    const response = await run({
      request: get({ scope: 'scope.workspace.w-1' }),
    })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unknown_scope')
  })

  test('a failed authorization lookup fails CLOSED as 503 retryable, BEFORE any hub contact', async () => {
    const hub = fakeHub(() => {
      throw new Error('hub must not be reached when authorization is unavailable')
    })
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: 'scope.team.t-1' }),
      resolveScopeRead: async () => ({
        kind: 'unavailable',
        messageSafe: 'authorization lookup failed; retry the request.',
      }),
    })
    expect(response.status).toBe(503)
    expect((await syncErrorBody(response)).code).toBe('storage_unavailable')
    expect(hub.requests).toHaveLength(0)
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

  // ---------------------------------------------------------------------
  // KS-8.x anonymous-read exception: scope.public.* is connectable WITHOUT
  // an authenticated actor; every other scope kind still 401s.
  // ---------------------------------------------------------------------

  test('POSITIVE: an ANONYMOUS caller (no session/token) can connect to a public scope', async () => {
    const hub = fakeHub(() => new Response(null, { status: 204 }))
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: PUBLIC_SCOPE }),
      userId: undefined,
    })
    expect(response.status).toBe(204)
    expect(hub.requests).toHaveLength(1)
    const forwardedUrl = new URL(hub.requests[0]!.url)
    expect(forwardedUrl.searchParams.get('scope')).toBe(PUBLIC_SCOPE)
  })

  test.each<Record<string, string>>([
    { scope: OWN_SCOPE },
    { scope: personalScope('user-2') },
    { scope: 'scope.team.t-1' },
    { scope: 'scope.agent_run.run-1' },
    { scope: 'scope.thread.thread-1' },
    { scope: 'scope.fleet_run.fleet-1' },
    { scope: 'scope.workspace.w-1' },
  ])(
    'SECURITY NEGATIVE: an ANONYMOUS caller reading a NON-public scope %j is still 401, hub never contacted',
    async params => {
      const hub = fakeHub(() => {
        throw new Error('hub must not be reached for an anonymous non-public read')
      })
      const response = await run({
        hubNamespace: hub.namespace,
        request: get(params),
        userId: undefined,
      })
      expect(response.status).toBe(401)
      const body = await syncErrorBody(response)
      expect(body.code).toBe('unauthenticated')
      expect(hub.requests).toHaveLength(0)
    },
  )

  test('SECURITY: a scope kind crafted to LOOK public ("scope.public_evil.x") does NOT grant anonymous access', async () => {
    const hub = fakeHub(() => {
      throw new Error('hub must not be reached for a non-public scope kind')
    })
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: 'scope.public_evil.x' }),
      userId: undefined,
    })
    expect(response.status).toBe(401)
    expect(hub.requests).toHaveLength(0)
  })

  test('an authenticated caller connecting to a public scope still passes their OWN userId through to resolveScopeRead (unchanged by the anonymous exception)', async () => {
    const hub = fakeHub(() => new Response(null, { status: 204 }))
    const seen: Array<string | undefined> = []
    const response = await run({
      hubNamespace: hub.namespace,
      request: get({ scope: PUBLIC_SCOPE }),
      resolveScopeRead: async (userId, scope) => {
        seen.push(userId)
        return defaultResolveScopeRead(userId, scope)
      },
    })
    expect(response.status).toBe(204)
    expect(seen).toEqual([USER_ID])
  })

  test('an anonymous connect that fails the rate limiter is 429 rate_limited, hub never contacted', async () => {
    const hub = fakeHub(() => {
      throw new Error('hub must not be reached when the anonymous rate limit denies')
    })
    const response = await run({
      anonymousRateLimit: () => false,
      hubNamespace: hub.namespace,
      request: get({ scope: PUBLIC_SCOPE }),
      userId: undefined,
    })
    expect(response.status).toBe(429)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('rate_limited')
    expect(body.retryable).toBe(true)
    expect(hub.requests).toHaveLength(0)
  })

  test('the anonymous rate limiter is NEVER consulted for an authenticated connect, even to a public scope', async () => {
    const hub = fakeHub(() => new Response(null, { status: 204 }))
    let calls = 0
    const response = await run({
      anonymousRateLimit: () => {
        calls += 1
        return false
      },
      hubNamespace: hub.namespace,
      request: get({ scope: PUBLIC_SCOPE }),
    })
    expect(response.status).toBe(204)
    expect(calls).toBe(0)
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
