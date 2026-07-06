import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../payments-ledger'
import {
  cloudChargeIdempotencyKey,
  cloudChargePayInPlan,
  cloudChargeReceiptRef,
  settleCloudPrimitiveCharge,
} from './cloud-metering'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

// Minimal real-SQL D1 adapter backed by node:sqlite (same pattern as the
// inference metering-hook harness). We exercise the shared cloud-metering seam
// against genuine SQL so the balance CHECK (never-negative) and the
// idempotency_key UNIQUE (no double-charge) guards are REAL, not modeled.
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

// Real ledger schema (load-bearing constraints copied verbatim from migration
// 0160: the balance CHECK and the idempotency_key UNIQUE).
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
const ACCOUNT = 'agent:cloud-metering-test'
const PRIMITIVE = 'cloud.fine_tuning.job'

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
    .bind(ACCOUNT, msat, NOW, NOW)
    .run()
}

const charge = (chargeMsat: number, chargeId = 'job1') =>
  ({
    accountRef: ACCOUNT,
    adapterId: 'fine-tuning-runtime',
    chargeId,
    chargeMsat,
    primitive: PRIMITIVE,
  }) as const

describe('cloud-metering refs', () => {
  test('idempotency + receipt refs are stable and public-safe', () => {
    expect(cloudChargeIdempotencyKey(PRIMITIVE, 'job1')).toBe(
      'cloud.fine_tuning.job:charge:job1',
    )
    expect(cloudChargeReceiptRef(PRIMITIVE, 'job1')).toBe(
      'receipt.cloud.fine_tuning.job.charge.job1',
    )
  })

  test('the pay-in plan is a single debit-only balance leg from the account', () => {
    const plan = cloudChargePayInPlan(charge(7000))
    expect(plan.payInType).toBe('adjustment')
    expect(plan.costMsat).toBe(7000)
    expect(plan.legs).toHaveLength(1)
    expect(plan.legs[0]?.direction).toBe('in')
    expect(plan.legs[0]?.partyRef).toBe(ACCOUNT)
    expect(plan.payerRef).toBe(ACCOUNT)
  })
})

describe('settleCloudPrimitiveCharge against real SQL', () => {
  test('decrements the funded balance receipt-first and reports metered', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)
    const outcome = await run(
      settleCloudPrimitiveCharge({ db, nowIso: () => NOW }, charge(7000)),
    )
    expect(outcome.metered).toBe(true)
    expect(outcome.receiptRef).toBe(cloudChargeReceiptRef(PRIMITIVE, 'job1'))
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(3000)
  })

  test('never goes negative: an over-charge fails the debit and reports not metered', async () => {
    const db = makeDb()
    await seedBalance(db, 1000)
    const outcome = await run(
      settleCloudPrimitiveCharge({ db, nowIso: () => NOW }, charge(7000)),
    )
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBeNull()
    expect(outcome.failureReason).toBe('insufficient_credit')
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(1000)
  })

  test('is idempotent per charge id: a replay never double-charges', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)
    const first = await run(
      settleCloudPrimitiveCharge({ db, nowIso: () => NOW }, charge(4000)),
    )
    const second = await run(
      settleCloudPrimitiveCharge({ db, nowIso: () => NOW }, charge(4000)),
    )
    expect(first.metered).toBe(true)
    expect(second.metered).toBe(true)
    const balance = await readAgentBalance(db, ACCOUNT)
    // Only ONE 4000-msat debit landed despite two settle calls.
    expect(balance?.availableMsat).toBe(6000)
  })

  test('a zero charge is metered with zeroCharge and writes no ledger row', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)
    const outcome = await run(
      settleCloudPrimitiveCharge({ db, nowIso: () => NOW }, charge(0)),
    )
    expect(outcome.metered).toBe(true)
    expect(outcome.zeroCharge).toBe(true)
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(10_000)
  })

  // KS-8.7 (#8318/#8337): a wired `mirror` must see the pay_ins/pay_in_legs
  // rows this charge just created — this was the D1-only gap the RUNBOOK
  // coverage list called out for `cloud/cloud-metering.ts` (fine-tuning +
  // sandbox-compute charges were converged by backfill sweeps only, never
  // in real time).
  test('a settled charge mirrors its pay_ins + pay_in_legs refs when a mirror is wired', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)
    const calls: Array<ReadonlyArray<{ table: string; key: unknown }>> = []
    const mirror = async (
      _db: unknown,
      refs: ReadonlyArray<{ table: string; key: unknown }>,
    ) => {
      calls.push(refs)
    }
    const outcome = await run(
      settleCloudPrimitiveCharge(
        { db, mirror, nowIso: () => NOW },
        charge(4000),
      ),
    )
    expect(outcome.metered).toBe(true)
    const payInId = `${PRIMITIVE}:payin:job1`
    const mirroredTables = calls.flat().map(ref => ref.table)
    expect(mirroredTables).toContain('pay_ins')
    expect(mirroredTables).toContain('pay_in_legs')
    const payInRefs = calls
      .flat()
      .filter(ref => ref.table === 'pay_ins')
      .map(ref => ref.key)
    expect(payInRefs).toContainEqual({ id: payInId })
  })

  test('no mirror wired: the charge still settles D1-only (backward compatible)', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)
    const outcome = await run(
      settleCloudPrimitiveCharge({ db, nowIso: () => NOW }, charge(4000)),
    )
    expect(outcome.metered).toBe(true)
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(6000)
  })
})
