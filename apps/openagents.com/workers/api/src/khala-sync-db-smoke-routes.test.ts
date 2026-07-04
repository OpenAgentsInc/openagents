import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  KHALA_SYNC_DB_SMOKE_ROUTE_REF,
  KHALA_SYNC_DB_SMOKE_SELECT_ONE,
  KHALA_SYNC_DB_SMOKE_TABLE_COUNT,
  KHALA_SYNC_TABLE_PREFIX_PATTERN,
  type KhalaSyncSmokeSqlClient,
  handleKhalaSyncDbSmoke,
  redactConnectionDetails,
} from './khala-sync-db-smoke-routes'

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@hyperdrive.local:5432/khala_sync_test'

const get = () =>
  new Request('https://openagents.com/api/internal/khala-sync/db-smoke')

type FakeClientScript = Readonly<{
  okRows?: ReadonlyArray<Record<string, unknown>>
  tableRows?: ReadonlyArray<Record<string, unknown>>
  queryError?: Error
  endError?: Error
}>

const makeFakeClient = (script: FakeClientScript = {}) => {
  const calls: Array<{ text: string; params: ReadonlyArray<string> }> = []
  let ended = 0

  const client: KhalaSyncSmokeSqlClient = {
    end: () => {
      ended += 1
      return script.endError === undefined
        ? Promise.resolve()
        : Promise.reject(script.endError)
    },
    query: (text, params) => {
      calls.push({ params, text })
      if (script.queryError !== undefined) {
        return Promise.reject(script.queryError)
      }
      if (text === KHALA_SYNC_DB_SMOKE_SELECT_ONE) {
        return Promise.resolve(script.okRows ?? [{ ok: 1 }])
      }
      return Promise.resolve(script.tableRows ?? [{ khala_sync_tables: 4 }])
    },
  }

  return {
    calls,
    client,
    endedCount: () => ended,
  }
}

const run = (
  input: Readonly<{
    authorized?: boolean
    binding?: { connectionString: string } | undefined
    client?: KhalaSyncSmokeSqlClient
    factoryError?: Error
    request?: Request
    nowMs?: () => number
  }> = {},
) =>
  Effect.runPromise(
    handleKhalaSyncDbSmoke(input.request ?? get(), {
      binding:
        'binding' in input
          ? input.binding
          : { connectionString: FAKE_CONNECTION_STRING },
      makeSqlClient: () =>
        input.factoryError !== undefined
          ? Promise.reject(input.factoryError)
          : Promise.resolve(input.client ?? makeFakeClient().client),
      nowMs: input.nowMs,
      requireOperator: async () => input.authorized ?? true,
    }),
  )

describe('handleKhalaSyncDbSmoke', () => {
  test('rejects non-GET methods', async () => {
    const response = await run({
      request: new Request(
        'https://openagents.com/api/internal/khala-sync/db-smoke',
        { method: 'POST' },
      ),
    })

    expect(response.status).toBe(405)
    expect(await response.json()).toMatchObject({
      error: 'method_not_allowed',
    })
  })

  test('requires the admin bearer guard', async () => {
    const response = await run({ authorized: false })

    expect(response.status).toBe(401)
    expect(await response.json()).toMatchObject({ error: 'unauthorized' })
  })

  test('is honest when the Hyperdrive binding is absent', async () => {
    const response = await run({ binding: undefined })

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ok: boolean
      reason: string
      routeRef: string
    }
    expect(body.ok).toBe(false)
    expect(body.reason).toContain('KHALA_SYNC_DB')
    expect(body.routeRef).toBe(KHALA_SYNC_DB_SMOKE_ROUTE_REF)
  })

  test('runs both bounded statements and returns the smoke payload', async () => {
    const fake = makeFakeClient({ tableRows: [{ khala_sync_tables: 4 }] })
    let tick = 0
    const response = await run({
      client: fake.client,
      nowMs: () => {
        tick += 12
        return tick
      },
    })

    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({
      khalaSyncTables: 4,
      latencyMs: expect.any(Number),
      ok: true,
      routeRef: KHALA_SYNC_DB_SMOKE_ROUTE_REF,
    })
    expect(response.headers.get('cache-control')).toBe('no-store')

    // Exactly the two allowed single statements, in order, with the table
    // count parameterized (transaction-mode-safe; no session state).
    expect(fake.calls).toEqual([
      { params: [], text: KHALA_SYNC_DB_SMOKE_SELECT_ONE },
      {
        params: [KHALA_SYNC_TABLE_PREFIX_PATTERN],
        text: KHALA_SYNC_DB_SMOKE_TABLE_COUNT,
      },
    ])

    // Connection is always released.
    expect(fake.endedCount()).toBe(1)
  })

  test('accepts a string count from the driver (bigint text mode)', async () => {
    const fake = makeFakeClient({ tableRows: [{ khala_sync_tables: '7' }] })
    const response = await run({ client: fake.client })

    expect(response.status).toBe(200)
    expect(await response.json()).toMatchObject({
      khalaSyncTables: 7,
      ok: true,
    })
  })

  test('maps an unexpected SELECT 1 result to a 503 without leaking details', async () => {
    const fake = makeFakeClient({ okRows: [{ ok: 0 }] })
    const response = await run({ client: fake.client })

    expect(response.status).toBe(503)
    expect(await response.json()).toMatchObject({
      error: 'khala_sync_db_smoke_failed',
      ok: false,
    })
    expect(fake.endedCount()).toBe(1)
  })

  test('maps a malformed table-count row to a 503', async () => {
    const fake = makeFakeClient({ tableRows: [{}] })
    const response = await run({ client: fake.client })

    expect(response.status).toBe(503)
    expect(await response.json()).toMatchObject({
      error: 'khala_sync_db_smoke_failed',
      ok: false,
    })
    expect(fake.endedCount()).toBe(1)
  })

  test('redacts connection details from driver errors and still ends the client', async () => {
    const fake = makeFakeClient({
      queryError: new Error(
        'connect ECONNREFUSED 10.11.12.13:5432 via postgresql://user:pw@10.11.12.13:5432/db',
      ),
    })
    const response = await run({ client: fake.client })

    expect(response.status).toBe(503)
    const body = (await response.json()) as { reason: string }
    expect(body.reason).not.toContain('10.11.12.13')
    expect(body.reason).not.toContain('user:pw')
    expect(body.reason).toContain('[redacted-address]')
    expect(fake.endedCount()).toBe(1)
  })

  test('maps a client factory failure to a redacted 503', async () => {
    const response = await run({
      factoryError: new Error('no such host at 34.1.2.3'),
    })

    expect(response.status).toBe(503)
    const body = (await response.json()) as { ok: boolean; reason: string }
    expect(body.ok).toBe(false)
    expect(body.reason).not.toContain('34.1.2.3')
  })

  test('an end() failure never masks a successful smoke', async () => {
    const fake = makeFakeClient({ endError: new Error('socket already gone') })
    const response = await run({ client: fake.client })

    expect(response.status).toBe(200)
    expect(await response.json()).toMatchObject({ ok: true })
  })
})

describe('redactConnectionDetails', () => {
  test('strips DSNs, IPv4, IPv6, and host:port material', () => {
    const redacted = redactConnectionDetails(
      'failed postgres://a:b@c:5432/d then 192.168.0.1:5432 then 2001:db8::1 end',
    )

    expect(redacted).not.toContain('a:b')
    expect(redacted).not.toContain('192.168.0.1')
    expect(redacted).not.toContain('2001:db8::1')
  })

  test('bounds the message length', () => {
    expect(redactConnectionDetails('x'.repeat(1000)).length).toBe(300)
  })
})
