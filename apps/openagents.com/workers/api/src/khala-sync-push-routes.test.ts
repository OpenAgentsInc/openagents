import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationResult,
  MutationId,
  PushResponse,
} from '@openagentsinc/khala-sync'
import {
  KhalaSyncClientStateMismatchError,
  KhalaSyncStorageError,
  type MutatorRegistry,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  type ExecutePushFn,
  handleKhalaSyncPush,
  KHALA_SYNC_PUSH_PATH,
  KHALA_SYNC_PUSH_SUPPORTED_SCHEMA_VERSIONS,
  type KhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { makeKhalaSyncWorkerMutatorRegistry } from './khala-sync-mutators'

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@hyperdrive.local:5432/khala_sync_test'

const registry: MutatorRegistry = makeKhalaSyncWorkerMutatorRegistry()

const validBody = () => ({
  protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
  schemaVersion: 1,
  clientGroupId: 'cg-1',
  clientId: 'c-1',
  mutations: [
    {
      mutationId: 1,
      name: 'sync.debugEcho',
      argsJson: JSON.stringify({
        scope: 'scope.user.user-1',
        entityId: 'e-1',
        echo: 'hello',
      }),
    },
  ],
})

const post = (body: unknown) =>
  new Request(`https://openagents.com${KHALA_SYNC_PUSH_PATH}`, {
    body: typeof body === 'string' ? body : JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
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

const okEngine =
  (captured?: { input?: Parameters<ExecutePushFn>[0] }): ExecutePushFn =>
  async input => {
    if (captured !== undefined) {
      captured.input = input
    }
    return new PushResponse({
      lastMutationId: 1,
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      results: [
        new MutationResult({
          mutationId: MutationId.make(1),
          status: 'applied',
        }),
      ],
    })
  }

const run = (
  input: Readonly<{
    request?: Request
    userId?: string | undefined
    binding?: { connectionString: string } | undefined
    client?: KhalaSyncPushSqlClient
    engine?: ExecutePushFn
  }> = {},
) =>
  Effect.runPromise(
    handleKhalaSyncPush(input.request ?? post(validBody()), {
      authenticate: async () =>
        'userId' in input
          ? input.userId === undefined
            ? undefined
            : { userId: input.userId }
          : { userId: 'user-1' },
      binding:
        'binding' in input
          ? input.binding
          : { connectionString: FAKE_CONNECTION_STRING },
      executePush: input.engine ?? okEngine(),
      makeSqlClient: async () => input.client ?? makeFakeClient().client,
      registry,
    }),
  )

describe('handleKhalaSyncPush', () => {
  test('non-POST methods are 405', async () => {
    const response = await run({
      request: new Request(`https://openagents.com${KHALA_SYNC_PUSH_PATH}`),
    })
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('POST')
  })

  test('unauthenticated requests are 401 with a typed SyncError', async () => {
    const response = await run({ userId: undefined })
    expect(response.status).toBe(401)
    const body = (await response.json()) as Record<string, unknown>
    expect(body._tag).toBe('SyncError')
    expect(body.code).toBe('unauthenticated')
    expect(body.retryable).toBe(false)
  })

  test('non-JSON bodies are 400 invalid_request', async () => {
    const response = await run({ request: post('this is not json') })
    expect(response.status).toBe(400)
    const body = (await response.json()) as Record<string, unknown>
    expect(body._tag).toBe('SyncError')
    expect(body.code).toBe('invalid_request')
  })

  test('protocol version mismatch is 400 protocol_version_unsupported', async () => {
    const response = await run({
      request: post({ ...validBody(), protocolVersion: 2 }),
    })
    expect(response.status).toBe(400)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.code).toBe('protocol_version_unsupported')
    expect(body.retryable).toBe(false)
  })

  test('unsupported schema version is 400 schema_version_unsupported', async () => {
    const unsupported =
      Math.max(...KHALA_SYNC_PUSH_SUPPORTED_SCHEMA_VERSIONS) + 1
    const response = await run({
      request: post({ ...validBody(), schemaVersion: unsupported }),
    })
    expect(response.status).toBe(400)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.code).toBe('schema_version_unsupported')
  })

  test('a malformed PushRequest is 400 invalid_request without echoing values', async () => {
    const malformed = { ...validBody(), clientId: '' } as Record<
      string,
      unknown
    >
    malformed.mutations = [{ mutationId: 0, name: 'BAD NAME', argsJson: 7 }]
    const response = await run({ request: post(malformed) })
    expect(response.status).toBe(400)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.code).toBe('invalid_request')
    expect(JSON.stringify(body)).not.toContain('BAD NAME')
  })

  test('absent Hyperdrive binding is 503 storage_unavailable (retryable)', async () => {
    const response = await run({ binding: undefined })
    expect(response.status).toBe(503)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
  })

  test('happy path: decodes, authenticates, executes, encodes PushResponse, ends the client', async () => {
    const fake = makeFakeClient()
    const captured: { input?: Parameters<ExecutePushFn>[0] } = {}
    const response = await run({
      client: fake.client,
      engine: okEngine(captured),
      userId: 'user-42',
    })
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as {
      protocolVersion: number
      results: Array<{ mutationId: number; status: string }>
      lastMutationId: number
    }
    expect(body.protocolVersion).toBe(KHALA_SYNC_PROTOCOL_VERSION)
    expect(body.results).toEqual([{ mutationId: 1, status: 'applied' }])
    expect(body.lastMutationId).toBe(1)

    // The engine received the AUTHENTICATED user (never the body) and the
    // decoded request.
    expect(captured.input?.userId).toBe('user-42')
    expect(String(captured.input?.request.clientGroupId)).toBe('cg-1')
    expect(captured.input?.request.mutations).toHaveLength(1)
    expect(captured.input?.sql).toBe(fake.client.sql)
    expect(fake.endedCount()).toBe(1)
  })

  test('client-group/user mismatch is a whole-request 403 unauthorized_scope', async () => {
    const fake = makeFakeClient()
    const response = await run({
      client: fake.client,
      engine: async () => {
        throw new KhalaSyncClientStateMismatchError(
          'cg-1' as never,
          'other-user',
          'user-1',
        )
      },
    })
    expect(response.status).toBe(403)
    const body = (await response.json()) as Record<string, unknown>
    expect(body._tag).toBe('SyncError')
    expect(body.code).toBe('unauthorized_scope')
    expect(body.retryable).toBe(false)
    expect(fake.endedCount()).toBe(1)
  })

  test('storage failures are 503 storage_unavailable (retryable), client still ended', async () => {
    const fake = makeFakeClient()
    const response = await run({
      client: fake.client,
      engine: async () => {
        throw new KhalaSyncStorageError('unavailable', 'boom')
      },
    })
    expect(response.status).toBe(503)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.code).toBe('storage_unavailable')
    expect(body.retryable).toBe(true)
    expect(fake.endedCount()).toBe(1)
  })

  test('unexpected engine failures are 500 internal', async () => {
    const response = await run({
      engine: async () => {
        throw new Error('surprise')
      },
    })
    expect(response.status).toBe(500)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.code).toBe('internal')
    expect(JSON.stringify(body)).not.toContain('surprise')
  })
})
