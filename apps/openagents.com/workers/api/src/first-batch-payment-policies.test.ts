// KS-8.7 (#8318/#8337): `first_batch_payment_policies` was the RUNBOOK's
// first named "still D1-only" writer for the billing decommission follow-up
// — `upsertFirstBatchPaymentPolicy` wrote through D1 with no live mirror
// call, converged only by the periodic `backfill-billing.ts` sweep. This
// suite proves the fail-soft mirror wiring directly against real SQLite
// (node:sqlite, the engine D1 is built on) — no operator-route mocking.

import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  systemFirstBatchPaymentPolicyRuntime,
  upsertFirstBatchPaymentPolicy,
} from './first-batch-payment-policies'

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

  async run<T = Row>(): Promise<{
    success: true
    results: Array<T>
    meta: { changes: number }
  }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return {
      meta: { changes: Number(result.changes) },
      results: [],
      success: true,
    }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

// Table only — foreign key enforcement is off by default in node:sqlite, so
// the referenced software_orders/adjutant_assignments/site_projects/users
// tables are not needed to exercise this module's own read/write behavior.
const SCHEMA = `
CREATE TABLE first_batch_payment_policies (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT NOT NULL,
  assignment_id TEXT,
  site_id TEXT,
  policy_mode TEXT NOT NULL CHECK (policy_mode IN ('public_beta_free', 'operator_grant')),
  applied_by_user_id TEXT,
  reason TEXT NOT NULL CHECK (length(reason) > 0),
  customer_safe_summary TEXT NOT NULL CHECK (length(customer_safe_summary) > 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE UNIQUE INDEX first_batch_payment_policies_order_active_idx
  ON first_batch_payment_policies(software_order_id)
  WHERE archived_at IS NULL;
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

describe('upsertFirstBatchPaymentPolicy mirror wiring', () => {
  test('with no mirror wired: writes D1-only (backward compatible)', async () => {
    const db = makeDb()
    const policy = await run(
      upsertFirstBatchPaymentPolicy(db, systemFirstBatchPaymentPolicyRuntime, {
        appliedByUserId: 'user_1',
        customerSafeSummary: 'Covered by the public beta free slice.',
        policyMode: 'public_beta_free',
        reason: 'First submitted-order batch is covered by public beta.',
        softwareOrderId: 'order_1',
      }),
    )
    expect(policy.softwareOrderId).toBe('order_1')
    expect(policy.policyMode).toBe('public_beta_free')
  })

  test('a wired mirror sees the converged first_batch_payment_policies row', async () => {
    const db = makeDb()
    const calls: Array<ReadonlyArray<{ table: string; key: unknown }>> = []
    const mirror = async (
      _db: unknown,
      refs: ReadonlyArray<{ table: string; key: unknown }>,
    ) => {
      calls.push(refs)
    }

    const policy = await run(
      upsertFirstBatchPaymentPolicy(
        db,
        { ...systemFirstBatchPaymentPolicyRuntime, mirror },
        {
          appliedByUserId: 'user_1',
          customerSafeSummary: 'Covered by an OpenAgents operator grant.',
          policyMode: 'operator_grant',
          reason: 'Operator-authorized no-charge grant.',
          softwareOrderId: 'order_2',
        },
      ),
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]).toEqual([
      { key: { id: policy.id }, table: 'first_batch_payment_policies' },
    ])
  })

  test('an UPDATE (re-apply on the same order) also mirrors the same row', async () => {
    const db = makeDb()
    const calls: Array<ReadonlyArray<{ table: string; key: unknown }>> = []
    const mirror = async (
      _db: unknown,
      refs: ReadonlyArray<{ table: string; key: unknown }>,
    ) => {
      calls.push(refs)
    }
    const runtime = { ...systemFirstBatchPaymentPolicyRuntime, mirror }
    const input = {
      appliedByUserId: 'user_1',
      customerSafeSummary: 'Covered by the public beta free slice.',
      policyMode: 'public_beta_free' as const,
      reason: 'First submitted-order batch is covered by public beta.',
      softwareOrderId: 'order_3',
    }

    const first = await run(upsertFirstBatchPaymentPolicy(db, runtime, input))
    const second = await run(
      upsertFirstBatchPaymentPolicy(db, runtime, {
        ...input,
        reason: 'Updated reason text.',
      }),
    )

    expect(first.id).toBe(second.id)
    expect(second.reason).toBe('Updated reason text.')
    expect(calls).toHaveLength(2)
    expect(calls[1]).toEqual([
      { key: { id: second.id }, table: 'first_batch_payment_policies' },
    ])
  })
})
