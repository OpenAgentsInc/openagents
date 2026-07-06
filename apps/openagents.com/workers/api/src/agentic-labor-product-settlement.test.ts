// Settlement-seam test against REAL SQL (node:sqlite), mirroring the cloud-
// metering harness so the never-negative balance CHECK and idempotency_key
// UNIQUE guards are genuine, not modeled. Verifies the armed `settled` path of
// `settleLaborProductOrder` runs a receipt-first debit through the shared ledger,
// and stays idempotent per order.

import { DatabaseSync } from 'node:sqlite'

import type { PaymentsLedgerDb } from './payments-ledger-db'
import { paymentsLedgerDbFromD1 } from './test/payments-ledger-sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildLaborProductFlowPlan,
  carryLaborProductOrderToSettlement,
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

// CFG-4 (#8519): the credits ledger is Postgres-authoritative in production;
// tests back the same `PaymentsLedgerDb` seam with this file's SQLite shim.
const makeDb = (): PaymentsLedgerDb => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return paymentsLedgerDbFromD1(
    new SqliteD1(raw) as unknown as import('./test/payments-ledger-sqlite').D1LikeDatabase,
  )
}

const seedBalance = async (db: PaymentsLedgerDb, msat: number): Promise<void> => {
  await db.batch([
    {
      params: [BUYER, msat, NOW, NOW],
      sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
       VALUES (?, ?, ?, ?)`,
    },
  ])
}

describe('settleLaborProductOrder armed against real SQL', () => {
  test('settles a delivered order receipt-first, decrementing the funded balance', async () => {
    const db = makeDb()
    // 100 sats == 100_000 msat. Fund 250_000 msat.
    await seedBalance(db, 250_000)
    const result = await run(
      settleLaborProductOrder(
        { ledgerDb: db, enabled: true, nowIso: () => NOW },
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
        { ledgerDb: db, enabled: true, nowIso: () => NOW },
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
          { ledgerDb: db, enabled: true, nowIso: () => NOW },
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

// Full sale carry-through against REAL SQL: the composed entry point
// (carryLaborProductOrderToSettlement) is the one thing a real sale flows
// through — order -> dispatch -> deliver -> settle -> recorded receipt — so this
// proves the WHOLE chain composes with money genuinely moving on the ledger and
// a dereferenceable receipt minted, not a fabricated `settled` result.
describe('carryLaborProductOrderToSettlement end-to-end against real SQL', () => {
  const orderRequest = {
    orderId: 'order-e2e-1',
    buyerRef: BUYER,
    listing,
  } as const
  const baseInput = {
    request: orderRequest,
    workerRef: 'agent:worker',
    artifactRef: 'artifact.repo_triage.order-e2e-1',
    adapterId: 'labor-runtime',
    createdAt: NOW,
    settledAt: '2026-06-20T02:00:00.000Z',
  } as const

  test('records a settled order + dereferenceable receipt when armed, owner-signed, and funded', async () => {
    const db = makeDb()
    await seedBalance(db, 250_000)
    const result = await run(
      carryLaborProductOrderToSettlement(
        { ledgerDb: db, enabled: true, nowIso: () => NOW },
        { ...baseInput, ownerSignOffRef: 'owner.sig.labor.e2e' },
      ),
    )

    expect(result._tag).toBe('recorded')
    if (result._tag !== 'recorded') return
    // The terminal stage is produced and identity is carried end to end.
    expect(result.plan.stage).toBe('settled')
    expect(result.plan.orderId).toBe('order-e2e-1')
    expect(result.plan.workerRef).toBe('agent:worker')
    expect(result.plan.artifactRef).toBe('artifact.repo_triage.order-e2e-1')
    // The receipt is dereferenceable and matches the order's own receipt ref.
    expect(result.receipt.settled).toBe(true)
    expect(result.receipt.receiptRef).toBe(
      laborProductOrderReceiptRef('order-e2e-1'),
    )
    expect(result.receipt.receiptRef).toBe(result.plan.settlement.receiptRef)
    expect(result.receipt.streamKind).toBe('labor')
    expect(result.receipt.settledAt).toBe('2026-06-20T02:00:00.000Z')
    // One settled order does not flip the promise.
    expect(result.receipt.promiseState).toBe('yellow')
    expect(result.outcome.metered).toBe(true)
    // Money genuinely moved: 100 sats == 100_000 msat debited from 250_000.
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(150_000)
  })

  test('stays disabled (no ledger IO, no receipt) when the flag is off — the default', async () => {
    const db = makeDb()
    await seedBalance(db, 250_000)
    const result = await run(
      carryLaborProductOrderToSettlement(
        { ledgerDb: db, enabled: false, nowIso: () => NOW },
        { ...baseInput, ownerSignOffRef: 'owner.sig.labor.e2e' },
      ),
    )
    expect(result._tag).toBe('disabled')
    // The default path never touches the balance.
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(250_000)
  })

  test('refuses to settle without an owner sign-off ref, even when armed', async () => {
    const db = makeDb()
    await seedBalance(db, 250_000)
    const result = await run(
      carryLaborProductOrderToSettlement(
        { ledgerDb: db, enabled: true, nowIso: () => NOW },
        baseInput,
      ),
    )
    expect(result._tag).toBe('not_authorized')
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(250_000)
  })

  test('mints NO receipt when the order is under-funded (no money moved)', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)
    const result = await run(
      carryLaborProductOrderToSettlement(
        { ledgerDb: db, enabled: true, nowIso: () => NOW },
        { ...baseInput, ownerSignOffRef: 'owner.sig.labor.e2e' },
      ),
    )
    // Settled on the ledger but moved no money => no dereferenceable receipt.
    expect(result._tag).toBe('not_settled')
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(10_000)
  })

  test('rejects a malformed order request before any ledger IO', async () => {
    const db = makeDb()
    await seedBalance(db, 250_000)
    const result = await run(
      carryLaborProductOrderToSettlement(
        { ledgerDb: db, enabled: true, nowIso: () => NOW },
        {
          ...baseInput,
          request: { ...orderRequest, orderId: '   ' },
          ownerSignOffRef: 'owner.sig.labor.e2e',
        },
      ),
    )
    expect(result._tag).toBe('rejected')
    if (result._tag === 'rejected') {
      expect(result.stage).toBe('ordered')
    }
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(250_000)
  })

  test('is idempotent per order: a replay records again but never double-charges', async () => {
    const db = makeDb()
    await seedBalance(db, 250_000)
    const carry = () =>
      run(
        carryLaborProductOrderToSettlement(
          { ledgerDb: db, enabled: true, nowIso: () => NOW },
          { ...baseInput, ownerSignOffRef: 'owner.sig.labor.e2e' },
        ),
      )
    const first = await carry()
    const second = await carry()
    expect(first._tag).toBe('recorded')
    expect(second._tag).toBe('recorded')
    if (first._tag === 'recorded' && second._tag === 'recorded') {
      expect(second.receipt.receiptRef).toBe(first.receipt.receiptRef)
    }
    // Only one 100_000 msat debit despite two full carry-throughs.
    const balance = await readAgentBalance(db, BUYER)
    expect(balance?.availableMsat).toBe(150_000)
  })
})
