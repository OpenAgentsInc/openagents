// Route tests for POST /api/sync/bootstrap (KS-4.4, #8297): auth guard,
// version gates, body decode, v1 scope gate, page flow (pageSize bounding,
// pageToken passthrough, nextPageToken/cursor page shapes), typed SyncError
// mapping (invalid page token → 400, behind-window → 410, storage → 503,
// unexpected → 500), no-store cache posture, and client teardown. All seams
// are injected — no network, no database.

import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BootstrapEntity,
  BootstrapResponse,
  decodeBootstrapResponse,
  KHALA_SYNC_PROTOCOL_VERSION,
  personalScope,
  publicScope,
  SyncScope,
  SyncVersionWatermark,
} from '@openagentsinc/khala-sync'
import {
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncInvalidPageTokenError,
  KhalaSyncStorageError,
  MAX_BOOTSTRAP_PAGE_SIZE,
  resolveScopeRead,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  type BootstrapFromPostgresFn,
  handleKhalaSyncBootstrap,
  KHALA_SYNC_BOOTSTRAP_PATH,
  type KhalaSyncBootstrapDependencies,
} from './khala-sync-bootstrap-routes'
import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import type { KhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

const decodeBootstrapEntity = S.decodeUnknownSync(BootstrapEntity)

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@hyperdrive.local:5432/khala_sync_test'

const USER_ID = 'user-1'
const OWN_SCOPE = personalScope(USER_ID)
const PUBLIC_SCOPE = publicScope('artanis.global')

const requestBody = (overrides: Record<string, unknown> = {}): unknown => ({
  clientGroupId: 'cg-1',
  protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
  schemaVersion: 1,
  scope: OWN_SCOPE,
  ...overrides,
})

const post = (body: unknown): Request =>
  new Request(`https://openagents.com${KHALA_SYNC_BOOTSTRAP_PATH}`, {
    body: typeof body === 'string' ? body : JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const entity = (id: string) =>
  decodeBootstrapEntity({
    entityId: id,
    entityType: 'note',
    postImageJson: JSON.stringify({ id }),
  })

const page = (
  input: Readonly<{
    entityIds?: ReadonlyArray<string>
    cursor?: number
    nextPageToken?: string
  }>,
) =>
  new BootstrapResponse({
    entities: (input.entityIds ?? []).map(entity),
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    scope: OWN_SCOPE,
    ...(input.nextPageToken !== undefined
      ? { nextPageToken: input.nextPageToken }
      : { cursor: SyncVersionWatermark.make(input.cursor ?? 0) }),
  })

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

/**
 * The REAL package resolver (KS-7.1) over deterministic in-test
 * capabilities: own personal scope + public scopes, plus any teams in
 * `memberOfTeams`. Same shape as the log-route suite.
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
    binding?: { connectionString: string } | undefined
    client?: KhalaSyncPushSqlClient
    bootstrap?: BootstrapFromPostgresFn
    resolveScopeRead?: KhalaSyncScopeReadResolver
    anonymousRateLimit?: (request: Request) => boolean
  }> = {},
) => {
  const deps: KhalaSyncBootstrapDependencies = {
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
    bootstrapFromPostgres:
      input.bootstrap ??
      (async () => {
        throw new Error('bootstrapFromPostgres must be injected for this test')
      }),
    makeSqlClient: async () => input.client ?? makeFakeClient().client,
    resolveScopeRead: input.resolveScopeRead ?? defaultResolveScopeRead,
  }
  return Effect.runPromise(
    handleKhalaSyncBootstrap(input.request ?? post(requestBody()), deps),
  )
}

const syncErrorBody = async (response: Response) =>
  (await response.json()) as {
    code: string
    messageSafe: string
    retryable: boolean
  }

describe('handleKhalaSyncBootstrap', () => {
  test('non-POST methods are 405', async () => {
    const response = await run({
      request: new Request(
        `https://openagents.com${KHALA_SYNC_BOOTSTRAP_PATH}`,
        { method: 'GET' },
      ),
    })
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('POST')
  })

  test('unauthenticated requests are 401 typed SyncError', async () => {
    const response = await run({ userId: undefined })
    expect(response.status).toBe(401)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('unauthenticated')
    expect(body.retryable).toBe(false)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('a non-JSON body is 400 invalid_request', async () => {
    const response = await run({ request: post('{nope') })
    expect(response.status).toBe(400)
    expect((await syncErrorBody(response)).code).toBe('invalid_request')
  })

  test('a wrong protocol version is 400 protocol_version_unsupported', async () => {
    const response = await run({
      request: post(requestBody({ protocolVersion: 2 })),
    })
    expect(response.status).toBe(400)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('protocol_version_unsupported')
    expect(body.retryable).toBe(false)
  })

  test.each<Record<string, unknown>>([
    { scope: 'not-a-scope' },
    { clientGroupId: '' },
    { pageSize: 0 },
    { pageSize: 1.5 },
    { pageToken: 42 },
    { schemaVersion: 'x' },
  ])('undecodable body override %j is 400 invalid_request', async override => {
    const response = await run({ request: post(requestBody(override)) })
    expect(response.status).toBe(400)
    expect((await syncErrorBody(response)).code).toBe('invalid_request')
  })

  test('an unsupported schema version is 400 schema_version_unsupported', async () => {
    const response = await run({
      request: post(requestBody({ schemaVersion: 99 })),
    })
    expect(response.status).toBe(400)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('schema_version_unsupported')
    expect(body.retryable).toBe(false)
  })

  test("another user's personal scope is 403 unauthorized_scope", async () => {
    const response = await run({
      request: post(requestBody({ scope: personalScope('user-2') })),
    })
    expect(response.status).toBe(403)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('unauthorized_scope')
    expect(body.retryable).toBe(false)
  })

  test('a NON-MEMBER is denied a team scope (403 unauthorized_scope)', async () => {
    const response = await run({
      request: post(requestBody({ scope: 'scope.team.t-1' })),
    })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unauthorized_scope')
  })

  test('a LIVE team member bootstraps the team scope', async () => {
    const response = await run({
      bootstrap: async () =>
        new BootstrapResponse({
          cursor: SyncVersionWatermark.make(0),
          entities: [],
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: S.decodeUnknownSync(SyncScope)('scope.team.t-1'),
        }),
      request: post(requestBody({ scope: 'scope.team.t-1' })),
      resolveScopeRead: testResolver({ memberOfTeams: new Set(['t-1']) }),
    })
    expect(response.status).toBe(200)
  })

  test('an unknown scope taxonomy member is gated CLOSED (403 unknown_scope)', async () => {
    const response = await run({
      request: post(requestBody({ scope: 'scope.workspace.w-1' })),
    })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unknown_scope')
  })

  test('a failed authorization lookup fails CLOSED as 503 retryable', async () => {
    const response = await run({
      request: post(requestBody({ scope: 'scope.team.t-1' })),
      resolveScopeRead: async () => ({
        kind: 'unavailable',
        messageSafe: 'authorization lookup failed; retry the request.',
      }),
    })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('missing KHALA_SYNC_DB binding is 503 storage_unavailable', async () => {
    const response = await run({ binding: undefined })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Page flow
  // -------------------------------------------------------------------------

  test('a first page passes scope + bounded pageSize and no token to the read', async () => {
    const captured: Array<unknown> = []
    const fake = makeFakeClient()
    const response = await run({
      bootstrap: async (sql, input) => {
        captured.push(input)
        expect(sql).toBe(fakeSqlHandle)
        return page({ entityIds: ['a'], nextPageToken: 'token-1' })
      },
      client: fake.client,
      request: post(requestBody({ pageSize: 25 })),
    })
    expect(response.status).toBe(200)
    expect(captured).toEqual([
      { pageSize: 25, pageToken: undefined, scope: OWN_SCOPE },
    ])
    const body = decodeBootstrapResponse(await response.json())
    expect(body.entities.map(e => String(e.entityId))).toEqual(['a'])
    expect(body.nextPageToken).toBe('token-1')
    expect(body.cursor).toBeUndefined()
    expect(fake.endedCount()).toBe(1)
  })

  test('a follow-up page forwards the pageToken; the final page carries cursor', async () => {
    const captured: Array<unknown> = []
    const response = await run({
      bootstrap: async (_sql, input) => {
        captured.push(input)
        return page({ cursor: 7, entityIds: ['z'] })
      },
      request: post(requestBody({ pageToken: 'token-1' })),
    })
    expect(response.status).toBe(200)
    expect(captured).toEqual([
      {
        pageSize: MAX_BOOTSTRAP_PAGE_SIZE,
        pageToken: 'token-1',
        scope: OWN_SCOPE,
      },
    ])
    const body = decodeBootstrapResponse(await response.json())
    expect(Number(body.cursor)).toBe(7)
    expect(body.nextPageToken).toBeUndefined()
  })

  test('a requested pageSize above the package max is clamped', async () => {
    const captured: Array<{ pageSize?: number | undefined }> = []
    await run({
      bootstrap: async (_sql, input) => {
        captured.push(input)
        return page({ cursor: 0 })
      },
      request: post(requestBody({ pageSize: 999_999 })),
    })
    expect(captured[0]?.pageSize).toBe(MAX_BOOTSTRAP_PAGE_SIZE)
  })

  test('every bootstrap page is no-store (paging-position-specific)', async () => {
    const response = await run({
      bootstrap: async () => page({ cursor: 3, entityIds: ['a'] }),
    })
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('etag')).toBeNull()
  })

  test('the public scope family is bootstrappable by any authenticated user', async () => {
    const response = await run({
      bootstrap: async () =>
        new BootstrapResponse({
          cursor: SyncVersionWatermark.make(0),
          entities: [],
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: PUBLIC_SCOPE,
        }),
      request: post(requestBody({ scope: PUBLIC_SCOPE })),
    })
    expect(response.status).toBe(200)
  })

  // -------------------------------------------------------------------------
  // KS-8.x anonymous-read exception: scope.public.* is bootstrappable
  // WITHOUT an authenticated actor; every other scope kind still 401s.
  // -------------------------------------------------------------------------

  test('POSITIVE: an ANONYMOUS caller (no session/token) can bootstrap a public scope', async () => {
    const response = await run({
      bootstrap: async () =>
        new BootstrapResponse({
          cursor: SyncVersionWatermark.make(0),
          entities: [],
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: PUBLIC_SCOPE,
        }),
      request: post(requestBody({ scope: PUBLIC_SCOPE })),
      userId: undefined,
    })
    expect(response.status).toBe(200)
  })

  test.each<Record<string, unknown>>([
    { scope: OWN_SCOPE },
    { scope: personalScope('user-2') },
    { scope: 'scope.team.t-1' },
    { scope: 'scope.agent_run.run-1' },
    { scope: 'scope.thread.thread-1' },
    { scope: 'scope.fleet_run.fleet-1' },
    { scope: 'scope.workspace.w-1' },
  ])(
    'SECURITY NEGATIVE: an ANONYMOUS caller bootstrapping a NON-public scope %j is still 401',
    async override => {
      const response = await run({
        request: post(requestBody(override)),
        userId: undefined,
      })
      expect(response.status).toBe(401)
      const body = await syncErrorBody(response)
      expect(body.code).toBe('unauthenticated')
    },
  )

  test('SECURITY: a scope kind crafted to LOOK public ("scope.public_evil.x") does NOT grant anonymous access', async () => {
    const response = await run({
      request: post(requestBody({ scope: 'scope.public_evil.x' })),
      userId: undefined,
    })
    expect(response.status).toBe(401)
  })

  test('an authenticated caller bootstrapping a public scope still passes their OWN userId through to resolveScopeRead', async () => {
    const seen: Array<string | undefined> = []
    const response = await run({
      bootstrap: async () =>
        new BootstrapResponse({
          cursor: SyncVersionWatermark.make(0),
          entities: [],
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: PUBLIC_SCOPE,
        }),
      request: post(requestBody({ scope: PUBLIC_SCOPE })),
      resolveScopeRead: async (userId, scope) => {
        seen.push(userId)
        return defaultResolveScopeRead(userId, scope)
      },
    })
    expect(response.status).toBe(200)
    expect(seen).toEqual([USER_ID])
  })

  test('an anonymous bootstrap that fails the rate limiter is 429 rate_limited', async () => {
    const response = await run({
      anonymousRateLimit: () => false,
      request: post(requestBody({ scope: PUBLIC_SCOPE })),
      userId: undefined,
    })
    expect(response.status).toBe(429)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('rate_limited')
    expect(body.retryable).toBe(true)
  })

  test('the anonymous rate limiter is NEVER consulted for an authenticated bootstrap, even of a public scope', async () => {
    let calls = 0
    const response = await run({
      anonymousRateLimit: () => {
        calls += 1
        return false
      },
      bootstrap: async () =>
        new BootstrapResponse({
          cursor: SyncVersionWatermark.make(0),
          entities: [],
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: PUBLIC_SCOPE,
        }),
      request: post(requestBody({ scope: PUBLIC_SCOPE })),
    })
    expect(response.status).toBe(200)
    expect(calls).toBe(0)
  })

  // -------------------------------------------------------------------------
  // Error mapping
  // -------------------------------------------------------------------------

  test('an invalid page token is 400 invalid_request (restart tokenless)', async () => {
    const fake = makeFakeClient()
    const response = await run({
      bootstrap: async () => {
        throw new KhalaSyncInvalidPageTokenError('token belongs to another scope')
      },
      client: fake.client,
      request: post(requestBody({ pageToken: 'stale' })),
    })
    expect(response.status).toBe(400)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('invalid_request')
    expect(body.retryable).toBe(false)
    expect(fake.endedCount()).toBe(1)
  })

  test('a snapshot behind the retained window is a 410 MustRefetch SyncError', async () => {
    const response = await run({
      bootstrap: async () => {
        throw new KhalaSyncCursorBehindRetainedWindowError(OWN_SCOPE, 3, 50)
      },
    })
    expect(response.status).toBe(410)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('cursor_behind_retained_window')
    expect(body.retryable).toBe(false)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('a storage failure is 503 retryable', async () => {
    const response = await run({
      bootstrap: async () => {
        throw new KhalaSyncStorageError('connection_failed', 'no route to db')
      },
    })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('an unexpected failure is 500 internal', async () => {
    const response = await run({
      bootstrap: async () => {
        throw new Error('boom')
      },
    })
    expect(response.status).toBe(500)
    expect((await syncErrorBody(response)).code).toBe('internal')
  })

  test('the sql client is torn down even when the read throws', async () => {
    const fake = makeFakeClient()
    await run({
      bootstrap: async () => {
        throw new Error('boom')
      },
      client: fake.client,
    })
    expect(fake.endedCount()).toBe(1)
  })
})
