// End-to-end proof of the async Khala batch-job flow (#6028 / EPIC #6017):
//
//   submit (request path)  ->  202 + jobId + enqueued BatchJobQueueMessage
//                          ->  consumer runs the job OFF the request path
//                          ->  status flips pending -> processing -> completed
//                          ->  closeout receipt is dereferenceable
//
// This is the wiring the issue calls out as the gap: the consumer
// (`executeBatchJob`) and the read routes already existed; what was missing was
// the PRODUCER (the submit route never enqueued an executable message) and the
// proof that submit -> queue -> consumer -> receipt is one continuous path. The
// test uses a single stateful in-memory D1 so the row the submit route inserts
// is the same row the consumer drives to `completed` and the receipt route then
// dereferences — no real edge call, no live queue.
import { Effect, Schema as S } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type BatchJobRoutesDeps,
  handleBatchJobReceiptRead,
  handleBatchJobsSubmit,
} from './batch-job-routes'
import {
  type BatchJobConsumerDeps,
  BatchJobQueueMessage,
  batchJobCloseoutReceiptRef,
  executeBatchJob,
} from './batch-job-consumer'
import { makeD1BatchJobStore } from './batch-job-store'
import type { MeteringContext, MeteringOutcome } from './metering-hook'
import {
  type InferenceProviderAdapter,
  type InferenceResult,
  InferenceProviderRegistry,
} from './provider-adapter'

const run = <A>(effect: Effect.Effect<A, never, never>): Promise<A> =>
  Effect.runPromise(effect)

// ---- a tiny stateful in-memory D1 ------------------------------------------
//
// Backs exactly the two surfaces this flow touches: the `inference_batch_jobs`
// row (insert on submit, update on consume, read on receipt) and a single
// `pay_ins` cost row (so the receipt route can resolve `cost_msat`). Routes by
// the same SQL fragments the store + receipt route use. Deliberately small —
// enough to prove the handoff, not a general D1.
type BatchRow = {
  job_id: string
  account_ref: string
  status: string
  charge_receipt_ref: string
  dataset_size: number
  processed_items: number
  failed_items: number
  results_r2_key: string | null
  created_at: string
  updated_at: string
  enqueued_at: string | null
  started_at: string | null
}

const makeStatefulDb = (): D1Database => {
  const rows = new Map<string, BatchRow>()
  // The submit charge persists nothing here (the charge ledger is mocked to
  // succeed), so seed the cost row keyed by the charge receipt ref the route
  // emits, so the receipt route's `pay_ins` lookup resolves a cost.
  const payInCostByReceiptRef = new Map<string, number>()

  const prepare = (sql: string) => {
    const make = (bindings: unknown[]) => ({
      first: async <T,>(): Promise<T | null> => {
        if (sql.includes('FROM inference_batch_jobs')) {
          const jobId = String(bindings[0])
          const row = rows.get(jobId)
          return (row ?? null) as T | null
        }
        if (sql.includes('FROM pay_ins') && sql.includes('cost_msat')) {
          const receiptRef = String(bindings[0])
          const cost = payInCostByReceiptRef.get(receiptRef)
          return (cost === undefined ? null : { cost_msat: cost }) as T | null
        }
        if (sql.includes('pay_ins') && sql.includes('idempotency_key')) {
          // charge idempotency probe — treat as first-time (no existing row).
          return null
        }
        return null
      },
      run: async () => {
        if (sql.startsWith('INSERT INTO inference_batch_jobs')) {
          const [
            jobId,
            accountRef,
            status,
            chargeReceiptRef,
            datasetSize,
            processedItems,
            failedItems,
            resultsR2Key,
            createdAt,
            updatedAt,
            enqueuedAt,
            startedAt,
          ] = bindings
          rows.set(String(jobId), {
            account_ref: String(accountRef),
            charge_receipt_ref: String(chargeReceiptRef),
            created_at: String(createdAt),
            dataset_size: Number(datasetSize),
            enqueued_at: enqueuedAt === null ? null : String(enqueuedAt),
            failed_items: Number(failedItems),
            job_id: String(jobId),
            processed_items: Number(processedItems),
            results_r2_key: resultsR2Key === null ? null : String(resultsR2Key),
            started_at: startedAt === null ? null : String(startedAt),
            status: String(status),
            updated_at: String(updatedAt),
          })
          // Seed the cost row the receipt route reads, so a completed job has a
          // resolvable cost (the charge ledger itself is mocked to succeed).
          payInCostByReceiptRef.set(String(chargeReceiptRef), 50_000)
          return { success: true }
        }
        if (sql.startsWith('UPDATE inference_batch_jobs')) {
          // bindings: status, updated_at, [processed_items], [failed_items],
          // [results_r2_key], job_id (last). Parse positionally from the SQL.
          const status = String(bindings[0])
          const updatedAt = String(bindings[1])
          const jobId = String(bindings[bindings.length - 1])
          const existing = rows.get(jobId)
          if (existing !== undefined) {
            let cursor = 2
            const next = { ...existing, status, updated_at: updatedAt }
            if (sql.includes('processed_items = ?')) {
              next.processed_items = Number(bindings[cursor])
              cursor += 1
            }
            if (sql.includes('failed_items = ?')) {
              next.failed_items = Number(bindings[cursor])
              cursor += 1
            }
            if (sql.includes('results_r2_key = ?')) {
              next.results_r2_key = String(bindings[cursor])
              cursor += 1
            }
            if (sql.includes('started_at = ?')) {
              next.started_at = String(bindings[cursor])
              cursor += 1
            }
            rows.set(jobId, next)
          }
          return { success: true }
        }
        return { success: true }
      },
    })
    return { bind: (...bindings: unknown[]) => make(bindings) }
  }

  return {
    batch: async () => [],
    prepare,
  } as unknown as D1Database
}

// A fake gateway adapter that always succeeds with a fixed usage. Stands in for
// the real provider lane (no network, no edge call).
const fakeGatewayAdapter = (id: string): InferenceProviderAdapter => ({
  complete: () =>
    Effect.succeed({
      content: 'fake completion',
      finishReason: 'stop',
      servedModel: `served/${id}`,
      usage: { completionTokens: 20, promptTokens: 10, totalTokens: 30 },
    } satisfies InferenceResult),
  id,
  stream: () => Effect.succeed([]),
})

const dispatchDeps = (adapter: InferenceProviderAdapter) => {
  const registry = new InferenceProviderRegistry()
  registry.register(adapter)
  return { plan: () => [adapter.id], registry, sleep: () => Effect.void }
}

// A metering hook that records each charge (so the test proves credits are
// decremented per executed item) without touching a ledger.
const recordingMetering = (): {
  hook: BatchJobConsumerDeps['meteringHook']
  calls: () => ReadonlyArray<MeteringContext>
} => {
  const calls: MeteringContext[] = []
  return {
    calls: () => calls,
    hook: (context: MeteringContext) => {
      calls.push(context)
      return Effect.succeed({
        metered: true,
        receiptRef: `receipt.inference.charge.${context.requestId}`,
      } satisfies MeteringOutcome)
    },
  }
}

const submitDeps = (
  db: D1Database,
  enqueue: (m: BatchJobQueueMessage) => Effect.Effect<void>,
): BatchJobRoutesDeps => ({
  authenticate: async () => ({ accountRef: 'agent:flow' }),
  db,
  enabled: true,
  enqueueBatchJob: enqueue,
  nowIso: () => '2026-06-22T00:00:00.000Z',
})

const receiptDeps = (db: D1Database): BatchJobRoutesDeps => ({
  authenticate: async () => undefined,
  db,
  enabled: true,
  nowIso: () => '2026-06-22T00:05:00.000Z',
})

const submitRequest = (body: unknown): Request =>
  new Request('https://openagents.com/v1/inference/batches', {
    body: JSON.stringify(body),
    method: 'POST',
  })

const receiptRequest = (receiptRef: string): Request =>
  new Request(
    `https://openagents.com/api/public/inference/batch-job-receipts/${receiptRef}`,
    { method: 'GET' },
  )

describe('async batch-job flow (submit -> queue -> consumer -> receipt)', () => {
  it('runs a detached khala job end-to-end and makes its receipt dereferenceable', async () => {
    const db = makeStatefulDb()
    const captured: BatchJobQueueMessage[] = []
    const enqueue = (message: BatchJobQueueMessage) => {
      captured.push(message)
      return Effect.void
    }

    // 1) SUBMIT a long khala job (a real executable item carrying messages).
    const submitResponse = await run(
      handleBatchJobsSubmit(
        submitRequest({
          dataset: [
            {
              completionTokens: 4000,
              messages: [
                {
                  content:
                    'build a really high quality single html file crossy road game with three.js',
                  role: 'user',
                },
              ],
              model: 'gemini-3.5-flash',
              promptTokens: 200,
            },
          ],
        }),
        submitDeps(db, enqueue),
      ),
    )

    // 202 Accepted + a job id off the request path (never blocks the edge).
    expect(submitResponse.status).toBe(202)
    const submitBody = (await submitResponse.json()) as {
      jobId: string
      status: string
    }
    expect(submitBody.status).toBe('accepted')
    const jobId = submitBody.jobId
    expect(jobId).toMatch(/^batch_/)

    // The producer enqueued exactly one executable message carrying the prompt.
    expect(captured).toHaveLength(1)
    expect(captured[0]?.jobId).toBe(jobId)
    expect(captured[0]?.items).toHaveLength(1)
    expect(captured[0]?.items[0]?.model).toBe('gemini-3.5-flash')
    // Book P0-3: the message carries the enqueue instant (START of the batch wait)
    // so the consumer can compute the wait.
    expect(captured[0]?.enqueuedAtIso).toBe('2026-06-22T00:00:00.000Z')

    // The receipt is NOT yet dereferenceable: the job is still pending.
    const earlyReceipt = await run(
      handleBatchJobReceiptRead(
        receiptRequest(batchJobCloseoutReceiptRef(jobId)),
        receiptDeps(db),
      ),
    )
    expect(earlyReceipt.status).toBe(404)

    // 2) CONSUMER runs the enqueued message OFF the request path. Re-decode the
    // captured message through the schema (as the queue handler does) so the
    // test exercises the same boundary.
    const message = S.decodeUnknownSync(BatchJobQueueMessage)(
      JSON.parse(JSON.stringify(captured[0])),
    )
    const metering = recordingMetering()
    const outcome = await run(
      executeBatchJob(
        {
          dispatch: dispatchDeps(fakeGatewayAdapter('fireworks')),
          meteringHook: metering.hook,
          // Book P0-3: the consumer stamps the start-of-processing time (END of
          // the batch wait) with this clock; with the 00:00:00 enqueue the receipt
          // discloses a real batchWaitMs of 120000 (2 min in queue).
          nowIso: () => '2026-06-22T00:02:00.000Z',
          store: makeD1BatchJobStore(db, () => '2026-06-22T00:02:00.000Z'),
        },
        message,
      ),
    )

    // The consumer drove the job to completion and metered the one item.
    expect(outcome.completed).toBe(true)
    expect(outcome.processedItems).toBe(1)
    expect(outcome.failedItems).toBe(0)
    expect(metering.calls()).toHaveLength(1)
    expect(metering.calls()[0]?.batch).toBe(true)
    expect(metering.calls()[0]?.accountRef).toBe('agent:flow')
    expect(metering.calls()[0]?.requestId).toBe(`${jobId}:0`)

    // 3) RECEIPT is now dereferenceable (status flipped to completed).
    const finalReceipt = await run(
      handleBatchJobReceiptRead(
        receiptRequest(batchJobCloseoutReceiptRef(jobId)),
        receiptDeps(db),
      ),
    )
    expect(finalReceipt.status).toBe(200)
    const receiptBody = (await finalReceipt.json()) as {
      receipt: {
        jobId: string
        totalItems: number
        successfulItems: number
        failedItems: number
        totalCostMsat: number
        openagents: {
          requestClass: string
          queueWaitMs: number | string
          batchWaitMs: number | string
          verificationClass: string
          settlementState: string
        }
      }
    }
    expect(receiptBody.receipt.jobId).toBe(jobId)
    expect(receiptBody.receipt.totalItems).toBe(1)
    expect(receiptBody.receipt.successfulItems).toBe(1)
    expect(receiptBody.receipt.failedItems).toBe(0)
    expect(receiptBody.receipt.totalCostMsat).toBe(50_000)

    // Book P0-3: the TERMINAL `openagents` telemetry record makes the detached job
    // auditable — distinguishable from an interactive stream (`requestClass:
    // batch`), with a measured zero edge queue wait and the REAL in-queue
    // batch wait (00:00:00 enqueue -> 00:02:00 consumer start = 120000ms).
    expect(receiptBody.receipt.openagents.requestClass).toBe('batch')
    expect(receiptBody.receipt.openagents.queueWaitMs).toBe(0)
    expect(receiptBody.receipt.openagents.batchWaitMs).toBe(120_000)
    expect(receiptBody.receipt.openagents.verificationClass).toBe('none')
    expect(receiptBody.receipt.openagents.settlementState).toBe(
      'not_applicable',
    )
  })

  it('re-delivering the same enqueued message is idempotent (no re-run, no re-charge)', async () => {
    const db = makeStatefulDb()
    const captured: BatchJobQueueMessage[] = []
    const enqueue = (message: BatchJobQueueMessage) => {
      captured.push(message)
      return Effect.void
    }

    await run(
      handleBatchJobsSubmit(
        submitRequest({
          dataset: [
            {
              completionTokens: 100,
              messages: [{ content: 'hello', role: 'user' }],
              model: 'gemini-3.5-flash',
              promptTokens: 10,
            },
          ],
        }),
        submitDeps(db, enqueue),
      ),
    )

    const message = S.decodeUnknownSync(BatchJobQueueMessage)(
      JSON.parse(JSON.stringify(captured[0])),
    )
    const store = makeD1BatchJobStore(db, () => '2026-06-22T00:02:00.000Z')

    const first = recordingMetering()
    await run(
      executeBatchJob(
        {
          dispatch: dispatchDeps(fakeGatewayAdapter('fireworks')),
          meteringHook: first.hook,
          store,
        },
        message,
      ),
    )
    expect(first.calls()).toHaveLength(1)

    // Re-deliver: the job is already completed, so the consumer no-ops — no
    // second dispatch, no second charge.
    const second = recordingMetering()
    const replay = await run(
      executeBatchJob(
        {
          dispatch: dispatchDeps(fakeGatewayAdapter('fireworks')),
          meteringHook: second.hook,
          store,
        },
        message,
      ),
    )
    expect(replay.completed).toBe(true)
    expect(second.calls()).toHaveLength(0)
  })

  it('is inert when no producer is wired: accepts + persists but enqueues nothing', async () => {
    const db = makeStatefulDb()

    const response = await run(
      handleBatchJobsSubmit(
        submitRequest({
          dataset: [
            {
              completionTokens: 100,
              messages: [{ content: 'hello', role: 'user' }],
              model: 'gemini-3.5-flash',
              promptTokens: 10,
            },
          ],
        }),
        {
          authenticate: async () => ({ accountRef: 'agent:flow' }),
          db,
          enabled: true,
          // No enqueueBatchJob seam => inert producer.
          nowIso: () => '2026-06-22T00:00:00.000Z',
        },
      ),
    )

    // Still accepts + charges (202), but nothing is queued (no seam): the job
    // stays pending and the receipt is not dereferenceable until a consumer
    // runs it. The receipt route confirms the pending row is non-resolvable.
    expect(response.status).toBe(202)
    const body = (await response.json()) as { jobId: string }
    const receipt = await run(
      handleBatchJobReceiptRead(
        receiptRequest(batchJobCloseoutReceiptRef(body.jobId)),
        receiptDeps(db),
      ),
    )
    expect(receipt.status).toBe(404)
  })
})
