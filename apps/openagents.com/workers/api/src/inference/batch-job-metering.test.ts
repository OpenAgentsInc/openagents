import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type SettleBatchJobChargeDeps,
  inferenceBatchJobChargeIdempotencyKey,
  inferenceBatchJobChargeReceiptRef,
  settleBatchJobCharge,
} from './batch-job-metering'

const makeMockDb = (
  executeStatements: () => Promise<void>,
  findAlready: () => Promise<unknown | null>,
): D1Database =>
  ({
    batch: async () => {
      await executeStatements()
      return [] as any
    },
    prepare: (sql: string) => {
      if (sql.includes('SELECT id FROM pay_ins WHERE idempotency_key = ?')) {
        return {
          bind: () => ({
            first: findAlready,
          }),
        } as any
      }
      return { bind: () => ({}) } as any
    },
  }) as unknown as D1Database

describe('settleBatchJobCharge', () => {
  it('returns ok: true and correct receiptRef when charge succeeds', async () => {
    let batchCalled = false
    const deps: SettleBatchJobChargeDeps = {
      db: makeMockDb(
        async () => {
          batchCalled = true
        },
        async () => null,
      ),
      nowIso: () => '2026-06-20T12:00:00.000Z',
    }

    const result = await Effect.runPromise(
      settleBatchJobCharge(deps, {
        accountRef: 'agent:123',
        costMsat: 1000,
        jobId: 'job_abc',
      }),
    )

    expect(result).toEqual({
      ok: true,
      receiptRef: inferenceBatchJobChargeReceiptRef('job_abc'),
    })
    expect(batchCalled).toBe(true)
  })

  it('returns ok: true for zero cost without writing to ledger', async () => {
    let batchCalled = false
    const deps: SettleBatchJobChargeDeps = {
      db: makeMockDb(
        async () => {
          batchCalled = true
        },
        async () => null,
      ),
      nowIso: () => '2026-06-20T12:00:00.000Z',
    }

    const result = await Effect.runPromise(
      settleBatchJobCharge(deps, {
        accountRef: 'agent:123',
        costMsat: 0,
        jobId: 'job_xyz',
      }),
    )

    expect(result).toEqual({
      ok: true,
      receiptRef: inferenceBatchJobChargeReceiptRef('job_xyz'),
    })
    expect(batchCalled).toBe(false)
  })

  it('returns ok: true if charge fails but idempotency key already exists', async () => {
    const deps: SettleBatchJobChargeDeps = {
      db: makeMockDb(
        async () => {
          throw new Error('Constraint violation')
        },
        async () => ({ id: 'payin:123' }),
      ),
      nowIso: () => '2026-06-20T12:00:00.000Z',
    }

    const result = await Effect.runPromise(
      settleBatchJobCharge(deps, {
        accountRef: 'agent:123',
        costMsat: 1000,
        jobId: 'job_existing',
      }),
    )

    expect(result).toEqual({
      ok: true,
      receiptRef: inferenceBatchJobChargeReceiptRef('job_existing'),
    })
  })

  it('returns ok: false if charge fails and does not already exist', async () => {
    const deps: SettleBatchJobChargeDeps = {
      db: makeMockDb(
        async () => {
          throw new Error('Insufficient funds')
        },
        async () => null,
      ),
      nowIso: () => '2026-06-20T12:00:00.000Z',
    }

    const result = await Effect.runPromise(
      settleBatchJobCharge(deps, {
        accountRef: 'agent:123',
        costMsat: 1000,
        jobId: 'job_fail',
      }),
    )

    expect(result).toEqual({
      ok: false,
      receiptRef: inferenceBatchJobChargeReceiptRef('job_fail'),
    })
  })

  it('generates the correct idempotency key', () => {
    expect(inferenceBatchJobChargeIdempotencyKey('job_123')).toBe(
      'inference:batch_job_charge:job_123',
    )
  })
})
