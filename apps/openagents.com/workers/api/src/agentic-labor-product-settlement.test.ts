// Settlement-seam test against REAL SQL (node:sqlite), mirroring the cloud-
// metering harness so the never-negative balance CHECK and idempotency_key
// UNIQUE guards are genuine, not modeled. Verifies the armed `settled` path of
// `settleLaborProductOrder` runs a receipt-first debit through the shared ledger,
// and stays idempotent per order.

import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildLaborProductFlowPlan,
  laborProductOrderReceiptRef,
  settleLaborProductOrder,
  type LaborProductListing,
} from './agentic-labor-product'
import { readAgentBalance } from './payments-ledger'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

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

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
    return { results }
  }

  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

// Real ledger schema (load-bearing constraints copied from the cloud-metering
// harness: the balance CHECK and the idempotency_key UNIQUE).
const SCHEMA = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  held_msat INTEGER NOT NULL DEFAULT 0,
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL,
  payer_ref TEXT NOT NULL,
  cost_msat INTEGER NOT NULL CHECK (cost_msat > 0),
  state TEXT NOT NULL,
  failure_reason TEXT,
  rung TEXT,
  context_ref TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  public_receipt_ref TEXT,
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL
);
CREATE TABLE pay_in_legs (
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
`

const NOW = '2026-06-19T12:00:00.000Z'
const BUYER = 'agent:labor-buyer'

const listing: LaborProductListing = {
  listingId: 'listing-1',
  sellerRef: 'agent:raynor',
  title: 'Repo triage labor product',
  summary: 'Triage one repo backlog and deliver a report.',
  capabilityRef: 'promise:autopilot.agentic_labor_products.v1',
  priceSats: 100,
}

const deliveredPlan = (orderId = 'order-1') => {
  const result = buildLaborProductFlowPlan({
    orderId,
    buyerRef: BUYER,
    listing,
    stage: 'delivered',
    workerRef: 'agent:worker',
    artifactRef: `artifact.repo_triage.${orderId}`,
    createdAt: NOW,
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.plan
}

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const seedBalance = async (db: D1Database, msat: number): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
       VALUES (?, ?, ?, ?)`,
    )
    .bind(BUYER, msat, NOW, NOW)
    .run()
}

describe('settleLaborProductOrder armed against real SQL', () => {
  test('settles a delivered order receipt-first, decrementing the funded balance', async () => {
    const db = makeDb()
    // 100 sats == 100_000 msat. Fund 250_000 msat.
    await seedBalance(db, 250_000)
    const result = await run(
      settleLaborProductOrder(
        { db, enabled: true, nowIso: () => NOW },
        {
          plan: deliveredPlan(),
          adapterId: 'labor-runtime',
          ownerSignOffRef: 'owner.sig.labor.1',
        },
      ),
    )
    expect(result._tag).toBe('settled')
    if (result._tag === 'settled') {
      expect(result.outcome.metered).toBe(true)
      expect(result.outcome.receiptRef).toBe(
        laborProductOrderReceiptRef('order-1'),
      )
    }
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(150_000)
  })

  test('never goes negative: an under-funded order is not metered, balance intact', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)
    const result = await run(
      settleLaborProductOrder(
        { db, enabled: true, nowIso: () => NOW },
        {
          plan: deliveredPlan(),
          adapterId: 'labor-runtime',
          ownerSignOffRef: 'owner.sig.labor.1',
        },
      ),
    )
    expect(result._tag).toBe('settled')
    if (result._tag === 'settled') {
      expect(result.outcome.metered).toBe(false)
    }
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(10_000)
  })

  test('is idempotent per order: a replay never double-charges', async () => {
    const db = makeDb()
    await seedBalance(db, 250_000)
    const settleOnce = () =>
      run(
        settleLaborProductOrder(
          { db, enabled: true, nowIso: () => NOW },
          {
            plan: deliveredPlan(),
            adapterId: 'labor-runtime',
            ownerSignOffRef: 'owner.sig.labor.1',
          },
        ),
      )
    await settleOnce()
    await settleOnce()
    const balance = await readAgentBalance(db, BUYER)
    // Only one 100_000 msat debit despite two settle calls.
    expect(balance?.availableMsat).toBe(150_000)
  })
})
