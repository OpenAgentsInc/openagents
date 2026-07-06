// TEST-ONLY adapter: back the CFG-4 Postgres-authoritative
// `PaymentsLedgerDb` seam (`../payments-ledger-db`) with a SQLite/D1-shaped
// database for fast behavioral tests. Production NEVER uses this — the only
// production wiring is `paymentsLedgerDbForEnv` (Cloud SQL Postgres through
// the `KHALA_SYNC_DB` Hyperdrive binding).
//
// Dialect honesty: the ledger builders' SQL must stay portable between
// SQLite (this adapter) and Postgres (production). `assertPortableLedgerSql`
// rejects known SQLite-only constructs so a test can never green-light SQL
// production would refuse; the real-Postgres contract suite
// (`../payments-ledger-postgres.contract.test.ts`) proves the semantics on
// the production dialect.

import { DatabaseSync } from 'node:sqlite'

import type {
  LedgerParam,
  LedgerRow,
  LedgerSqlStatement,
  PaymentsLedgerDb,
} from '../payments-ledger-db'

const SQLITE_ONLY_SQL =
  /\bINSERT\s+OR\s+(?:IGNORE|REPLACE|ROLLBACK|ABORT|FAIL)\b|\bdatetime\s*\(|\bstrftime\s*\(|\bjulianday\s*\(|\brandomblob\s*\(|\bunixepoch\s*\(/i

export const assertPortableLedgerSql = (sql: string): void => {
  if (SQLITE_ONLY_SQL.test(sql)) {
    throw new Error(
      `ledger SQL is not Postgres-portable (SQLite-only construct): ${sql}`,
    )
  }
}

/** Minimal D1-shaped surface both real D1 and the per-test SQLite shims
 * expose — enough for atomic batches and reads. */
export type D1LikeDatabase = Readonly<{
  prepare: (sql: string) => {
    bind: (...values: ReadonlyArray<unknown>) => {
      all: <T>() => Promise<{ results?: Array<T> }>
      run: () => Promise<unknown>
    }
  }
  batch?: (statements: ReadonlyArray<unknown>) => Promise<unknown>
}>

/**
 * Wrap a D1-shaped SQLite database as a `PaymentsLedgerDb`. When the shim
 * has no `batch`, statements run sequentially — fine for single-threaded
 * test databases (atomicity-under-failure cases belong to the Postgres
 * contract suite).
 */
export const paymentsLedgerDbFromD1 = (
  db: D1LikeDatabase,
): PaymentsLedgerDb => ({
  batch: async (statements: ReadonlyArray<LedgerSqlStatement>) => {
    for (const statement of statements) assertPortableLedgerSql(statement.sql)
    if (typeof db.batch === 'function') {
      await db.batch(
        statements.map(statement =>
          db.prepare(statement.sql).bind(...statement.params),
        ),
      )
      return
    }
    for (const statement of statements) {
      await db.prepare(statement.sql).bind(...statement.params).run()
    }
  },
  query: async (sql: string, params: ReadonlyArray<LedgerParam> = []) => {
    assertPortableLedgerSql(sql)
    const result = await db.prepare(sql).bind(...params).all<LedgerRow>()
    return result.results ?? []
  },
})

/**
 * Self-contained in-memory ledger test database over `node:sqlite`, with a
 * PROPERLY TRANSACTIONAL `batch` (BEGIN … COMMIT, ROLLBACK on failure) so
 * tests that assert atomic abort semantics (insufficient funds, idempotency
 * replay) keep the same all-or-nothing behavior production Postgres has.
 *
 * `schema` defaults to the credits-domain tables; pass extra DDL for tests
 * that co-locate other tables in the same ledger database.
 */
export const makeLedgerSqliteDb = (
  extraSchema = '',
): PaymentsLedgerDb & Readonly<{ raw: DatabaseSync }> => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(CREDITS_LEDGER_SQLITE_SCHEMA)
  if (extraSchema.trim().length > 0) raw.exec(extraSchema)

  const run = (sql: string, params: ReadonlyArray<LedgerParam>) => {
    raw
      .prepare(sql)
      .run(...(params.map(value => (value === undefined ? null : value)) as never[]))
  }

  return {
    batch: async (statements: ReadonlyArray<LedgerSqlStatement>) => {
      for (const statement of statements) assertPortableLedgerSql(statement.sql)
      raw.exec('BEGIN')
      try {
        for (const statement of statements) run(statement.sql, statement.params)
        raw.exec('COMMIT')
      } catch (error) {
        raw.exec('ROLLBACK')
        throw error
      }
    },
    query: async (sql: string, params: ReadonlyArray<LedgerParam> = []) => {
      assertPortableLedgerSql(sql)
      return raw
        .prepare(sql)
        .all(...(params as never[])) as unknown as Array<LedgerRow>
    },
    raw,
  }
}

/** SQLite rendering of the credits-domain tables (mirrors the D1 worker
 * migrations 0160/0167/0211/0261/0308 shapes and the Postgres twins in
 * khala-sync-server 0015/0016). */
export const CREDITS_LEDGER_SQLITE_SCHEMA = `
CREATE TABLE IF NOT EXISTS agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0),
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0)
);

CREATE TABLE IF NOT EXISTS pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL,
  payer_ref TEXT NOT NULL,
  cost_msat INTEGER NOT NULL CHECK (cost_msat > 0),
  state TEXT NOT NULL,
  failure_reason TEXT,
  rung TEXT,
  context_ref TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL,
  public_receipt_ref TEXT
);

CREATE TABLE IF NOT EXISTS pay_in_legs (
  id TEXT PRIMARY KEY,
  pay_in_id TEXT NOT NULL,
  direction TEXT NOT NULL,
  kind TEXT NOT NULL,
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS labor_escrows (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  state TEXT NOT NULL,
  funding_source TEXT NOT NULL DEFAULT 'ledger_balance',
  job_event_id TEXT NOT NULL,
  acceptance_event_ref TEXT,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  release_receipt_ref TEXT UNIQUE,
  refund_receipt_ref TEXT UNIQUE,
  forfeit_receipt_ref TEXT UNIQUE,
  forfeit_destination TEXT,
  forfeit_destination_actor_ref TEXT,
  forfeit_condition_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  released_at TEXT,
  refunded_at TEXT,
  forfeited_at TEXT,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS labor_escrow_receipts (
  id TEXT PRIMARY KEY,
  escrow_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  transition_kind TEXT NOT NULL,
  work_request_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  receipt_ref TEXT NOT NULL UNIQUE,
  evidence_ref TEXT,
  state_after TEXT NOT NULL,
  forfeit_destination TEXT,
  forfeit_destination_actor_ref TEXT,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE (escrow_id, transition_kind)
);
`
