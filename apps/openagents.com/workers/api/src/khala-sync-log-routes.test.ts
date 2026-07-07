// Route tests for GET /api/sync/log (KS-4.3, #8296): auth guard, v1 scope
// gate, param validation, hub-first serving, hub-410/409 → Postgres
// fallthrough, Postgres behind-window → 410 MustRefetch, cache headers
// (ETag on (scope, nextCursor), 304 revalidation, no-store at the live
// edge), and client teardown. All seams are injected — no network, no
// database, no Durable Objects.

import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  decodeChangelogEntry,
  decodeLogPage,
  KHALA_SYNC_PROTOCOL_VERSION,
  LogPage,
  personalScope,
  publicScope,
  SyncScope,
  SyncVersionWatermark,
} from '@openagentsinc/khala-sync'
import {
  DEFAULT_LOG_PAGE_LIMIT,
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncStorageError,
  resolveScopeRead,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import type {
  KhalaSyncHubNamespaceLike,
  KhalaSyncHubStubLike,
} from './khala-sync-hub-do'
import {
  handleKhalaSyncLog,
  KHALA_SYNC_LOG_PATH,
  type KhalaSyncLogDependencies,
  logPageEtag,
  type LogPageFromPostgresFn,
} from './khala-sync-log-routes'
import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import {
  makeKhalaSyncScopeReadResolver,
  type KhalaSyncScopeReadResolver,
} from './khala-sync-scope-auth'

const encodeLogPage = S.encodeSync(LogPage)
const scopeOf = S.decodeUnknownSync(SyncScope)

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@hyperdrive.local:5432/khala_sync_test'

const USER_ID = 'user-1'
const OWN_SCOPE = personalScope(USER_ID)
const PUBLIC_SCOPE = publicScope('artanis.global')

const entry = (version: number, entityId = `e-${version}`) =>
  decodeChangelogEntry({
    committedAt: '2026-07-04T00:00:00.000Z',
    entityId,
    entityType: 'note',
    op: 'upsert',
    postImageJson: JSON.stringify({ id: entityId }),
    scope: OWN_SCOPE,
    version,
  })

const page = (
  input: Readonly<{
    scope?: string
    versions?: ReadonlyArray<number>
    nextCursor: number
    upToDate: boolean
  }>,
) =>
  new LogPage({
    entries: (input.versions ?? []).map(version => entry(version)),
    nextCursor: SyncVersionWatermark.make(input.nextCursor),
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    scope: scopeOf(input.scope ?? OWN_SCOPE),
    upToDate: input.upToDate,
  })

const get = (
  params: Readonly<Record<string, string>> = { scope: OWN_SCOPE },
  headers: Readonly<Record<string, string>> = {},
) => {
  const url = new URL(`https://openagents.com${KHALA_SYNC_LOG_PATH}`)
  for (const [key, value] of Object.entries(params)) {
    url.searchParams.set(key, value)
  }
  return new Request(url.toString(), { headers, method: 'GET' })
}

const fakeSqlHandle = (() => {
  throw new Error('fake sql handle must never be queried in route tests')
}) as unknown as SyncSql

const makeFakeClient = () => {
  let ended = 0
  const client: KhalaSyncPushSqlClient = {
    end: () => {
      ended += 1
      return Promise.resolve()
    },
    sql: fakeSqlHandle,
  }
  return { client, endedCount: () => ended }
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

const hubServing = (served: LogPage) =>
  fakeHub(() => Response.json(encodeLogPage(served)))

const neverMakeSqlClient = async (): Promise<KhalaSyncPushSqlClient> => {
  throw new Error('makeSqlClient must not be called on a hub window hit')
}

const neverLogPage: LogPageFromPostgresFn = async () => {
  throw new Error('postgres logPage must not be called on a hub window hit')
}

/**
 * The REAL package resolver (KS-7.1) over deterministic in-test
 * capabilities. Defaults preserve the pre-KS-7.1 test surface: own
 * personal scope + public scopes only; membership/ownership denies.
 */
const testResolver = (
  input: Readonly<{
    memberOfTeams?: ReadonlySet<string>
    fleetOwners?: Readonly<Record<string, string>>
  }> = {},
): KhalaSyncScopeReadResolver =>
  (userId, scope) =>
    resolveScopeRead(
      {
        canReadAgentRun: async () => false,
        canReadThread: async () => false,
        isTeamMember: async (uid, teamId) =>
          uid === userId && (input.memberOfTeams?.has(teamId) ?? false),
        readFleetScopeOwner: async requested =>
          input.fleetOwners?.[requested] ?? null,
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
    binding?: { connectionString: string } | undefined
    client?: KhalaSyncPushSqlClient
    logPage?: LogPageFromPostgresFn
    resolveScopeRead?: KhalaSyncScopeReadResolver
    anonymousRateLimit?: (request: Request) => boolean
  }> = {},
) => {
  const deps: KhalaSyncLogDependencies = {
    anonymousRateLimit: input.anonymousRateLimit,
    authenticate: async () =>
      'userId' in input
        ? input.userId === undefined
          ? undefined
          : { userId: input.userId }
        : { userId: USER_ID },
    binding:
      'binding' in input
        ? input.binding
        : { connectionString: FAKE_CONNECTION_STRING },
    hubNamespace: 'hubNamespace' in input ? input.hubNamespace : undefined,
    logPageFromPostgres: input.logPage ?? neverLogPage,
    makeSqlClient: async () => input.client ?? makeFakeClient().client,
    resolveScopeRead: input.resolveScopeRead ?? defaultResolveScopeRead,
  }
  return Effect.runPromise(handleKhalaSyncLog(input.request ?? get(), deps))
}

const syncErrorBody = async (response: Response) =>
  (await response.json()) as {
    code: string
    messageSafe: string
    retryable: boolean
  }

describe('handleKhalaSyncLog', () => {
  test('non-GET methods are 405', async () => {
    const response = await run({
      request: new Request(`https://openagents.com${KHALA_SYNC_LOG_PATH}`, {
        method: 'POST',
      }),
    })
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })

  test('unauthenticated requests are 401 typed SyncError', async () => {
    const response = await run({ userId: undefined })
    expect(response.status).toBe(401)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('unauthenticated')
    expect(body.retryable).toBe(false)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test.each<Record<string, string>>([
    { scope: 'not-a-scope' },
    { cursor: '-1', scope: OWN_SCOPE },
    { cursor: 'abc', scope: OWN_SCOPE },
    { cursor: '1.5', scope: OWN_SCOPE },
    { limit: '0', scope: OWN_SCOPE },
    { limit: 'abc', scope: OWN_SCOPE },
  ])('invalid query %j is 400 invalid_request', async params => {
    const response = await run({ request: get(params) })
    expect(response.status).toBe(400)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('invalid_request')
  })

  test('missing scope is 400 invalid_request', async () => {
    const response = await run({ request: get({}) })
    expect(response.status).toBe(400)
    expect((await syncErrorBody(response)).code).toBe('invalid_request')
  })

  test("another user's personal scope is 403 unauthorized_scope", async () => {
    const response = await run({
      request: get({ scope: personalScope('user-2') }),
    })
    expect(response.status).toBe(403)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('unauthorized_scope')
    expect(body.retryable).toBe(false)
  })

  test('a NON-MEMBER is denied a team scope (403 unauthorized_scope)', async () => {
    const response = await run({ request: get({ scope: 'scope.team.t-1' }) })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unauthorized_scope')
  })

  test('a LIVE team member reads the team scope (hub-served)', async () => {
    const TEAM_SCOPE = scopeOf('scope.team.t-1')
    const served = page({ nextCursor: 1, scope: TEAM_SCOPE, upToDate: true })
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ scope: TEAM_SCOPE }),
      resolveScopeRead: testResolver({ memberOfTeams: new Set(['t-1']) }),
    })
    expect(response.status).toBe(200)
  })

  test('an unknown scope taxonomy member is gated CLOSED (403 unknown_scope)', async () => {
    const response = await run({
      request: get({ scope: 'scope.workspace.w-1' }),
    })
    expect(response.status).toBe(403)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('unknown_scope')
    expect(body.retryable).toBe(false)
  })

  test('a failed authorization lookup fails CLOSED as 503 retryable (never a grant)', async () => {
    const response = await run({
      request: get({ scope: 'scope.team.t-1' }),
      resolveScopeRead: testResolver({
        memberOfTeams: {
          has: () => {
            throw new Error('D1 unavailable')
          },
        } as unknown as ReadonlySet<string>,
      }),
    })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Hub-first serving
  // -------------------------------------------------------------------------

  test('hub window hit serves the page without touching Postgres', async () => {
    const served = page({ nextCursor: 3, upToDate: false, versions: [2, 3] })
    const hub = hubServing(served)
    const response = await Effect.runPromise(
      handleKhalaSyncLog(get({ cursor: '1', limit: '2', scope: OWN_SCOPE }), {
        authenticate: async () => ({ userId: USER_ID }),
        resolveScopeRead: defaultResolveScopeRead,
        binding: { connectionString: FAKE_CONNECTION_STRING },
        hubNamespace: hub.namespace,
        logPageFromPostgres: neverLogPage,
        makeSqlClient: neverMakeSqlClient,
      }),
    )
    expect(response.status).toBe(200)
    const body = decodeLogPage(await response.json())
    expect(body.entries).toHaveLength(2)
    expect(Number(body.nextCursor)).toBe(3)
    expect(body.upToDate).toBe(false)

    // The DO is addressed by scope and asked for exactly the parsed params.
    expect(hub.idsRequested).toEqual([OWN_SCOPE])
    const hubUrl = new URL(hub.requests[0]!.url)
    expect(hubUrl.pathname).toBe('/log')
    expect(hubUrl.searchParams.get('scope')).toBe(OWN_SCOPE)
    expect(hubUrl.searchParams.get('cursor')).toBe('1')
    expect(hubUrl.searchParams.get('limit')).toBe('2')
  })

  test('cursor defaults to 0 and limit defaults to the package default', async () => {
    const hub = hubServing(page({ nextCursor: 0, upToDate: true }))
    await Effect.runPromise(
      handleKhalaSyncLog(get({ scope: OWN_SCOPE }), {
        authenticate: async () => ({ userId: USER_ID }),
        resolveScopeRead: defaultResolveScopeRead,
        binding: { connectionString: FAKE_CONNECTION_STRING },
        hubNamespace: hub.namespace,
        logPageFromPostgres: neverLogPage,
        makeSqlClient: neverMakeSqlClient,
      }),
    )
    const hubUrl = new URL(hub.requests[0]!.url)
    expect(hubUrl.searchParams.get('cursor')).toBe('0')
    expect(hubUrl.searchParams.get('limit')).toBe(String(DEFAULT_LOG_PAGE_LIMIT))
  })

  test('limit above the package max is clamped', async () => {
    const hub = hubServing(page({ nextCursor: 0, upToDate: true }))
    await Effect.runPromise(
      handleKhalaSyncLog(get({ limit: '999999', scope: OWN_SCOPE }), {
        authenticate: async () => ({ userId: USER_ID }),
        resolveScopeRead: defaultResolveScopeRead,
        binding: { connectionString: FAKE_CONNECTION_STRING },
        hubNamespace: hub.namespace,
        logPageFromPostgres: neverLogPage,
        makeSqlClient: neverMakeSqlClient,
      }),
    )
    const hubUrl = new URL(hub.requests[0]!.url)
    expect(hubUrl.searchParams.get('limit')).toBe('1000')
  })

  // -------------------------------------------------------------------------
  // Fallthrough to Postgres
  // -------------------------------------------------------------------------

  const behindWindowHub = () =>
    fakeHub(() =>
      Response.json(
        {
          _tag: 'SyncError',
          code: 'cursor_behind_retained_window',
          messageSafe: 'behind hub window',
          retryable: false,
        },
        { status: 410 },
      ),
    )

  test('hub 410 behind-window falls through to the Postgres read', async () => {
    const served = page({ nextCursor: 9, upToDate: true, versions: [9] })
    const captured: Array<unknown> = []
    const fake = makeFakeClient()
    const response = await run({
      client: fake.client,
      hubNamespace: behindWindowHub().namespace,
      logPage: async (sql, input) => {
        captured.push(input)
        expect(sql).toBe(fakeSqlHandle)
        return served
      },
      request: get({ cursor: '8', limit: '10', scope: OWN_SCOPE }),
    })
    expect(response.status).toBe(200)
    expect(decodeLogPage(await response.json()).upToDate).toBe(true)
    expect(captured).toEqual([
      { afterVersion: 8, limit: 10, scope: OWN_SCOPE },
    ])
    expect(fake.endedCount()).toBe(1)
  })

  test('hub 409 (cursor ahead of a reset hub) falls through to Postgres', async () => {
    const hub = fakeHub(() =>
      Response.json(
        {
          _tag: 'SyncError',
          code: 'storage_unavailable',
          messageSafe: 'ahead of window',
          retryable: true,
        },
        { status: 409 },
      ),
    )
    const response = await run({
      hubNamespace: hub.namespace,
      logPage: async () => page({ nextCursor: 4, upToDate: true, versions: [4] }),
      request: get({ cursor: '3', scope: OWN_SCOPE }),
    })
    expect(response.status).toBe(200)
    expect(hub.requests).toHaveLength(1)
  })

  test('a throwing hub stub falls through to Postgres', async () => {
    const hub = fakeHub(() => {
      throw new Error('hub exploded')
    })
    const response = await run({
      hubNamespace: hub.namespace,
      logPage: async () => page({ nextCursor: 1, upToDate: true, versions: [1] }),
    })
    expect(response.status).toBe(200)
  })

  test('a hub 200 with an undecodable body falls through to Postgres', async () => {
    const hub = fakeHub(() => Response.json({ nonsense: true }))
    const response = await run({
      hubNamespace: hub.namespace,
      logPage: async () => page({ nextCursor: 1, upToDate: true, versions: [1] }),
    })
    expect(response.status).toBe(200)
  })

  test('absent hub binding goes straight to Postgres', async () => {
    const response = await run({
      hubNamespace: undefined,
      logPage: async () => page({ nextCursor: 2, upToDate: true, versions: [2] }),
    })
    expect(response.status).toBe(200)
  })

  // -------------------------------------------------------------------------
  // Postgres error mapping
  // -------------------------------------------------------------------------

  test('missing KHALA_SYNC_DB binding is 503 storage_unavailable', async () => {
    const response = await run({ binding: undefined })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('Postgres behind-retained-window is a 410 MustRefetch SyncError', async () => {
    const fake = makeFakeClient()
    const response = await run({
      client: fake.client,
      logPage: async () => {
        throw new KhalaSyncCursorBehindRetainedWindowError(OWN_SCOPE, 1, 50)
      },
      request: get({ cursor: '1', scope: OWN_SCOPE }),
    })
    expect(response.status).toBe(410)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('cursor_behind_retained_window')
    expect(body.retryable).toBe(false)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(fake.endedCount()).toBe(1)
  })

  test('Postgres storage failure is 503 retryable', async () => {
    const response = await run({
      logPage: async () => {
        throw new KhalaSyncStorageError('connection_failed', 'no route to db')
      },
    })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('unexpected Postgres failure is 500 internal', async () => {
    const response = await run({
      logPage: async () => {
        throw new Error('boom')
      },
    })
    expect(response.status).toBe(500)
    expect((await syncErrorBody(response)).code).toBe('internal')
  })

  test('the sql client is torn down even when the read throws', async () => {
    const fake = makeFakeClient()
    await run({
      client: fake.client,
      logPage: async () => {
        throw new Error('boom')
      },
    })
    expect(fake.endedCount()).toBe(1)
  })

  // -------------------------------------------------------------------------
  // Cache headers (ETag on (scope, nextCursor))
  // -------------------------------------------------------------------------

  test('non-upToDate pages carry a stable ETag and private revalidation', async () => {
    const served = page({ nextCursor: 5, upToDate: false, versions: [4, 5] })
    const first = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ cursor: '3', scope: OWN_SCOPE }),
    })
    const second = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ cursor: '3', scope: OWN_SCOPE }),
    })
    const expected = logPageEtag(served.scope, 5)
    expect(first.status).toBe(200)
    expect(first.headers.get('etag')).toBe(expected)
    expect(second.headers.get('etag')).toBe(expected)
    expect(first.headers.get('cache-control')).toBe(
      'private, max-age=0, must-revalidate',
    )
  })

  test('If-None-Match on a non-upToDate page revalidates to 304', async () => {
    const served = page({ nextCursor: 5, upToDate: false, versions: [4, 5] })
    const etag = logPageEtag(served.scope, 5)
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get(
        { cursor: '3', scope: OWN_SCOPE },
        { 'if-none-match': etag },
      ),
    })
    expect(response.status).toBe(304)
    expect(response.headers.get('etag')).toBe(etag)
    expect(await response.text()).toBe('')
  })

  test('a stale If-None-Match still serves the full page', async () => {
    const served = page({ nextCursor: 5, upToDate: false, versions: [4, 5] })
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get(
        { cursor: '3', scope: OWN_SCOPE },
        { 'if-none-match': logPageEtag(served.scope, 4) },
      ),
    })
    expect(response.status).toBe(200)
  })

  test('upToDate pages are no-store with no ETag (live edge)', async () => {
    const served = page({ nextCursor: 6, upToDate: true, versions: [6] })
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ cursor: '5', scope: OWN_SCOPE }),
    })
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('etag')).toBeNull()
  })

  test('Postgres-served pages get the same cache treatment as hub pages', async () => {
    const served = page({ nextCursor: 7, upToDate: false, versions: [7] })
    const response = await run({
      logPage: async () => served,
      request: get({ cursor: '6', scope: OWN_SCOPE }),
    })
    expect(response.status).toBe(200)
    expect(response.headers.get('etag')).toBe(logPageEtag(served.scope, 7))
    expect(response.headers.get('cache-control')).toBe(
      'private, max-age=0, must-revalidate',
    )
  })

  test('the public scope family is readable by any authenticated user', async () => {
    const served = page({
      nextCursor: 1,
      scope: PUBLIC_SCOPE,
      upToDate: true,
    })
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ scope: PUBLIC_SCOPE }),
    })
    expect(response.status).toBe(200)
  })

  // ---------------------------------------------------------------------
  // KS-8.x anonymous-read exception: scope.public.* is readable WITHOUT an
  // authenticated actor; every other scope kind still 401s.
  // ---------------------------------------------------------------------

  test('POSITIVE: an ANONYMOUS caller (no session/token) can read a public scope', async () => {
    const served = page({ nextCursor: 1, scope: PUBLIC_SCOPE, upToDate: true })
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ scope: PUBLIC_SCOPE }),
      userId: undefined,
    })
    expect(response.status).toBe(200)
  })

  // ---------------------------------------------------------------------
  // CFG D1 evacuation (#8515): a `scope.public.*` read is a public PROJECTION
  // and must NEVER 500. Live on the Cloud Run monolith,
  // `scope.public.activity-timeline` returned an empty-body top-level 500
  // (the oversized cron snapshot row's Postgres read/transaction crashed).
  // Every failure on a public scope now degrades to a 200 EMPTY page
  // (`upToDate:false` so a poller retries); non-public scopes stay fail-CLOSED.
  // ---------------------------------------------------------------------

  test('CFG-#8515: a public scope with a THROWING Postgres read degrades to 200 EMPTY (never 500)', async () => {
    const response = await run({
      request: get({ scope: PUBLIC_SCOPE, cursor: '5' }),
      userId: undefined,
      logPage: async () => {
        throw new Error('d1-http bridge query failed (401)')
      },
    })
    expect(response.status).toBe(200)
    const body = decodeLogPage(await response.json())
    expect(body.entries).toHaveLength(0)
    expect(Number(body.nextCursor)).toBe(5)
    expect(body.upToDate).toBe(false)
  })

  test('CFG-#8515: a public scope with a KhalaSyncStorageError read degrades to 200 EMPTY (never 503)', async () => {
    const response = await run({
      request: get({ scope: PUBLIC_SCOPE }),
      userId: undefined,
      logPage: async () => {
        throw new KhalaSyncStorageError('connection_failed', 'no route to db')
      },
    })
    expect(response.status).toBe(200)
    expect(decodeLogPage(await response.json()).entries).toHaveLength(0)
  })

  test('CFG-#8515: an UNCAUGHT resolver defect on a public scope degrades to 200 EMPTY (never a top-level 500)', async () => {
    const response = await run({
      request: get({ scope: PUBLIC_SCOPE }),
      userId: undefined,
      resolveScopeRead: (() => {
        throw new Error('unexpected resolver defect')
      }) as unknown as KhalaSyncScopeReadResolver,
    })
    expect(response.status).toBe(200)
    expect(decodeLogPage(await response.json()).entries).toHaveLength(0)
  })

  test('CFG-#8515: a public scope with NO KHALA_SYNC_DB binding degrades to 200 EMPTY (not 503)', async () => {
    const response = await run({
      request: get({ scope: PUBLIC_SCOPE }),
      userId: undefined,
      binding: undefined,
    })
    expect(response.status).toBe(200)
    expect(decodeLogPage(await response.json()).entries).toHaveLength(0)
  })

  test('CFG-#8515: a NON-public scope still fails CLOSED (500) on an unexpected read error', async () => {
    const response = await run({
      request: get({ scope: OWN_SCOPE }),
      logPage: async () => {
        throw new Error('boom')
      },
    })
    expect(response.status).toBe(500)
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
    'SECURITY NEGATIVE: an ANONYMOUS caller reading a NON-public scope %j is still 401',
    async params => {
      const response = await run({ request: get(params), userId: undefined })
      expect(response.status).toBe(401)
      const body = await syncErrorBody(response)
      expect(body.code).toBe('unauthenticated')
    },
  )

  test('SECURITY: a scope kind crafted to LOOK public ("scope.public_evil.x") does NOT grant anonymous access', async () => {
    const response = await run({
      request: get({ scope: 'scope.public_evil.x' }),
      userId: undefined,
    })
    expect(response.status).toBe(401)
  })

  test('an authenticated caller reading a public scope still passes their OWN userId through to resolveScopeRead', async () => {
    const served = page({ nextCursor: 1, scope: PUBLIC_SCOPE, upToDate: true })
    const seen: Array<string | undefined> = []
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ scope: PUBLIC_SCOPE }),
      resolveScopeRead: async (userId, scope) => {
        seen.push(userId)
        return defaultResolveScopeRead(userId, scope)
      },
    })
    expect(response.status).toBe(200)
    expect(seen).toEqual([USER_ID])
  })

  // ---------------------------------------------------------------------
  // CFG D1 evacuation (#8515): the actor-auth path can throw when its
  // D1-backed lookup is 401-dead. A public scope must degrade to anonymous
  // and still serve; a non-public scope must fail CLOSED with a typed 503.
  // ---------------------------------------------------------------------

  test('CFG #8515: a THROWING auth path on a PUBLIC scope degrades to anonymous and still serves 200', async () => {
    const served = page({ nextCursor: 1, scope: PUBLIC_SCOPE, upToDate: true })
    const response = await Effect.runPromise(
      handleKhalaSyncLog(get({ scope: PUBLIC_SCOPE }), {
        authenticate: async () => {
          throw new Error('d1-http bridge query failed (401)')
        },
        binding: { connectionString: FAKE_CONNECTION_STRING },
        hubNamespace: hubServing(served).namespace,
        resolveScopeRead: defaultResolveScopeRead,
      }),
    )
    expect(response.status).toBe(200)
  })

  test('CFG #8515: a THROWING auth path on a NON-public scope is a typed 503 (fail closed, retryable)', async () => {
    const response = await Effect.runPromise(
      handleKhalaSyncLog(get({ scope: OWN_SCOPE }), {
        authenticate: async () => {
          throw new Error('d1-http bridge query failed (401)')
        },
        binding: { connectionString: FAKE_CONNECTION_STRING },
        hubNamespace: undefined,
        resolveScopeRead: defaultResolveScopeRead,
      }),
    )
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('an anonymous read that fails the rate limiter is 429 rate_limited', async () => {
    const response = await run({
      anonymousRateLimit: () => false,
      request: get({ scope: PUBLIC_SCOPE }),
      userId: undefined,
    })
    expect(response.status).toBe(429)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('rate_limited')
    expect(body.retryable).toBe(true)
  })

  test('the anonymous rate limiter is NEVER consulted for an authenticated read, even of a public scope', async () => {
    const served = page({ nextCursor: 1, scope: PUBLIC_SCOPE, upToDate: true })
    let calls = 0
    const response = await run({
      anonymousRateLimit: () => {
        calls += 1
        return false
      },
      hubNamespace: hubServing(served).namespace,
      request: get({ scope: PUBLIC_SCOPE }),
    })
    expect(response.status).toBe(200)
    expect(calls).toBe(0)
  })
})

// ---------------------------------------------------------------------------
// KS-6.1 (#8302) fleet_run scopes via khala_sync_scope_owners — now resolved
// through the REAL KS-7.1 Worker resolver (makeKhalaSyncScopeReadResolver)
// wired into the route, over a fake sql client. D1 must never be consulted
// for fleet scopes.
// ---------------------------------------------------------------------------

describe('fleet_run scope read gate (KS-6.1 via the KS-7.1 resolver)', () => {
  const FLEET_SCOPE = scopeOf('scope.fleet_run.fleet-run.pylon.abc123')

  const untouchableD1 = new Proxy(
    {},
    {
      get: () => {
        throw new Error('D1 must not be consulted for fleet_run scopes')
      },
    },
  ) as D1Database

  /** A client whose sql answers ONLY the scope-owner SELECT. */
  const ownerLookupClient = (owner: string | null) => {
    let ended = 0
    const sql = ((
      strings: TemplateStringsArray,
      ..._values: ReadonlyArray<unknown>
    ) => {
      const text = strings.join('$')
      if (text.includes('khala_sync_scope_owners')) {
        return Promise.resolve(
          owner === null ? [] : [{ owner_user_id: owner }],
        )
      }
      return Promise.reject(
        new Error('only the scope-owner lookup may hit sql in these tests'),
      )
    }) as unknown as SyncSql
    const client: KhalaSyncPushSqlClient = {
      end: () => {
        ended += 1
        return Promise.resolve()
      },
      sql,
    }
    return { client, endedCount: () => ended }
  }

  const workerResolver = (fake: { client: KhalaSyncPushSqlClient }) =>
    makeKhalaSyncScopeReadResolver({
      binding: { connectionString: FAKE_CONNECTION_STRING },
      db: untouchableD1,
      makeSqlClient: async () => fake.client,
    })

  test('the scope OWNER reads an owned fleet scope (hub-served)', async () => {
    const served = page({ nextCursor: 2, scope: FLEET_SCOPE, upToDate: true })
    const fake = ownerLookupClient(USER_ID)
    const response = await run({
      hubNamespace: hubServing(served).namespace,
      request: get({ scope: FLEET_SCOPE }),
      resolveScopeRead: workerResolver(fake),
    })
    expect(response.status).toBe(200)
    // The owner-lookup client is always released.
    expect(fake.endedCount()).toBe(1)
  })

  test('a FOREIGN user gets 403 unauthorized_scope', async () => {
    const fake = ownerLookupClient('someone-else')
    const response = await run({
      request: get({ scope: FLEET_SCOPE }),
      resolveScopeRead: workerResolver(fake),
    })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unauthorized_scope')
    expect(fake.endedCount()).toBe(1)
  })

  test('an UNOWNED fleet scope is denied (fail-closed)', async () => {
    const fake = ownerLookupClient(null)
    const response = await run({
      request: get({ scope: FLEET_SCOPE }),
      resolveScopeRead: workerResolver(fake),
    })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unauthorized_scope')
  })

  test('binding absent ⇒ 503, never a grant', async () => {
    const fake = ownerLookupClient(USER_ID)
    const response = await run({
      request: get({ scope: FLEET_SCOPE }),
      resolveScopeRead: makeKhalaSyncScopeReadResolver({
        binding: undefined,
        db: untouchableD1,
        makeSqlClient: async () => fake.client,
      }),
    })
    expect(response.status).toBe(503)
    expect((await syncErrorBody(response)).code).toBe('storage_unavailable')
  })

  test('a failed owner lookup ⇒ 503 retryable, never a grant', async () => {
    const sql = (() =>
      Promise.reject(new Error('boom'))) as unknown as SyncSql
    const response = await run({
      request: get({ scope: FLEET_SCOPE }),
      resolveScopeRead: workerResolver({
        client: { end: () => Promise.resolve(), sql },
      }),
    })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })
})
