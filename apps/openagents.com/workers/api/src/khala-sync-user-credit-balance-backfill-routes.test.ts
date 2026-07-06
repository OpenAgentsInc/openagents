// Route tests for the admin per-user credit-balance backfill surface (issue
// #8505, Part 2). The batch driver core is covered in
// khala-sync-user-credit-balance.test.ts; these tests drive the HTTP
// contract: admin gate, method allowlist, request-body parsing, and honest
// typed failures.

import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  handleKhalaSyncUserCreditBalanceBackfill,
  KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_PATH,
} from './khala-sync-user-credit-balance-backfill-routes'
import type { UserCreditBalanceBackfillDeps } from './khala-sync-user-credit-balance'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { paymentsLedgerDbFromD1 } from './test/payments-ledger-sqlite'

const url = `https://openagents.com${KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_PATH}`

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
const makeD1WithUsers = (
  userIds: ReadonlyArray<string>,
): { db: D1Database; ledgerDb: PaymentsLedgerDb } => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(`
    CREATE TABLE users (id TEXT PRIMARY KEY, kind TEXT NOT NULL, deleted_at TEXT);
    CREATE TABLE agent_balances (actor_ref TEXT PRIMARY KEY, balance_msat INTEGER NOT NULL);
  `)
  const d1 = new SqliteD1(raw) as unknown as D1Database
  for (const userId of userIds) {
    ;(d1 as unknown as SqliteD1).prepare(`INSERT INTO users (id, kind) VALUES (?, 'human')`).bind(userId).all()
  }
  return { db: d1, ledgerDb: paymentsLedgerDbFromD1(d1 as never) }
}

const makeFakeSql = (): SyncSql => {
  const run = async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): Promise<any> => {
    const text = strings.join('?')
    if (text.includes('SELECT balance_usd_cents, last_event_at')) return []
    if (text.includes('SELECT balance_usd_cents FROM khala_sync_user_credit_balances')) return []
    if (text.includes('INSERT INTO khala_sync_user_credit_balances')) {
      return [{ balance_usd_cents: Number(values[1]), last_event_at: null }]
    }
    if (text.includes('INSERT INTO khala_sync_user_credit_balance_repairs')) return []
    if (text.includes('INSERT INTO khala_sync_scopes')) return [{ last_version: 1 }]
    if (text.includes('INSERT INTO khala_sync_changelog')) {
      return [{ committed_at: '2026-07-04T12:00:00.000Z' }]
    }
    throw new Error(`unscripted: ${text.slice(0, 60)}`)
  }
  const sql = run as unknown as SyncSql & {
    begin: <A>(fn: (tx: SyncTransactionSql) => Promise<A>) => Promise<A>
  }
  ;(sql as { begin: unknown }).begin = async <A>(fn: (tx: SyncTransactionSql) => Promise<A>): Promise<A> =>
    fn(run as unknown as SyncTransactionSql)
  return sql as SyncSql
}

const call = (request: Request, deps: UserCreditBalanceBackfillDeps, authorized = true): Promise<Response> =>
  Effect.runPromise(
    handleKhalaSyncUserCreditBalanceBackfill(request, {
      backfillDeps: deps,
      requireOperator: async () => authorized,
    }),
  )

describe(`${KHALA_SYNC_USER_CREDIT_BALANCE_BACKFILL_PATH}`, () => {
  test('requires the admin bearer (401 otherwise)', async () => {
    const { db, ledgerDb } = makeD1WithUsers(['user-a'])
    const response = await call(new Request(url, { method: 'POST' }), { binding: undefined, db, ledgerDb }, false)
    expect(response.status).toBe(401)
  })

  test('rejects non-POST methods', async () => {
    const { db, ledgerDb } = makeD1WithUsers(['user-a'])
    const response = await call(new Request(url), { binding: undefined, db, ledgerDb })
    expect(response.status).toBe(405)
  })

  test('honest 503 when the KHALA_SYNC_DB binding is absent', async () => {
    const { db, ledgerDb } = makeD1WithUsers(['user-a'])
    const response = await call(new Request(url, { method: 'POST' }), { binding: undefined, db, ledgerDb })
    expect(response.status).toBe(503)
    const body = (await response.json()) as { reason: string }
    expect(body.reason).toBe('no_binding')
  })

  test('backfills a page of human users and returns a report', async () => {
    const { db, ledgerDb } = makeD1WithUsers(['user-a', 'user-b'])
    const sql = makeFakeSql()
    const response = await call(new Request(url, { body: '{}', method: 'POST' }), {
      binding: { connectionString: 'postgres://hyperdrive-fake' },
      db,
      ledgerDb,
      makeSqlClient: async () => ({ end: async () => undefined, sql }),
    })
    expect(response.status).toBe(200)
    const body = (await response.json()) as { ok: boolean; report: { processedCount: number; backfilledCount: number; nextCursor: string | null } }
    expect(body.ok).toBe(true)
    expect(body.report.processedCount).toBe(2)
    expect(body.report.backfilledCount).toBe(2)
    expect(body.report.nextCursor).toBeNull()
  })

  test('honors an explicit limit/cursor for pagination', async () => {
    const { db, ledgerDb } = makeD1WithUsers(['user-a', 'user-b', 'user-c'])
    const sql = makeFakeSql()
    const response = await call(
      new Request(url, { body: JSON.stringify({ limit: 2 }), method: 'POST' }),
      {
        binding: { connectionString: 'postgres://hyperdrive-fake' },
        db,
        ledgerDb,
        makeSqlClient: async () => ({ end: async () => undefined, sql }),
      },
    )
    const body = (await response.json()) as { report: { processedCount: number; nextCursor: string | null } }
    expect(body.report.processedCount).toBe(2)
    expect(body.report.nextCursor).toBe('user-b')
  })

  test('an invalid JSON body is a 400', async () => {
    const { db, ledgerDb } = makeD1WithUsers(['user-a'])
    const response = await call(
      new Request(url, { body: 'not json {{', method: 'POST' }),
      { binding: { connectionString: 'postgres://hyperdrive-fake' }, db, ledgerDb },
    )
    expect(response.status).toBe(400)
  })
})
