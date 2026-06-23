import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../payments-ledger'
import {
  isPublicSafeCloudPrimitiveReceiptProjection,
  makeD1CloudPrimitiveReceiptStore,
  publicCloudPrimitiveReceiptFromRecord,
} from './cloud-primitive-receipts'
import {
  makeLedgerSandboxMeteringHook,
  sandboxRentalReceiptRef,
} from './sandbox-compute-service-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

// Minimal real-SQL D1 adapter backed by node:sqlite (same harness the
// cloud-metering test uses), so the metered debit AND the receipt read run
// against genuine SQL — the receipt this test dereferences is a real settled
// ledger row, not a mock.
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

// Real ledger schema (load-bearing constraints from migration 0160).
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

const NOW = '2026-06-23T12:00:00.000Z'
const ACCOUNT = 'agent:sandbox-receipt-test'

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

describe('cloud-primitive receipt projection', () => {
  test('only a PAID adjustment charge with a cloud prefix projects', () => {
    expect(
      publicCloudPrimitiveReceiptFromRecord(
        {
          contextRef: null,
          createdAt: NOW,
          payInType: 'adjustment',
          receiptRef: 'receipt.cloud.sandbox_compute.rental.charge.s1',
          state: 'paid',
          stateChangedAt: NOW,
        },
        NOW,
      ),
    ).not.toBeNull()

    // A still-pending charge is NOT a dereferenceable receipt.
    expect(
      publicCloudPrimitiveReceiptFromRecord(
        {
          contextRef: null,
          createdAt: NOW,
          payInType: 'adjustment',
          receiptRef: 'receipt.cloud.sandbox_compute.rental.charge.s1',
          state: 'pending',
          stateChangedAt: NOW,
        },
        NOW,
      ),
    ).toBeNull()

    // A non-cloud ref does NOT resolve here.
    expect(
      publicCloudPrimitiveReceiptFromRecord(
        {
          contextRef: null,
          createdAt: NOW,
          payInType: 'adjustment',
          receiptRef: 'receipt.inference.charge.req1',
          state: 'paid',
          stateChangedAt: NOW,
        },
        NOW,
      ),
    ).toBeNull()
  })

  test('the projection is public-safe (no payment material)', () => {
    const projection = publicCloudPrimitiveReceiptFromRecord(
      {
        contextRef: null,
        createdAt: NOW,
        payInType: 'adjustment',
        receiptRef: 'receipt.cloud.fine_tuning.job.charge.j1',
        state: 'paid',
        stateChangedAt: NOW,
      },
      NOW,
    )
    expect(projection).not.toBeNull()
    expect(projection?.kind).toBe('fine_tuning_job')
    expect(projection?.ledgerState).toBe('paid')
    expect(isPublicSafeCloudPrimitiveReceiptProjection(projection)).toBe(true)
  })
})

// THE PROOF: rent -> real metered debit -> dereferenceable PAID receipt.
// This is the receipt artifact `cloud.sandbox_compute_service.v1` was missing.
describe('end-to-end: metered sandbox rental yields a dereferenceable receipt', () => {
  test('a closed rental debits credits AND the receipt the surface advertises resolves', async () => {
    const db = makeDb()
    await seedBalance(db, 10_000)

    const sandboxId = 'sbx_proof_1'

    // (1) RENT closes with REAL metered usage. The live ledger metering hook
    // prices it from usage (never an estimate), debits credits receipt-first
    // through the shared cloud-metering seam, and marks the debit PAID.
    const hook = makeLedgerSandboxMeteringHook({
      db,
      nowIso: () => NOW,
      // Pure pricing from real metered usage. Use integer-msat-per-second math
      // so the charge is exact (no float drift): 10 msat per wall-second.
      priceUsd: context => context.usage?.wallSeconds ?? 0,
      usdToMsat: seconds => seconds * 10,
    })

    const metering = await run(
      hook({
        accountRef: ACCOUNT,
        sandboxId,
        image: 'oa-sandbox-base',
        usage: { wallSeconds: 300 },
      }),
    )

    expect(metering.metered).toBe(true)
    // The ref the surface advertises is the ref we will dereference.
    const advertisedRef = sandboxRentalReceiptRef(sandboxId)
    expect(metering.receiptRef).toBe(advertisedRef)
    expect(advertisedRef).toBe(
      'receipt.cloud.sandbox_compute.rental.charge.sbx_proof_1',
    )

    // The metered debit actually moved credits (300s * 10 msat = 3000 msat).
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(7000)

    // (2) DEREFERENCE: the advertised receipt resolves against the real ledger
    // row written above, and projects a public-safe PAID receipt. THIS is the
    // dereferenceable proof of rent -> metered -> charge.
    const store = makeD1CloudPrimitiveReceiptStore(db)
    const record = await store.readCloudPrimitiveReceiptByRef(advertisedRef)
    expect(record).not.toBeNull()
    expect(record?.state).toBe('paid')

    const receipt = publicCloudPrimitiveReceiptFromRecord(record!, NOW)
    expect(receipt).not.toBeNull()
    expect(receipt?.kind).toBe('sandbox_compute_rental')
    expect(receipt?.ledgerState).toBe('paid')
    expect(receipt?.receiptRef).toBe(advertisedRef)
    expect(receipt?.sourceRefs).toContain(
      `route:/api/public/cloud/receipts/${advertisedRef}`,
    )
    // Honest: the receipt itself never claims the promise is green.
    expect(receipt?.caveatRefs).toContain(
      'caveat.public.cloud_primitive_demand_provenance_and_owner_signoff_pending',
    )
    expect(isPublicSafeCloudPrimitiveReceiptProjection(receipt)).toBe(true)
  })

  test('an unknown / unsettled ref does not resolve', async () => {
    const db = makeDb()
    const store = makeD1CloudPrimitiveReceiptStore(db)
    expect(
      await store.readCloudPrimitiveReceiptByRef(
        'receipt.cloud.sandbox_compute.rental.charge.nope',
      ),
    ).toBeNull()
  })
})
