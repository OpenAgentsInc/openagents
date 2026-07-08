// Incident 2026-07-08: on the Cloud Run monolith every Khala Sync Postgres
// seam used to open a FRESH postgres.js client per statement and end it,
// blowing past Cloud Run's 100-connections-per-instance cap to khala-sync-pg.
// These tests pin the shared-pool contract that fixes it: on the server
// runtime one client is constructed per (connectionString, variant) and reused
// (with a no-op end), and on Workers the legacy fresh-per-acquire client with a
// real teardown is preserved.

import { afterEach, describe, expect, test, vi } from 'vitest'

import {
  __resetSharedPostgresPoolsForTests,
  __sharedPostgresPoolCountForTests,
  acquireSharedPostgresClient,
  resolveServerPoolMax,
} from './khala-sync-postgres-pool'

type FakeClient = {
  id: number
  optionsSeen: Record<string, unknown>
  end: ReturnType<typeof vi.fn>
}

const makeFactory = () => {
  let next = 0
  const created: Array<FakeClient> = []
  const createClient = (
    _connectionString: string,
    options: Record<string, unknown>,
  ): FakeClient => {
    const client: FakeClient = {
      end: vi.fn(async () => undefined),
      id: next++,
      optionsSeen: options,
    }
    created.push(client)
    return client
  }
  return { created, createClient }
}

afterEach(() => {
  __resetSharedPostgresPoolsForTests()
})

describe('acquireSharedPostgresClient (server runtime)', () => {
  test('reuses ONE pooled client across acquires for the same (dsn, variant)', async () => {
    const { created, createClient } = makeFactory()

    const a = await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: { prepare: false },
      runtime: 'server',
      variant: 'sync',
    })
    const b = await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: { prepare: false },
      runtime: 'server',
      variant: 'sync',
    })

    // Same underlying client instance — the pool is reused, not reconstructed.
    expect(a.sql).toBe(b.sql)
    expect(created).toHaveLength(1)
    expect(__sharedPostgresPoolCountForTests()).toBe(1)
  })

  test('end() is a no-op on the shared pool (never tears the pool down)', async () => {
    const { created, createClient } = makeFactory()

    const handle = await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: {},
      runtime: 'server',
      variant: 'sync',
    })
    await handle.end()

    expect(created[0]!.end).not.toHaveBeenCalled()
  })

  test('injects a pool `max` (not the legacy max:1)', async () => {
    const { created, createClient } = makeFactory()

    await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: { prepare: false },
      runtime: 'server',
      variant: 'sync',
    })

    expect(created[0]!.optionsSeen['max']).toBe(resolveServerPoolMax())
    expect(created[0]!.optionsSeen['max']).toBeGreaterThan(1)
    expect(created[0]!.optionsSeen['prepare']).toBe(false)
  })

  test('different variants against the same dsn get SEPARATE pools', async () => {
    const { created, createClient } = makeFactory()

    const sync = await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: {},
      runtime: 'server',
      variant: 'sync',
    })
    const d1 = await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: {},
      runtime: 'server',
      variant: 'd1-bigint',
    })

    expect(sync.sql).not.toBe(d1.sql)
    expect(created).toHaveLength(2)
    expect(__sharedPostgresPoolCountForTests()).toBe(2)
  })

  test('different dsns get separate pools', async () => {
    const { created, createClient } = makeFactory()

    await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db-a',
      createClient,
      options: {},
      runtime: 'server',
      variant: 'sync',
    })
    await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db-b',
      createClient,
      options: {},
      runtime: 'server',
      variant: 'sync',
    })

    expect(created).toHaveLength(2)
  })
})

describe('acquireSharedPostgresClient (workers runtime)', () => {
  test('constructs a FRESH max:1 client per acquire with a real teardown', async () => {
    const { created, createClient } = makeFactory()

    const a = await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: { prepare: false },
      runtime: 'workers',
      variant: 'sync',
    })
    const b = await acquireSharedPostgresClient<FakeClient>({
      connectionString: 'postgres://x/db',
      createClient,
      options: { prepare: false },
      runtime: 'workers',
      variant: 'sync',
    })

    // Fresh client each time (Workers cannot reuse sockets across requests).
    expect(a.sql).not.toBe(b.sql)
    expect(created).toHaveLength(2)
    expect(created[0]!.optionsSeen['max']).toBe(1)

    // end() really tears the per-acquire client down.
    await a.end()
    expect(created[0]!.end).toHaveBeenCalledTimes(1)

    // Workers acquires never populate the server pool cache.
    expect(__sharedPostgresPoolCountForTests()).toBe(0)
  })
})

describe('resolveServerPoolMax', () => {
  test('defaults to 10 when unset or invalid', () => {
    expect(resolveServerPoolMax({})).toBe(10)
    expect(resolveServerPoolMax({ KHALA_SYNC_PG_POOL_MAX: '' })).toBe(10)
    expect(resolveServerPoolMax({ KHALA_SYNC_PG_POOL_MAX: 'nope' })).toBe(10)
    expect(resolveServerPoolMax({ KHALA_SYNC_PG_POOL_MAX: '0' })).toBe(10)
  })

  test('honors a valid override', () => {
    expect(resolveServerPoolMax({ KHALA_SYNC_PG_POOL_MAX: '25' })).toBe(25)
    expect(resolveServerPoolMax({ KHALA_SYNC_PG_POOL_MAX: '7.9' })).toBe(7)
  })
})
