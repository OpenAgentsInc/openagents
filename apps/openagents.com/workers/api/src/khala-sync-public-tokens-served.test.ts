// KS-6.3 (#8304): unit tests for the Worker-side public tokens-served
// projection seams — fail-soft producer, cached reader, and reconcile.
// These drive the REAL @openagentsinc/khala-sync-server projection code
// over a scripted fake postgres.js-shaped client (pattern-matched tagged
// templates), so CI needs no database; the full Postgres integration lives
// in packages/khala-sync-server/src/public-counter-projection.test.ts.

import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'
import { beforeEach, describe, expect, test } from 'vitest'

import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import {
  readTokensServedProjectionCached,
  reconcileTokensServedProjection,
  recordTokensServedProjectionBestEffort,
  resetTokensServedProjectionCacheForTests,
  type TokensServedProjectionLog,
} from './khala-sync-public-tokens-served'

const observedAt = '2026-07-04T12:00:00.000Z'

beforeEach(() => {
  resetTokensServedProjectionCacheForTests()
})

// ---------------------------------------------------------------------------
// Scripted fake Postgres (pattern-matched tagged-template SQL)
// ---------------------------------------------------------------------------

type FakePgState = {
  /** khala_sync_public_counters row, or null before the backfill. */
  counter: { total: number; lastEventAt: string | null } | null
  /** applied idempotency keys (khala_sync_counter_applied). */
  applied: Set<string>
  /** scope version counter (khala_sync_scopes). */
  lastVersion: number
  changelogAppends: Array<{ postImageJson: string | null }>
  repairs: Array<{ previousTotal: number | null; newTotal: number; source: string; auditNote: string }>
}

const makeFakePg = (
  initial?: Partial<Pick<FakePgState, 'counter'>>,
): { state: FakePgState; sql: SyncSql } => {
  const state: FakePgState = {
    applied: new Set(),
    changelogAppends: [],
    counter: initial?.counter ?? null,
    lastVersion: 0,
    repairs: [],
  }

  const run = async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): Promise<any> => {
    const text = strings.join('?')
    if (text.includes('INSERT INTO khala_sync_counter_applied')) {
      const key = String(values[1])
      if (state.applied.has(key)) return []
      state.applied.add(key)
      return [{ idempotency_key: key }]
    }
    if (text.includes('UPDATE khala_sync_public_counters')) {
      if (state.counter === null) return []
      const delta = Number(values[0])
      const eventAt = String(values[1])
      state.counter = {
        lastEventAt:
          state.counter.lastEventAt === null ||
          eventAt > state.counter.lastEventAt
            ? eventAt
            : state.counter.lastEventAt,
        total: state.counter.total + delta,
      }
      return [
        {
          last_event_at: state.counter.lastEventAt,
          total: state.counter.total,
        },
      ]
    }
    if (text.includes('INSERT INTO khala_sync_public_counters')) {
      const total = Number(values[1])
      state.counter = {
        lastEventAt: state.counter?.lastEventAt ?? null,
        total,
      }
      return [
        {
          last_event_at: state.counter.lastEventAt,
          total: state.counter.total,
        },
      ]
    }
    if (text.includes('SELECT total, last_event_at')) {
      return state.counter === null
        ? []
        : [
            {
              last_event_at: state.counter.lastEventAt,
              total: state.counter.total,
            },
          ]
    }
    if (text.includes('SELECT total FROM khala_sync_public_counters')) {
      return state.counter === null ? [] : [{ total: state.counter.total }]
    }
    if (text.includes('INSERT INTO khala_sync_public_counter_repairs')) {
      state.repairs.push({
        auditNote: String(values[4]),
        newTotal: Number(values[2]),
        previousTotal: values[1] === null ? null : Number(values[1]),
        source: String(values[3]),
      })
      return []
    }
    if (text.includes('INSERT INTO khala_sync_scopes')) {
      state.lastVersion += 1
      return [{ last_version: state.lastVersion }]
    }
    if (text.includes('INSERT INTO khala_sync_changelog')) {
      state.changelogAppends.push({
        postImageJson: values[5] === null ? null : String(values[5]),
      })
      return [{ committed_at: observedAt }]
    }
    throw new Error(`fake pg: unscripted statement: ${text.slice(0, 80)}`)
  }

  const sql = run as unknown as SyncSql & {
    begin: <A>(fn: (tx: SyncTransactionSql) => Promise<A>) => Promise<A>
  }
  ;(sql as { begin: unknown }).begin = async <A>(
    fn: (tx: SyncTransactionSql) => Promise<A>,
  ): Promise<A> => {
    // Snapshot-rollback semantics for the fields the projection touches.
    const snapshot = {
      applied: new Set(state.applied),
      changelogAppends: [...state.changelogAppends],
      counter: state.counter === null ? null : { ...state.counter },
      lastVersion: state.lastVersion,
      repairs: [...state.repairs],
    }
    try {
      return await fn(run as unknown as SyncTransactionSql)
    } catch (error) {
      state.applied = snapshot.applied
      state.changelogAppends = snapshot.changelogAppends
      state.counter = snapshot.counter
      state.lastVersion = snapshot.lastVersion
      state.repairs = snapshot.repairs
      throw error
    }
  }
  return { sql: sql as SyncSql, state }
}

const clientFor = (sql: SyncSql): KhalaSyncPushSqlClient => ({
  end: async () => undefined,
  sql,
})

const binding = { connectionString: 'postgres://hyperdrive-fake' }

type LogCall = { event: string; fields: Record<string, string | number> }
const makeLog = (): { calls: Array<LogCall>; log: TokensServedProjectionLog } => {
  const calls: Array<LogCall> = []
  return {
    calls,
    log: (event, fields) => {
      calls.push({ event, fields: { ...fields } })
    },
  }
}

// ---------------------------------------------------------------------------
// Producer
// ---------------------------------------------------------------------------

describe('recordTokensServedProjectionBestEffort', () => {
  test('skips without a binding and never constructs a client', async () => {
    const outcome = await recordTokensServedProjectionBestEffort(
      {
        binding: undefined,
        makeSqlClient: async () => {
          throw new Error('must not be constructed')
        },
      },
      { idempotencyKey: 'evt-1', observedAt, tokensServedDelta: 10 },
    )
    expect(outcome).toEqual({ outcome: 'skipped_no_binding' })
  })

  test('skips a zero/negative delta', async () => {
    const outcome = await recordTokensServedProjectionBestEffort(
      { binding },
      { idempotencyKey: 'evt-1', observedAt, tokensServedDelta: 0 },
    )
    expect(outcome).toEqual({ outcome: 'skipped_zero_delta' })
  })

  test('applies exact-once: increment + changelog append; replay is a duplicate no-op', async () => {
    const { sql, state } = makeFakePg({
      counter: { lastEventAt: null, total: 1_000 },
    })
    const deps = { binding, makeSqlClient: async () => clientFor(sql) }
    const event = {
      idempotencyKey: 'evt-exact-once',
      observedAt,
      tokensServedDelta: 140,
    }

    const first = await recordTokensServedProjectionBestEffort(deps, event)
    const replay = await recordTokensServedProjectionBestEffort(deps, event)

    expect(first).toEqual({ outcome: 'applied', total: 1_140 })
    expect(replay).toEqual({ outcome: 'duplicate_idempotency_key' })
    expect(state.counter?.total).toBe(1_140)
    expect(state.changelogAppends).toHaveLength(1)
    expect(state.changelogAppends[0]?.postImageJson).toContain('"total":1140')
    expect(state.changelogAppends[0]?.postImageJson).toContain(
      '"counterId":"tokens-served"',
    )
  })

  test('pre-backfill counter refusal is quiet and rolls the guard back (fail-soft)', async () => {
    const { sql, state } = makeFakePg() // no counter row yet
    const { calls, log } = makeLog()

    const outcome = await recordTokensServedProjectionBestEffort(
      { binding, log, makeSqlClient: async () => clientFor(sql) },
      { idempotencyKey: 'evt-pre', observedAt, tokensServedDelta: 5 },
    )

    expect(outcome.outcome).toBe('failed')
    if (outcome.outcome === 'failed') {
      expect(outcome.diagnostic.reason).toBe('counter_not_initialized')
    }
    // Guard rolled back: after the backfill the same event may apply.
    expect(state.applied.size).toBe(0)
    // Pre-backfill refusals are expected — not error noise.
    expect(calls).toHaveLength(0)
  })

  test('a failing client factory is swallowed into a logged typed diagnostic', async () => {
    const { calls, log } = makeLog()
    const outcome = await recordTokensServedProjectionBestEffort(
      {
        binding,
        log,
        makeSqlClient: async () => {
          throw new Error('dial tcp: connection refused postgres://secret')
        },
      },
      { idempotencyKey: 'evt-broken', observedAt, tokensServedDelta: 5 },
    )
    expect(outcome.outcome).toBe('failed')
    expect(calls).toHaveLength(1)
    expect(calls[0]?.event).toBe('khala_sync_tokens_served_projection_failed')
    expect(JSON.stringify(calls[0])).not.toContain('secret')
  })
})

// ---------------------------------------------------------------------------
// Cached reader
// ---------------------------------------------------------------------------

describe('readTokensServedProjectionCached', () => {
  test('reads the counter row and caches it for the declared window', async () => {
    const { sql, state } = makeFakePg({
      counter: { lastEventAt: observedAt, total: 42 },
    })
    let clientConstructions = 0
    let nowMs = 0
    const deps = {
      binding,
      makeSqlClient: async () => {
        clientConstructions += 1
        return clientFor(sql)
      },
      nowMs: () => nowMs,
    }

    expect(await readTokensServedProjectionCached(deps)).toEqual({
      lastEventAt: observedAt,
      tokensServed: 42,
    })
    state.counter = { lastEventAt: observedAt, total: 99 }
    nowMs = 1_999 // still inside the 2s window
    expect(await readTokensServedProjectionCached(deps)).toEqual({
      lastEventAt: observedAt,
      tokensServed: 42,
    })
    nowMs = 2_001
    expect(await readTokensServedProjectionCached(deps)).toEqual({
      lastEventAt: observedAt,
      tokensServed: 99,
    })
    expect(clientConstructions).toBe(2)
  })

  test('misses (undefined) on binding absence, read failure, and missing row — never caches a miss', async () => {
    expect(
      await readTokensServedProjectionCached({ binding: undefined }),
    ).toBeUndefined()

    const { sql } = makeFakePg() // no row
    expect(
      await readTokensServedProjectionCached({
        binding,
        makeSqlClient: async () => clientFor(sql),
      }),
    ).toBeUndefined()

    expect(
      await readTokensServedProjectionCached({
        binding,
        makeSqlClient: async () => {
          throw new Error('unreachable')
        },
      }),
    ).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// Reconcile (invariant 8)
// ---------------------------------------------------------------------------

describe('reconcileTokensServedProjection', () => {
  test('detects seeded drift without writing, and logs the typed diagnostic', async () => {
    const { sql, state } = makeFakePg({
      counter: { lastEventAt: observedAt, total: 900 },
    })
    const { calls, log } = makeLog()

    const result = await reconcileTokensServedProjection(
      {
        binding,
        log,
        makeSqlClient: async () => clientFor(sql),
        nowIso: () => observedAt,
        readExactTokensServed: async () => 1_000,
      },
      { repair: false },
    )

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.report).toMatchObject({
        driftTokens: 100,
        exactTokensServed: 1_000,
        inSync: false,
        projectedTokensServed: 900,
        repaired: false,
      })
    }
    // Detect-only: nothing was written.
    expect(state.counter?.total).toBe(900)
    expect(state.repairs).toHaveLength(0)
    expect(calls.map(call => call.event)).toEqual([
      'khala_sync_tokens_served_projection_drift',
    ])
  })

  test('in-sync reconcile reports zero drift and stays quiet', async () => {
    const { sql } = makeFakePg({
      counter: { lastEventAt: observedAt, total: 1_000 },
    })
    const { calls, log } = makeLog()

    const result = await reconcileTokensServedProjection(
      {
        binding,
        log,
        makeSqlClient: async () => clientFor(sql),
        readExactTokensServed: async () => 1_000,
      },
      { repair: false },
    )

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.report.inSync).toBe(true)
      expect(result.report.driftTokens).toBe(0)
    }
    expect(calls).toHaveLength(0)
  })

  test('repair realigns projection to the exact SUM with the audit row', async () => {
    const { sql, state } = makeFakePg({
      counter: { lastEventAt: observedAt, total: 900 },
    })

    const result = await reconcileTokensServedProjection(
      {
        binding,
        makeSqlClient: async () => clientFor(sql),
        readExactTokensServed: async () => 1_000,
      },
      { auditNote: 'operator-approved drift repair (test)', repair: true },
    )

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.report).toMatchObject({
        previousTotal: 900,
        projectedTokensServed: 1_000,
        repairSource: 'reconcile_repair',
        repaired: true,
      })
    }
    expect(state.counter?.total).toBe(1_000)
    expect(state.repairs).toEqual([
      {
        auditNote: 'operator-approved drift repair (test)',
        newTotal: 1_000,
        previousTotal: 900,
        source: 'reconcile_repair',
      },
    ])
    // The repaired post-image replicated into the scope.
    expect(state.changelogAppends).toHaveLength(1)
  })

  test('repair against an uninitialized counter is the first-deploy backfill', async () => {
    const { sql, state } = makeFakePg() // no counter row yet

    const result = await reconcileTokensServedProjection(
      {
        binding,
        makeSqlClient: async () => clientFor(sql),
        readExactTokensServed: async () => 123_456,
      },
      { repair: true },
    )

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.report).toMatchObject({
        driftTokens: null,
        previousTotal: null,
        projectedTokensServed: 123_456,
        repairSource: 'backfill',
        repaired: true,
      })
    }
    expect(state.counter?.total).toBe(123_456)
    expect(state.repairs[0]?.source).toBe('backfill')
  })

  test('honest typed failures: no binding / exact-read failure', async () => {
    expect(
      await reconcileTokensServedProjection(
        {
          binding: undefined,
          readExactTokensServed: async () => 0,
        },
        { repair: false },
      ),
    ).toMatchObject({ ok: false, reason: 'no_binding' })

    expect(
      await reconcileTokensServedProjection(
        {
          binding,
          makeSqlClient: async () => clientFor(makeFakePg().sql),
          readExactTokensServed: async () => {
            throw new Error('D1 down')
          },
        },
        { repair: false },
      ),
    ).toMatchObject({ ok: false, reason: 'exact_read_failed' })
  })
})
