// Route tests for POST /api/sync/cvr-pull (KS-7.2, #8306): the flag gate
// (KHALA_SYNC_CVR unset/≠'1' ⇒ 404 — indistinguishable from an
// unregistered route), auth guard, version gates, body decode, the KS-7.1
// scope gate, request→service input mapping (cvrVersion/drift), typed
// SyncError mapping (row-set-too-large → 400, storage → 503, unexpected →
// 500), no-store cache posture, and client teardown. All seams are
// injected — no network, no database.

import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CvrDel,
  CvrPullResponse,
  CvrVersion,
  decodeCvrPullResponse,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  personalScope,
  SyncScope,
  SyncVersionWatermark,
} from '@openagentsinc/khala-sync'
import {
  type CvrPullInput,
  KhalaSyncCvrRowSetTooLargeError,
  KhalaSyncStorageError,
  resolveScopeRead,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  type CvrPullFromPostgresFn,
  handleKhalaSyncCvrPull,
  isKhalaSyncCvrEnabled,
  KHALA_SYNC_CVR_PULL_PATH,
  type KhalaSyncCvrPullDependencies,
} from './khala-sync-cvr-routes'
import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import type { KhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@hyperdrive.local:5432/khala_sync_test'

const USER_ID = 'user-1'
const OWN_SCOPE = personalScope(USER_ID)

const requestBody = (overrides: Record<string, unknown> = {}): unknown => ({
  clientGroupId: 'cg-1',
  protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
  schemaVersion: 1,
  scope: OWN_SCOPE,
  ...overrides,
})

const post = (body: unknown): Request =>
  new Request(`https://openagents.com${KHALA_SYNC_CVR_PULL_PATH}`, {
    body: typeof body === 'string' ? body : JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const emptyResponse = (scope: SyncScope): CvrPullResponse =>
  new CvrPullResponse({
    cursor: SyncVersionWatermark.make(0),
    cvrVersion: CvrVersion.make(1),
    dels: [],
    mode: 'reset',
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    puts: [],
    scope,
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

/** The REAL KS-7.1 resolver over deterministic in-test capabilities. */
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
    enabled?: boolean
    userId?: string | undefined
    binding?: { connectionString: string } | undefined
    client?: KhalaSyncPushSqlClient
    cvrPull?: CvrPullFromPostgresFn
    resolveScopeRead?: KhalaSyncScopeReadResolver
  }> = {},
) => {
  const deps: KhalaSyncCvrPullDependencies = {
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
    cvrPullFromPostgres:
      input.cvrPull ??
      (async () => {
        throw new Error('cvrPullFromPostgres must be injected for this test')
      }),
    enabled: input.enabled ?? true,
    makeSqlClient: async () => input.client ?? makeFakeClient().client,
    resolveScopeRead: input.resolveScopeRead ?? defaultResolveScopeRead,
  }
  return Effect.runPromise(
    handleKhalaSyncCvrPull(input.request ?? post(requestBody()), deps),
  )
}

const syncErrorBody = async (response: Response) =>
  (await response.json()) as {
    code: string
    messageSafe: string
    retryable: boolean
  }

describe('isKhalaSyncCvrEnabled', () => {
  test("only the literal '1' turns the surface on", () => {
    expect(isKhalaSyncCvrEnabled('1')).toBe(true)
    expect(isKhalaSyncCvrEnabled(undefined)).toBe(false)
    expect(isKhalaSyncCvrEnabled('0')).toBe(false)
    expect(isKhalaSyncCvrEnabled('true')).toBe(false)
  })
})

describe('handleKhalaSyncCvrPull', () => {
  test('flag OFF: 404 for every request — even valid authenticated ones (zero behavior change)', async () => {
    const response = await run({
      cvrPull: async () => emptyResponse(OWN_SCOPE),
      enabled: false,
    })
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({ error: 'not_found' })
  })

  test('flag OFF wins over the method guard (no 405 leak reveals the route)', async () => {
    const response = await run({
      enabled: false,
      request: new Request(
        `https://openagents.com${KHALA_SYNC_CVR_PULL_PATH}`,
        { method: 'GET' },
      ),
    })
    expect(response.status).toBe(404)
  })

  test('non-POST methods are 405', async () => {
    const response = await run({
      request: new Request(
        `https://openagents.com${KHALA_SYNC_CVR_PULL_PATH}`,
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
    expect((await syncErrorBody(response)).code).toBe(
      'protocol_version_unsupported',
    )
  })

  test.each<Record<string, unknown>>([
    { scope: 'not-a-scope' },
    { clientGroupId: '' },
    { cvrVersion: 0 },
    { cvrVersion: 1.5 },
    { drift: [{ entityId: 'x', entityType: 'thing' }] }, // missing version
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
    expect((await syncErrorBody(response)).code).toBe(
      'schema_version_unsupported',
    )
  })

  test("another user's personal scope is 403 unauthorized_scope", async () => {
    const response = await run({
      request: post(requestBody({ scope: personalScope('user-2') })),
    })
    expect(response.status).toBe(403)
    expect((await syncErrorBody(response)).code).toBe('unauthorized_scope')
  })

  test('an absent KHALA_SYNC_DB binding is 503 storage_unavailable', async () => {
    const response = await run({
      binding: undefined,
      cvrPull: async () => emptyResponse(OWN_SCOPE),
    })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('happy path: request fields map to the service input; the response encodes; the client is ended', async () => {
    const fake = makeFakeClient()
    const inputs: Array<CvrPullInput> = []
    const scope = S.decodeUnknownSync(SyncScope)('scope.team.t-1')
    const response = await run({
      client: fake.client,
      cvrPull: async (_sql, input) => {
        inputs.push(input)
        return new CvrPullResponse({
          cursor: SyncVersionWatermark.make(41),
          cvrVersion: CvrVersion.make(7),
          dels: [
            new CvrDel({
              entityId: EntityId.make('gone'),
              entityType: EntityType.make('thing'),
            }),
          ],
          mode: 'diff',
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          puts: [],
          scope,
        })
      },
      request: post(
        requestBody({
          cvrVersion: 6,
          drift: [{ entityId: 'w', entityType: 'thing', version: 40 }],
          scope,
        }),
      ),
      resolveScopeRead: testResolver({ memberOfTeams: new Set(['t-1']) }),
    })
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const decoded = decodeCvrPullResponse(await response.json())
    expect(decoded.mode).toBe('diff')
    expect(Number(decoded.cvrVersion)).toBe(7)
    expect(decoded.dels).toHaveLength(1)
    expect(inputs).toHaveLength(1)
    expect(inputs[0]).toMatchObject({
      clientGroupId: 'cg-1',
      cvrVersion: 6,
      drift: [{ entityId: 'w', entityType: 'thing', version: 40 }],
      scope,
    })
    expect(fake.endedCount()).toBe(1)
  })

  test('an absent cvrVersion maps to null (reset-mode pull)', async () => {
    const inputs: Array<CvrPullInput> = []
    const response = await run({
      cvrPull: async (_sql, input) => {
        inputs.push(input)
        return emptyResponse(OWN_SCOPE)
      },
    })
    expect(response.status).toBe(200)
    expect(inputs[0]!.cvrVersion).toBeNull()
    expect(inputs[0]!.drift).toEqual([])
  })

  test('a too-large row set is 400 invalid_request (fall back to the paged bootstrap) and still ends the client', async () => {
    const fake = makeFakeClient()
    const response = await run({
      client: fake.client,
      cvrPull: async () => {
        throw new KhalaSyncCvrRowSetTooLargeError(String(OWN_SCOPE), 50_000)
      },
    })
    expect(response.status).toBe(400)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('invalid_request')
    expect(body.retryable).toBe(false)
    expect(fake.endedCount()).toBe(1)
  })

  test('a storage failure is 503 storage_unavailable (retryable) and still ends the client', async () => {
    const fake = makeFakeClient()
    const response = await run({
      client: fake.client,
      cvrPull: async () => {
        throw new KhalaSyncStorageError('connection_failed', 'pg down')
      },
    })
    expect(response.status).toBe(503)
    const body = await syncErrorBody(response)
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
    expect(fake.endedCount()).toBe(1)
  })

  test('an unexpected failure is 500 internal and still ends the client', async () => {
    const fake = makeFakeClient()
    const response = await run({
      client: fake.client,
      cvrPull: async () => {
        throw new Error('boom')
      },
    })
    expect(response.status).toBe(500)
    expect((await syncErrorBody(response)).code).toBe('internal')
    expect(fake.endedCount()).toBe(1)
  })
})
