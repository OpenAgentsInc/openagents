// Unit tests for the Worker-side per-user credit-balance projection seams
// (issue #8505, Part 2) — fail-soft producer and the paginated backfill
// driver. These drive the REAL @openagentsinc/khala-sync-server projection
// code over a scripted fake postgres.js-shaped client (pattern-matched
// tagged templates, same technique as
// `khala-sync-public-tokens-served.test.ts`) plus a real in-memory SQLite D1
// fake for the user-listing query, so CI needs no external database; the
// full Postgres integration lives in
// packages/khala-sync-server/src/user-credit-balance-projection.test.ts.

import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import {
  backfillUserCreditBalancesBatch,
  listUsersForCreditBalanceBackfill,
  recordUserCreditBalanceDeltaBestEffort,
  userIdFromAgentRef,
  type UserCreditBalanceProjectionLog,
} from './khala-sync-user-credit-balance'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { paymentsLedgerDbFromD1 } from './test/payments-ledger-sqlite'

const observedAt = '2026-07-04T12:00:00.000Z'

// ---------------------------------------------------------------------------
// Scripted fake Postgres (pattern-matched tagged-template SQL)
// ---------------------------------------------------------------------------

type FakePgState = {
  /** khala_sync_user_credit_balances rows, keyed by user_id. */
  balances: Map<string, { balanceUsdCents: number; lastEventAt: string | null }>
  applied: Set<string>
  lastVersion: number
  changelogAppends: Array<{ postImageJson: string | null }>
  repairs: Array<{ userId: string; previousBalance: number | null; newBalance: number; source: string }>
}

const makeFakePg = (): { state: FakePgState; sql: SyncSql } => {
  const state: FakePgState = {
    applied: new Set(),
    balances: new Map(),
    changelogAppends: [],
    lastVersion: 0,
    repairs: [],
  }

  const run = async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): Promise<any> => {
    const text = strings.join('?')
    if (text.includes('INSERT INTO khala_sync_user_credit_balance_applied')) {
      const userId = String(values[0])
      const key = String(values[1])
      const compound = `${userId}::${key}`
      if (state.applied.has(compound)) return []
      state.applied.add(compound)
      return [{ idempotency_key: key }]
    }
    if (text.includes('UPDATE khala_sync_user_credit_balances')) {
      // interpolations: [deltaUsdCents, observedAt, observedAt, userId] — the
      // GREATEST(COALESCE(...), ...) expression references observedAt twice.
      const userId = String(values[3])
      const existing = state.balances.get(userId)
      if (existing === undefined) return []
      const delta = Number(values[0])
      const eventAt = String(values[1])
      const newBalance = existing.balanceUsdCents + delta
      if (newBalance < 0) {
        throw new Error('khala_sync_user_credit_balances_balance_usd_cents_check')
      }
      const updated = {
        balanceUsdCents: newBalance,
        lastEventAt:
          existing.lastEventAt === null || eventAt > existing.lastEventAt
            ? eventAt
            : existing.lastEventAt,
      }
      state.balances.set(userId, updated)
      return [{ balance_usd_cents: updated.balanceUsdCents, last_event_at: updated.lastEventAt }]
    }
    if (text.includes('INSERT INTO khala_sync_user_credit_balances')) {
      const userId = String(values[0])
      // Test-only sentinel: simulates a genuine per-user repair failure
      // (e.g. a transient constraint/connection error) so the batch
      // driver's fail-soft per-user try/catch has something real to catch.
      if (userId === 'poison-user') {
        throw new Error('simulated repair failure for poison-user')
      }
      const balanceUsdCents = Number(values[1])
      const existing = state.balances.get(userId)
      const updated = { balanceUsdCents, lastEventAt: existing?.lastEventAt ?? null }
      state.balances.set(userId, updated)
      return [{ balance_usd_cents: updated.balanceUsdCents, last_event_at: updated.lastEventAt }]
    }
    if (text.includes('SELECT balance_usd_cents, last_event_at')) {
      const userId = String(values[0])
      const existing = state.balances.get(userId)
      return existing === undefined
        ? []
        : [{ balance_usd_cents: existing.balanceUsdCents, last_event_at: existing.lastEventAt }]
    }
    if (text.includes('SELECT balance_usd_cents FROM khala_sync_user_credit_balances')) {
      const userId = String(values[0])
      const existing = state.balances.get(userId)
      return existing === undefined ? [] : [{ balance_usd_cents: existing.balanceUsdCents }]
    }
    if (text.includes('INSERT INTO khala_sync_user_credit_balance_repairs')) {
      state.repairs.push({
        newBalance: Number(values[2]),
        previousBalance: values[1] === null ? null : Number(values[1]),
        source: String(values[3]),
        userId: String(values[0]),
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
    const snapshot = {
      applied: new Set(state.applied),
      balances: new Map(
        Array.from(state.balances.entries(), ([key, value]) => [key, { ...value }] as const),
      ),
      changelogAppends: [...state.changelogAppends],
      lastVersion: state.lastVersion,
      repairs: [...state.repairs],
    }
    try {
      return await fn(run as unknown as SyncTransactionSql)
    } catch (error) {
      state.applied = snapshot.applied
      state.balances = snapshot.balances
      state.changelogAppends = snapshot.changelogAppends
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
const makeLog = (): { calls: Array<LogCall>; log: UserCreditBalanceProjectionLog } => {
  const calls: Array<LogCall> = []
  return {
    calls,
    log: (event, fields) => {
      calls.push({ event, fields: { ...fields } })
    },
  }
}

// ---------------------------------------------------------------------------
// userIdFromAgentRef
// ---------------------------------------------------------------------------

describe('userIdFromAgentRef', () => {
  test('parses a valid agent: ref', () => {
    expect(userIdFromAgentRef('agent:user-123')).toBe('user-123')
  })

  test('returns undefined for a non-agent ref (never guesses)', () => {
    expect(userIdFromAgentRef('pubkey:abcdef')).toBeUndefined()
  })

  test('returns undefined for an empty userId suffix', () => {
    expect(userIdFromAgentRef('agent:')).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// Producer
// ---------------------------------------------------------------------------

describe('recordUserCreditBalanceDeltaBestEffort', () => {
  test('skips without a binding and never constructs a client', async () => {
    const outcome = await recordUserCreditBalanceDeltaBestEffort(
      {
        binding: undefined,
        makeSqlClient: async () => {
          throw new Error('must not be constructed')
        },
      },
      { deltaUsdCents: 500, idempotencyKey: 'evt-1', observedAt, userId: 'user-1' },
    )
    expect(outcome).toEqual({ outcome: 'skipped_no_binding' })
  })

  test('skips a zero delta', async () => {
    const outcome = await recordUserCreditBalanceDeltaBestEffort(
      { binding },
      { deltaUsdCents: 0, idempotencyKey: 'evt-1', observedAt, userId: 'user-1' },
    )
    expect(outcome).toEqual({ outcome: 'skipped_zero_delta' })
  })

  test('applies a grant (positive delta) exact-once; replay is a duplicate no-op', async () => {
    const { sql, state } = makeFakePg()
    state.balances.set('user-1', { balanceUsdCents: 1_000, lastEventAt: null })
    const deps = { binding, makeSqlClient: async () => clientFor(sql) }
    const event = {
      deltaUsdCents: 500,
      idempotencyKey: 'evt-grant-1',
      observedAt,
      userId: 'user-1',
    }

    const first = await recordUserCreditBalanceDeltaBestEffort(deps, event)
    const replay = await recordUserCreditBalanceDeltaBestEffort(deps, event)

    expect(first).toEqual({ balanceUsdCents: 1_500, outcome: 'applied' })
    expect(replay).toEqual({ outcome: 'duplicate_idempotency_key' })
    expect(state.balances.get('user-1')?.balanceUsdCents).toBe(1_500)
    expect(state.changelogAppends).toHaveLength(1)
    expect(state.changelogAppends[0]?.postImageJson).toContain('"balanceUsdCents":1500')
    expect(state.changelogAppends[0]?.postImageJson).toContain('"userId":"user-1"')
  })

  test('applies a charge (negative delta) exact-once', async () => {
    const { sql, state } = makeFakePg()
    state.balances.set('user-1', { balanceUsdCents: 1_000, lastEventAt: null })
    const deps = { binding, makeSqlClient: async () => clientFor(sql) }

    const outcome = await recordUserCreditBalanceDeltaBestEffort(deps, {
      deltaUsdCents: -400,
      idempotencyKey: 'evt-charge-1',
      observedAt,
      userId: 'user-1',
    })
    expect(outcome).toEqual({ balanceUsdCents: 600, outcome: 'applied' })
  })

  test('pre-backfill refusal is quiet and rolls the guard back (fail-soft)', async () => {
    const { sql, state } = makeFakePg() // no balance row yet
    const { calls, log } = makeLog()
    const deps = { binding, log, makeSqlClient: async () => clientFor(sql) }

    const outcome = await recordUserCreditBalanceDeltaBestEffort(deps, {
      deltaUsdCents: 500,
      idempotencyKey: 'evt-pre-1',
      observedAt,
      userId: 'never-backfilled-user',
    })
    expect(outcome.outcome).toBe('failed')
    if (outcome.outcome === 'failed') {
      expect(outcome.diagnostic.reason).toBe('credit_balance_not_initialized')
    }
    // Quiet: not logged as an error (expected pre-backfill state).
    expect(calls).toHaveLength(0)
    expect(state.applied.size).toBe(0)
  })

  test('one user\'s delta never touches another user\'s balance', async () => {
    const { sql, state } = makeFakePg()
    state.balances.set('user-1', { balanceUsdCents: 1_000, lastEventAt: null })
    state.balances.set('user-2', { balanceUsdCents: 5_000, lastEventAt: null })
    const deps = { binding, makeSqlClient: async () => clientFor(sql) }

    await recordUserCreditBalanceDeltaBestEffort(deps, {
      deltaUsdCents: -200,
      idempotencyKey: 'evt-user-1-charge',
      observedAt,
      userId: 'user-1',
    })
    expect(state.balances.get('user-1')?.balanceUsdCents).toBe(800)
    expect(state.balances.get('user-2')?.balanceUsdCents).toBe(5_000)
  })
})

// ---------------------------------------------------------------------------
// Backfill batch driver (real in-memory SQLite D1 + fake Postgres)
// ---------------------------------------------------------------------------

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return { results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T> }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

// CFG-4 (#8519): `users` stays on D1; `agent_balances` reads go through the
// credits ledger handle. In tests both share one underlying SQLite database.
const makeD1 = (): { db: D1Database; ledger: PaymentsLedgerDb } => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(`
    CREATE TABLE users (id TEXT PRIMARY KEY, kind TEXT NOT NULL, deleted_at TEXT);
    CREATE TABLE agent_balances (actor_ref TEXT PRIMARY KEY, balance_msat INTEGER NOT NULL);
  `)
  const db = new SqliteD1(raw) as unknown as D1Database
  return { db, ledger: paymentsLedgerDbFromD1(db as never) }
}

const insertUser = (db: D1Database, userId: string): void => {
  ;(db as unknown as SqliteD1).prepare(`INSERT INTO users (id, kind) VALUES (?, 'human')`).bind(userId).all()
}

const insertBalance = (db: D1Database, userId: string, balanceMsat: number): void => {
  ;(db as unknown as SqliteD1)
    .prepare(`INSERT INTO agent_balances (actor_ref, balance_msat) VALUES (?, ?)`)
    .bind(`agent:${userId}`, balanceMsat)
    .all()
}

describe('listUsersForCreditBalanceBackfill', () => {
  test('lists human users with a default $0 for one who has never been charged or granted', async () => {
    const { db, ledger } = makeD1()
    insertUser(db, 'user-a')
    insertUser(db, 'user-b')
    insertBalance(db, 'user-b', 5_000_000)

    const rows = await listUsersForCreditBalanceBackfill(ledger, { limit: 10 })
    expect(rows).toEqual([
      { balanceMsat: 0, userId: 'user-a' },
      { balanceMsat: 5_000_000, userId: 'user-b' },
    ])
  })

  test('paginates by a keyset user-id cursor', async () => {
    const { db, ledger } = makeD1()
    insertUser(db, 'user-a')
    insertUser(db, 'user-b')
    insertUser(db, 'user-c')

    const firstPage = await listUsersForCreditBalanceBackfill(ledger, { limit: 2 })
    expect(firstPage.map(row => row.userId)).toEqual(['user-a', 'user-b'])
    const secondPage = await listUsersForCreditBalanceBackfill(ledger, {
      cursor: firstPage[firstPage.length - 1]?.userId,
      limit: 2,
    })
    expect(secondPage.map(row => row.userId)).toEqual(['user-c'])
  })
})

describe('backfillUserCreditBalancesBatch', () => {
  test('backfills a never-initialized user to the exact D1 balance', async () => {
    const { db, ledger } = makeD1()
    insertUser(db, 'user-a')
    insertBalance(db, 'user-a', 5_000_000) // $5.00 at DEFAULT_BTC_USD

    const { sql, state } = makeFakePg()
    const result = await backfillUserCreditBalancesBatch(
      { binding, ledgerDb: ledger, makeSqlClient: async () => clientFor(sql) },
      { limit: 10 },
    )
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.report).toEqual({
        backfilledCount: 1,
        failedCount: 0,
        nextCursor: null,
        processedCount: 1,
        reconciledCount: 0,
        unchangedCount: 0,
      })
    }
    expect(state.balances.get('user-a')?.balanceUsdCents).toBe(500)
    expect(state.repairs).toEqual([
      { newBalance: 500, previousBalance: null, source: 'backfill', userId: 'user-a' },
    ])
  })

  test('reconciles a drifted already-initialized user and leaves an exact match unchanged', async () => {
    const { db, ledger } = makeD1()
    insertUser(db, 'drifted-user')
    insertBalance(db, 'drifted-user', 5_000_000) // exact D1 balance now: $5.00 == 500 cents
    insertUser(db, 'exact-user')
    insertBalance(db, 'exact-user', 1_000_000) // $1.00 == 100 cents

    const { sql, state } = makeFakePg()
    // Seed projection state as if a prior backfill happened, but the
    // "drifted-user" projection is now stale (a lost best-effort delta).
    state.balances.set('drifted-user', { balanceUsdCents: 200, lastEventAt: null })
    state.balances.set('exact-user', { balanceUsdCents: 100, lastEventAt: null })

    const result = await backfillUserCreditBalancesBatch(
      { binding, ledgerDb: ledger, makeSqlClient: async () => clientFor(sql) },
      { limit: 10 },
    )
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.report).toEqual({
        backfilledCount: 0,
        failedCount: 0,
        nextCursor: null,
        processedCount: 2,
        reconciledCount: 1,
        unchangedCount: 1,
      })
    }
    expect(state.balances.get('drifted-user')?.balanceUsdCents).toBe(500)
    expect(state.balances.get('exact-user')?.balanceUsdCents).toBe(100)
    expect(state.repairs).toEqual([
      { newBalance: 500, previousBalance: 200, source: 'reconcile_repair', userId: 'drifted-user' },
    ])
  })

  test('reports no_binding without touching D1 or constructing a client', async () => {
    const { db, ledger } = makeD1()
    insertUser(db, 'user-a')
    const result = await backfillUserCreditBalancesBatch(
      {
        binding: undefined,
        ledgerDb: ledger,
        makeSqlClient: async () => {
          throw new Error('must not be constructed')
        },
      },
      { limit: 10 },
    )
    expect(result).toEqual({
      messageSafe:
        'Khala Sync storage is not configured on this deployment ' +
        '(env.KHALA_SYNC_DB Hyperdrive binding is absent).',
      ok: false,
      reason: 'no_binding',
    })
  })

  test('a per-user repair failure is fail-soft: other users in the page still get backfilled', async () => {
    const { db, ledger } = makeD1()
    insertUser(db, 'poison-user')
    insertBalance(db, 'poison-user', 1_000_000)
    insertUser(db, 'user-b')
    insertBalance(db, 'user-b', 2_000_000)

    const { sql, state } = makeFakePg()
    const { calls, log } = makeLog()
    const result = await backfillUserCreditBalancesBatch(
      { binding, ledgerDb: ledger, log, makeSqlClient: async () => clientFor(sql) },
      { limit: 10 },
    )
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.report.processedCount).toBe(2)
      expect(result.report.failedCount).toBe(1)
      expect(result.report.backfilledCount).toBe(1)
    }
    // The poisoned user never got a row; the other user in the same page
    // still succeeded.
    expect(state.balances.has('poison-user')).toBe(false)
    expect(state.balances.get('user-b')?.balanceUsdCents).toBe(200)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.event).toBe('khala_sync_user_credit_balance_backfill_failed')
  })
})
