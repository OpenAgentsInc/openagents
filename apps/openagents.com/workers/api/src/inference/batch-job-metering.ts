import { Effect } from 'effect'

import { workerLogEntry } from '../observability'
import {
  type PayInPlan,
  createPayInStatements,
  runLedgerStatements,
} from '../payments-ledger'

class BatchJobChargePersistenceError extends Error {
  readonly _tag = 'BatchJobChargePersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'BatchJobChargePersistenceError'
  }
}

const batchJobChargePersistenceError = (error: unknown) =>
  new BatchJobChargePersistenceError(error)

export const inferenceBatchJobChargeReceiptRef = (jobId: string): string =>
  `receipt.inference.batch_job_charge.${jobId}`

export const inferenceBatchJobChargeIdempotencyKey = (jobId: string): string =>
  `inference:batch_job_charge:${jobId}`

export const inferenceBatchJobChargeContextRef = (jobId: string): string =>
  `inference:batch_job:${jobId}`

export const batchJobChargePayInPlan = (
  input: Readonly<{
    jobId: string
    accountRef: string
    costMsat: number
  }>,
): PayInPlan => ({
  contextRef: inferenceBatchJobChargeContextRef(input.jobId),
  costMsat: input.costMsat,
  genesisId: null,
  idempotencyKey: inferenceBatchJobChargeIdempotencyKey(input.jobId),
  legs: [
    {
      amountMsat: input.costMsat,
      direction: 'in',
      externalRef: 'batch_job_charge',
      kind: 'balance',
      legId: `${input.jobId}:debit`,
      partyRef: input.accountRef,
    },
  ],
  payInId: `inference:batch_job_payin:${input.jobId}`,
  payInType: 'adjustment',
  payerRef: input.accountRef,
  publicReceiptRef: inferenceBatchJobChargeReceiptRef(input.jobId),
  rung: null,
})

export type SettleBatchJobChargeDeps = Readonly<{
  db: D1Database
  nowIso: () => string
}>

export const settleBatchJobCharge = (
  deps: SettleBatchJobChargeDeps,
  input: Readonly<{
    jobId: string
    accountRef: string
    costMsat: number
  }>,
): Effect.Effect<{ ok: boolean; receiptRef: string }> =>
  Effect.gen(function* () {
    const receiptRef = inferenceBatchJobChargeReceiptRef(input.jobId)

    if (input.costMsat <= 0) {
      yield* Effect.logInfo(
        workerLogEntry('inference.batch_job.zero_charge', {
          accountRef: input.accountRef,
          jobId: input.jobId,
        }),
      )
      return { ok: true, receiptRef }
    }

    const plan = batchJobChargePayInPlan(input)

    const settle = yield* Effect.tryPromise({
      catch: batchJobChargePersistenceError,
      try: () =>
        runLedgerStatements(deps.db, createPayInStatements(plan, deps.nowIso())),
    }).pipe(
      Effect.map(() => ({ ok: true as const })),
      Effect.catch(() => Effect.succeed({ ok: false as const })),
    )

    if (settle.ok) {
      yield* Effect.logInfo(
        workerLogEntry('inference.batch_job.charged', {
          accountRef: input.accountRef,
          costMsat: input.costMsat,
          jobId: input.jobId,
        }),
      )
      return { ok: true, receiptRef }
    }

    const already = yield* Effect.tryPromise({
      catch: batchJobChargePersistenceError,
      try: () =>
        deps.db
          .prepare('SELECT id FROM pay_ins WHERE idempotency_key = ? LIMIT 1')
          .bind(inferenceBatchJobChargeIdempotencyKey(input.jobId))
          .first(),
    }).pipe(Effect.catch(() => Effect.succeed(null)))

    if (already !== null) {
      return { ok: true, receiptRef }
    }

    yield* Effect.logInfo(
      workerLogEntry('inference.batch_job.charge_failed', {
        accountRef: input.accountRef,
        jobId: input.jobId,
      }),
    )

    return { ok: false, receiptRef }
  })
