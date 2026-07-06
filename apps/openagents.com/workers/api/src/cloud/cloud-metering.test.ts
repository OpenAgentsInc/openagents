import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../payments-ledger'
import type { PaymentsLedgerDb } from '../payments-ledger-db'
import { makeLedgerSqliteDb } from '../test/payments-ledger-sqlite'
import {
  cloudChargeIdempotencyKey,
  cloudChargePayInPlan,
  cloudChargeReceiptRef,
  settleCloudPrimitiveCharge,
} from './cloud-metering'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

// CFG-4 (#8519): the shared cloud-metering seam now runs against the
// Postgres-authoritative `PaymentsLedgerDb`. These tests back it with the
// in-memory ledger SQLite adapter (`makeLedgerSqliteDb`) so the balance CHECK
// (never-negative) and the idempotency_key UNIQUE (no double-charge) guards
// are REAL SQL constraints, not modeled; the Postgres contract suite proves
// the same semantics on the production dialect.
//
// NOTE: the KS-8.7 D1→Postgres MIRROR tests that used to live here were
// DELETED with the mirror itself — the ledger db is the only store now.

const NOW = '2026-06-19T12:00:00.000Z'
const ACCOUNT = 'agent:cloud-metering-test'
const PRIMITIVE = 'cloud.fine_tuning.job'

const makeDb = (): PaymentsLedgerDb => makeLedgerSqliteDb()

const seedBalance = async (
  ledgerDb: PaymentsLedgerDb,
  msat: number,
): Promise<void> => {
  await ledgerDb.batch([
    {
      params: [ACCOUNT, msat, NOW, NOW],
      sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
            VALUES (?, ?, ?, ?)`,
    },
  ])
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
    const ledgerDb = makeDb()
    await seedBalance(ledgerDb, 10_000)
    const outcome = await run(
      settleCloudPrimitiveCharge({ ledgerDb, nowIso: () => NOW }, charge(7000)),
    )
    expect(outcome.metered).toBe(true)
    expect(outcome.receiptRef).toBe(cloudChargeReceiptRef(PRIMITIVE, 'job1'))
    const balance = await readAgentBalance(ledgerDb, ACCOUNT)
    expect(balance?.availableMsat).toBe(3000)
  })

  // Issue #8505 (Part 2): the fail-soft Khala Sync credit-balance projection
  // seam, shared with the inference metering hook.
  describe('recordCreditBalanceProjection seam (#8505)', () => {
    const makeRecorder = () => {
      const calls: Array<{
        accountRef: string
        idempotencyKey: string
        deltaUsdCents: number
        observedAt: string
      }> = []
      return {
        calls,
        recorder: async (event: (typeof calls)[number]) => {
          calls.push(event)
        },
      }
    }

    test('fires once for a fresh charge, with a negative delta and the charge idempotency key', async () => {
      const ledgerDb = makeDb()
      await seedBalance(ledgerDb, 10_000)
      const { calls, recorder } = makeRecorder()
      const outcome = await run(
        settleCloudPrimitiveCharge(
          { ledgerDb, nowIso: () => NOW, recordCreditBalanceProjection: recorder },
          charge(7000),
        ),
      )
      expect(outcome.metered).toBe(true)
      expect(calls).toHaveLength(1)
      expect(calls[0]?.accountRef).toBe(ACCOUNT)
      expect(calls[0]?.idempotencyKey).toBe(cloudChargeIdempotencyKey(PRIMITIVE, 'job1'))
      expect(calls[0]?.observedAt).toBe(NOW)
      expect(calls[0]?.deltaUsdCents).toBeLessThan(0)
    })

    test('never fires on an idempotent replay of the same charge id', async () => {
      const ledgerDb = makeDb()
      await seedBalance(ledgerDb, 10_000)
      const { calls, recorder } = makeRecorder()
      const deps = { ledgerDb, nowIso: () => NOW, recordCreditBalanceProjection: recorder }
      await run(settleCloudPrimitiveCharge(deps, charge(4000)))
      await run(settleCloudPrimitiveCharge(deps, charge(4000)))
      expect(calls).toHaveLength(1)
    })

    test('never fires for a zero charge', async () => {
      const ledgerDb = makeDb()
      await seedBalance(ledgerDb, 10_000)
      const { calls, recorder } = makeRecorder()
      await run(
        settleCloudPrimitiveCharge(
          { ledgerDb, nowIso: () => NOW, recordCreditBalanceProjection: recorder },
          charge(0),
        ),
      )
      expect(calls).toHaveLength(0)
    })
  })

  test('never goes negative: an over-charge fails the debit and reports not metered', async () => {
    const ledgerDb = makeDb()
    await seedBalance(ledgerDb, 1000)
    const outcome = await run(
      settleCloudPrimitiveCharge({ ledgerDb, nowIso: () => NOW }, charge(7000)),
    )
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBeNull()
    expect(outcome.failureReason).toBe('insufficient_credit')
    const balance = await readAgentBalance(ledgerDb, ACCOUNT)
    expect(balance?.availableMsat).toBe(1000)
  })

  test('is idempotent per charge id: a replay never double-charges', async () => {
    const ledgerDb = makeDb()
    await seedBalance(ledgerDb, 10_000)
    const first = await run(
      settleCloudPrimitiveCharge({ ledgerDb, nowIso: () => NOW }, charge(4000)),
    )
    const second = await run(
      settleCloudPrimitiveCharge({ ledgerDb, nowIso: () => NOW }, charge(4000)),
    )
    expect(first.metered).toBe(true)
    expect(second.metered).toBe(true)
    const balance = await readAgentBalance(ledgerDb, ACCOUNT)
    // Only ONE 4000-msat debit landed despite two settle calls.
    expect(balance?.availableMsat).toBe(6000)
  })

  test('a zero charge is metered with zeroCharge and writes no ledger row', async () => {
    const ledgerDb = makeDb()
    await seedBalance(ledgerDb, 10_000)
    const outcome = await run(
      settleCloudPrimitiveCharge({ ledgerDb, nowIso: () => NOW }, charge(0)),
    )
    expect(outcome.metered).toBe(true)
    expect(outcome.zeroCharge).toBe(true)
    const balance = await readAgentBalance(ledgerDb, ACCOUNT)
    expect(balance?.availableMsat).toBe(10_000)
  })

  test('a broken ledger store never reports metered and never throws', async () => {
    const ledgerDb: PaymentsLedgerDb = {
      batch: async () => {
        throw new Error('ledger unavailable')
      },
      query: async () => {
        throw new Error('ledger unavailable')
      },
    }
    const outcome = await run(
      settleCloudPrimitiveCharge({ ledgerDb, nowIso: () => NOW }, charge(4000)),
    )
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBeNull()
  })
})
