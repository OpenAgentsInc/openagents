// Wiring-level tests for the Khala Sync route family (ST-3, #8509).
//
// Every other khala-sync route test injects a FAKE `authenticate` — which is
// exactly why the 2026-07-06 production bug (the `/api/sync/connect` route
// wiring authenticating the RAW request instead of the `?token=`-normalized
// one, 401ing every mobile live tail) was invisible to the whole suite: the
// fake never cared which request it got. These tests close that class by
// driving the REAL wiring exported from `./index` — the actual route-table
// handlers and `khalaSyncRouteWiring` factories, whose `authenticate` is the
// production `authenticateRequestActor` closure — against a FAKE env (an
// in-memory D1 answering the agent-credential token-hash lookup for one
// known bearer, a recording hub namespace, no network).
//
// The load-bearing assertions:
//  - CONNECT: a request whose ONLY credential is a `?token=<agent-bearer>`
//    query param (NO Authorization header — a WebSocket client cannot set
//    one) authenticates through the real route-table entry and reaches the
//    hub. This test FAILS with a 401 if the route is ever rewired to
//    authenticate a request lacking the promoted Authorization header (the
//    exact production regression).
//  - LOG / BOOTSTRAP / PUSH: the same query-param-only credential is
//    REJECTED (401) — these are plain HTTP routes that read the
//    Authorization header directly, documenting that connect is the one
//    special WS case — while the same bearer in an Authorization header
//    authenticates (the request proceeds past auth to the storage/body
//    gates).
//
// Audit refs: docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md §R3,
// docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  KHALA_SYNC_PROTOCOL_VERSION,
  personalScope,
} from '@openagentsinc/khala-sync'

import { AGENT_TOKEN_PREFIX, sha256Hex } from './agent-registration'
import {
  exactRouteHandlerForPath,
  khalaSyncRouteWiring,
  type Env,
} from './index'
import {
  KHALA_SYNC_BOOTSTRAP_PATH,
} from './khala-sync-bootstrap-routes'
import {
  KHALA_SYNC_CONNECT_PATH,
  withBearerFromQueryToken,
} from './khala-sync-connect-routes'
import type {
  KhalaSyncHubNamespaceLike,
  KhalaSyncHubStubLike,
} from './khala-sync-hub-do'
import { KHALA_SYNC_LOG_PATH } from './khala-sync-log-routes'
import { KHALA_SYNC_PUSH_PATH } from './khala-sync-push-routes'

// ---------------------------------------------------------------------------
// The one known agent the fake env recognizes
// ---------------------------------------------------------------------------

const AGENT_BEARER = `${AGENT_TOKEN_PREFIX}st3_wiring_test_bearer`
const AGENT_USER_ID = 'user_agent_st3_wiring'
const AGENT_SCOPE = personalScope(AGENT_USER_ID)

// ---------------------------------------------------------------------------
// Fake env: in-memory D1 + recording hub. No network, no Durable Objects.
// ---------------------------------------------------------------------------

/**
 * A D1 fake shaped for the REAL `makeAgentRegistrationStoreForEnv` D1 store:
 * the agent-credential token-hash SELECT answers the known agent's row for
 * the known bearer's hash (and null otherwise); every other statement
 * answers empty. Any unexpectedly-required surface throws loudly.
 */
const fakeAgentD1 = (bearerTokenHash: string) => {
  const agentRow = {
    avatar_url: null,
    created_at: '2026-07-06T00:00:00.000Z',
    credential_id: 'agent_credential_st3_wiring',
    display_name: 'ST3 Wiring Test Agent',
    metadata_json: '{}',
    openauth_user_id: null,
    primary_email: null,
    status: 'active',
    token_prefix: AGENT_BEARER.slice(0, 16),
    updated_at: '2026-07-06T00:00:00.000Z',
    user_id: AGENT_USER_ID,
  }
  const statement = (sql: string, args: ReadonlyArray<unknown>) => ({
    all: async () => ({ results: [], success: true }),
    bind: (...bound: ReadonlyArray<unknown>) => statement(sql, bound),
    first: async () =>
      sql.includes('FROM agent_credentials') && args[0] === bearerTokenHash
        ? agentRow
        : null,
    raw: async () => [],
    run: async () => ({ meta: {}, results: [], success: true }),
  })
  return {
    batch: async () => [],
    prepare: (sql: string) => statement(sql, []),
  }
}

/** CFG-4 Domain 2 (#8519): the Postgres identity handle for the gate's
 * `users` read — answers the known agent's active row. */
const fakeIdentityDb = () => ({
  batch: () => Promise.resolve(),
  query: (sql: string, params: ReadonlyArray<unknown> = []) =>
    Promise.resolve(
      sql.includes('FROM users') && params.map(String).includes(AGENT_USER_ID)
        ? [
            {
              avatar_url: null,
              created_at: '2026-07-06T00:00:00.000Z',
              display_name: 'ST3 Wiring Test Agent',
              id: AGENT_USER_ID,
              primary_email: null,
              updated_at: '2026-07-06T00:00:00.000Z',
            },
          ]
        : [],
    ),
})

/** A hub namespace whose per-scope stub records and answers `respond`. */
const fakeHub = (respond: (request: Request) => Response) => {
  const forwarded: Array<Request> = []
  const idsRequested: Array<string> = []
  const stub: KhalaSyncHubStubLike = {
    fetch: async request => {
      forwarded.push(request)
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
  return { forwarded, idsRequested, namespace }
}

const makeFakeEnv = async (
  options: Readonly<{ hub?: KhalaSyncHubNamespaceLike }> = {},
): Promise<Env> =>
  ({
    // Never consulted on the agent-bearer path (agent auth resolves before
    // the mobile-session bearer fallback); present so an accidental read
    // fails loudly instead of silently succeeding.
    AUTH_STORAGE: {
      get: () => {
        throw new Error('AUTH_STORAGE must not be consulted in these tests')
      },
    },
    OPENAGENTS_DB: fakeAgentD1(await sha256Hex(AGENT_BEARER)),
    // CFG-4 Domain 2 (#8519): the agent auth gate's `users` half reads the
    // Postgres identity handle — the fake env serves the known agent's row
    // through the IDENTITY_DB test-override slot.
    IDENTITY_DB: fakeIdentityDb(),
    ...(options.hub === undefined ? {} : { KHALA_SYNC_HUB: options.hub }),
    // KHALA_SYNC_DB deliberately absent: every identity/remainder mirror
    // resolves to undefined and the scope resolver stays in-memory for
    // `scope.user.*` (no capability callback runs for an owner match).
  }) as unknown as Env

const fakeCtx = (): ExecutionContext =>
  ({
    passThroughOnException: () => {},
    props: {},
    waitUntil: () => {},
  }) as unknown as ExecutionContext

const routeHandler = (path: string) => {
  const handler = exactRouteHandlerForPath(path)
  if (handler === undefined) {
    throw new Error(`route table has no entry for ${path}`)
  }
  return handler
}

const dispatch = async (path: string, request: Request, env: Env) =>
  Effect.runPromise(routeHandler(path)(request, env, fakeCtx()))

const syncBody = async (response: Response) =>
  (await response.json()) as { code?: string }

// ---------------------------------------------------------------------------
// CONNECT — the WS special case: `?token=` is the ONLY credential channel
// ---------------------------------------------------------------------------

describe('connect route wiring (real authenticateRequestActor)', () => {
  test('a `?token=` query bearer with NO Authorization header authenticates through the REAL route-table entry and reaches the hub', async () => {
    const hub = fakeHub(() => new Response('{"hub":true}', { status: 200 }))
    const env = await makeFakeEnv({ hub: hub.namespace })
    const url = new URL(`https://openagents.com${KHALA_SYNC_CONNECT_PATH}`)
    url.searchParams.set('scope', AGENT_SCOPE)
    url.searchParams.set('token', AGENT_BEARER)
    const request = new Request(url.toString(), {
      headers: { upgrade: 'websocket' },
      method: 'GET',
    })
    expect(request.headers.get('authorization')).toBeNull()

    const response = await dispatch(KHALA_SYNC_CONNECT_PATH, request, env)

    // The 2026-07-06 regression (wiring `authenticate` to the RAW request,
    // which has no Authorization header) turns this exact response into a
    // 401 `unauthenticated` — this assertion is the standing tripwire.
    expect(response.status).toBe(200)
    expect(hub.idsRequested).toEqual([AGENT_SCOPE])
    expect(hub.forwarded).toHaveLength(1)
  })

  test('with no credential at all the same route-table entry answers 401 for a private scope', async () => {
    const hub = fakeHub(() => new Response('{"hub":true}', { status: 200 }))
    const env = await makeFakeEnv({ hub: hub.namespace })
    const url = new URL(`https://openagents.com${KHALA_SYNC_CONNECT_PATH}`)
    url.searchParams.set('scope', AGENT_SCOPE)
    const request = new Request(url.toString(), {
      headers: { upgrade: 'websocket' },
      method: 'GET',
    })

    const response = await dispatch(KHALA_SYNC_CONNECT_PATH, request, env)

    expect(response.status).toBe(401)
    expect((await syncBody(response)).code).toBe('unauthenticated')
    expect(hub.forwarded).toHaveLength(0)
  })

  test("makeConnectDeps' authenticate resolves the actor from its ARGUMENT (the normalized request), never a closed-over raw request", async () => {
    const env = await makeFakeEnv()
    const deps = khalaSyncRouteWiring.makeConnectDeps(env, fakeCtx())
    const url = new URL(`https://openagents.com${KHALA_SYNC_CONNECT_PATH}`)
    url.searchParams.set('scope', AGENT_SCOPE)
    url.searchParams.set('token', AGENT_BEARER)
    const rawRequest = new Request(url.toString(), { method: 'GET' })

    // The handler's production composition: promote `?token=` into an
    // Authorization header (the real helper), then authenticate THAT.
    await expect(
      deps.authenticate(withBearerFromQueryToken(rawRequest)),
    ).resolves.toEqual({ userId: AGENT_USER_ID })

    // The raw request (header-less) must NOT authenticate: proves the
    // closure reads its argument — were it wired to a captured raw request,
    // both calls would answer identically and the distinction would vanish.
    await expect(deps.authenticate(rawRequest)).resolves.toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// LOG / BOOTSTRAP / PUSH — header-read routes: `?token=` must NOT work
// ---------------------------------------------------------------------------

describe('log route wiring (real authenticateRequestActor)', () => {
  const logUrl = (params: Readonly<Record<string, string>>) => {
    const url = new URL(`https://openagents.com${KHALA_SYNC_LOG_PATH}`)
    for (const [key, value] of Object.entries(params)) {
      url.searchParams.set(key, value)
    }
    return url.toString()
  }

  test('an Authorization-header agent bearer authenticates through the REAL route-table entry (proceeds past auth to the storage gate)', async () => {
    const env = await makeFakeEnv()
    const request = new Request(logUrl({ scope: AGENT_SCOPE }), {
      headers: { authorization: `Bearer ${AGENT_BEARER}` },
      method: 'GET',
    })

    const response = await dispatch(KHALA_SYNC_LOG_PATH, request, env)

    // Past auth + the scope gate; 503 is the absent-KHALA_SYNC_DB storage
    // gate — NOT 401. Auth mis-wiring flips this to 401 `unauthenticated`.
    expect(response.status).toBe(503)
    expect((await syncBody(response)).code).toBe('storage_unavailable')
  })

  test('a `?token=` query bearer is NOT a credential here — connect is the one special WS case', async () => {
    const env = await makeFakeEnv()
    const request = new Request(
      logUrl({ scope: AGENT_SCOPE, token: AGENT_BEARER }),
      { method: 'GET' },
    )

    const response = await dispatch(KHALA_SYNC_LOG_PATH, request, env)

    expect(response.status).toBe(401)
    expect((await syncBody(response)).code).toBe('unauthenticated')
  })
})

describe('bootstrap route wiring (real authenticateRequestActor)', () => {
  const bootstrapRequest = (
    headers: Readonly<Record<string, string>>,
    tokenQueryParam?: string,
  ) => {
    const url = new URL(`https://openagents.com${KHALA_SYNC_BOOTSTRAP_PATH}`)
    if (tokenQueryParam !== undefined) {
      url.searchParams.set('token', tokenQueryParam)
    }
    return new Request(url.toString(), {
      body: JSON.stringify({
        clientGroupId: 'cg-st3-wiring',
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        schemaVersion: 1,
        scope: AGENT_SCOPE,
      }),
      headers: { 'content-type': 'application/json', ...headers },
      method: 'POST',
    })
  }

  test('an Authorization-header agent bearer authenticates through the REAL route-table entry (proceeds past auth to the storage gate)', async () => {
    const env = await makeFakeEnv()
    const request = bootstrapRequest({
      authorization: `Bearer ${AGENT_BEARER}`,
    })

    const response = await dispatch(KHALA_SYNC_BOOTSTRAP_PATH, request, env)

    expect(response.status).toBe(503)
    expect((await syncBody(response)).code).toBe('storage_unavailable')
  })

  test('a `?token=` query bearer is NOT a credential here — connect is the one special WS case', async () => {
    const env = await makeFakeEnv()
    const request = bootstrapRequest({}, AGENT_BEARER)

    const response = await dispatch(KHALA_SYNC_BOOTSTRAP_PATH, request, env)

    expect(response.status).toBe(401)
    expect((await syncBody(response)).code).toBe('unauthenticated')
  })
})

describe('push route wiring (real authenticateRequestActor)', () => {
  const pushRequest = (
    headers: Readonly<Record<string, string>>,
    tokenQueryParam?: string,
  ) => {
    const url = new URL(`https://openagents.com${KHALA_SYNC_PUSH_PATH}`)
    if (tokenQueryParam !== undefined) {
      url.searchParams.set('token', tokenQueryParam)
    }
    return new Request(url.toString(), {
      body: '{}',
      headers: { 'content-type': 'application/json', ...headers },
      method: 'POST',
    })
  }

  test('an Authorization-header agent bearer authenticates through the REAL route-table entry (proceeds past auth to the body gates)', async () => {
    const env = await makeFakeEnv()
    const request = pushRequest({ authorization: `Bearer ${AGENT_BEARER}` })

    const response = await dispatch(KHALA_SYNC_PUSH_PATH, request, env)

    // Push authenticates FIRST; the empty body then fails the protocol
    // version gate (400) — proving the request got past auth. Auth
    // mis-wiring flips this to 401 `unauthenticated`.
    expect(response.status).toBe(400)
    expect((await syncBody(response)).code).toBe(
      'protocol_version_unsupported',
    )
  })

  test('a `?token=` query bearer is NOT a credential here — connect is the one special WS case', async () => {
    const env = await makeFakeEnv()
    const request = pushRequest({}, AGENT_BEARER)

    const response = await dispatch(KHALA_SYNC_PUSH_PATH, request, env)

    expect(response.status).toBe(401)
    expect((await syncBody(response)).code).toBe('unauthenticated')
  })
})
