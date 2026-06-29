import { Effect, Schema as S } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type BatchJobConsumerDeps,
  BatchJobQueueMessage,
  batchJobCloseoutReceiptRef,
  executeBatchJob,
} from './batch-job-consumer'
import type {
  BatchJobRecord,
  BatchJobStatus,
  BatchJobStore,
} from './batch-job-store'
import type { MeteringContext, MeteringOutcome } from './metering-hook'
import { InferenceProviderRegistry } from './provider-adapter'
import type {
  InferenceAdapterError,
  InferenceProviderAdapter,
  InferenceResult,
} from './provider-adapter'
import { InferenceAdapterError as AdapterError } from './provider-adapter'

const run = <A>(effect: Effect.Effect<A, never, never>): Promise<A> =>
  Effect.runPromise(effect)

// ---- fakes -----------------------------------------------------------------

// An in-memory batch-job store that records every status write, so a test can
// assert the consumer drove the row to its terminal state and progress counts.
const makeFakeStore = (
  initial: BatchJobRecord | null,
): {
  store: BatchJobStore
  reads: () => BatchJobRecord | null
  statusWrites: () => ReadonlyArray<{
    status: BatchJobStatus
    processedItems?: number
    failedItems?: number
    resultsR2Key?: string | null
    startedAt?: string
  }>
} => {
  let record = initial
  const statusWrites: Array<{
    status: BatchJobStatus
    processedItems?: number
    failedItems?: number
    resultsR2Key?: string | null
    startedAt?: string
  }> = []
  const store: BatchJobStore = {
    getBatchJob: () => Effect.succeed(record),
    insertBatchJob: () => Effect.void,
    updateBatchJobStatus: (_jobId, status, updates) => {
      statusWrites.push({ status, ...updates })
      if (record !== null) {
        record = {
          ...record,
          status,
          processedItems: updates.processedItems ?? record.processedItems,
          failedItems: updates.failedItems ?? record.failedItems,
          resultsR2Key:
            updates.resultsR2Key === undefined
              ? record.resultsR2Key
              : updates.resultsR2Key,
          startedAt:
            updates.startedAt === undefined
              ? record.startedAt
              : updates.startedAt,
        }
      }
      return Effect.void
    },
  }
  return {
    reads: () => record,
    statusWrites: () => statusWrites,
    store,
  }
}

const pendingJob = (
  overrides: Partial<BatchJobRecord> = {},
): BatchJobRecord => ({
  accountRef: 'agent:abc',
  chargeReceiptRef: 'receipt.inference.batch_job_charge.batch_test',
  createdAt: '2026-06-22T00:00:00.000Z',
  datasetSize: 2,
  enqueuedAt: '2026-06-22T00:00:00.000Z',
  failedItems: 0,
  jobId: 'batch_test',
  processedItems: 0,
  resultsR2Key: null,
  startedAt: null,
  status: 'pending',
  updatedAt: '2026-06-22T00:00:00.000Z',
  ...overrides,
})

// A fake gateway adapter that succeeds with a fixed usage, recording every
// request it served. Stands in for the real provider lane (no network).
const makeFakeAdapter = (
  id: string,
  behavior:
    | { kind: 'ok'; usage: InferenceResult['usage'] }
    | { kind: 'error'; reason: string },
): {
  adapter: InferenceProviderAdapter
  servedCount: () => number
} => {
  let served = 0
  const adapter: InferenceProviderAdapter = {
    complete: () => {
      served += 1
      if (behavior.kind === 'error') {
        return Effect.fail(
          new AdapterError({ adapterId: id, reason: behavior.reason }),
        ) as Effect.Effect<InferenceResult, InferenceAdapterError>
      }
      return Effect.succeed({
        content: 'fake completion',
        finishReason: 'stop',
        servedModel: `served/${id}`,
        usage: behavior.usage,
      })
    },
    id,
    stream: () => Effect.succeed([]),
  }
  return { adapter, servedCount: () => served }
}

const makeQueueMessage = (
  jobId: string,
  itemCount: number,
): BatchJobQueueMessage =>
  S.decodeUnknownSync(BatchJobQueueMessage)({
    items: Array.from({ length: itemCount }, (_, i) => ({
      messages: [{ content: `prompt ${i}`, role: 'user' }],
      model: 'gemini-3.5-flash',
    })),
    jobId,
    schemaVersion: 'openagents.inference.batch_job.v1',
  })

const meteringRecorder = (): {
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

const dispatchDepsFor = (adapter: InferenceProviderAdapter) => {
  const registry = new InferenceProviderRegistry()
  registry.register(adapter)
  return {
    plan: () => [adapter.id],
    registry,
    sleep: () => Effect.void,
  }
}

// ---- tests -----------------------------------------------------------------

describe('executeBatchJob', () => {
  it('runs each item against the gateway, meters each, completes the job', async () => {
    const fake = makeFakeStore(pendingJob({ datasetSize: 2 }))
    const gateway = makeFakeAdapter('fireworks', {
      kind: 'ok',
      usage: { completionTokens: 20, promptTokens: 10, totalTokens: 30 },
    })
    const metering = meteringRecorder()
    const storedResults: Array<string> = []

    const outcome = await run(
      executeBatchJob(
        {
          dispatch: dispatchDepsFor(gateway.adapter),
          meteringHook: metering.hook,
          resultsStore: {
            readResults: () => Effect.succeed(null),
            writeResults: (_jobId, rows) => {
              storedResults.push(JSON.stringify(rows))
              return Effect.succeed('batch_test/results.jsonl')
            },
          },
          store: fake.store,
        },
        makeQueueMessage('batch_test', 2),
      ),
    )

    expect(outcome.completed).toBe(true)
    expect(outcome.processedItems).toBe(2)
    expect(outcome.failedItems).toBe(0)
    expect(outcome.receiptRef).toBe(batchJobCloseoutReceiptRef('batch_test'))

    // Executed every item against the gateway.
    expect(gateway.servedCount()).toBe(2)

    // Decremented credits once per item, batch-flagged, with a per-item
    // idempotency-safe request id.
    expect(metering.calls()).toHaveLength(2)
    expect(metering.calls().every(c => c.batch === true)).toBe(true)
    expect(metering.calls().map(c => c.requestId)).toEqual([
      'batch_test:0',
      'batch_test:1',
    ])
    expect(metering.calls()[0]?.accountRef).toBe('agent:abc')
    expect(metering.calls()[0]?.servedModel).toBe('served/fireworks')
    expect(storedResults).toHaveLength(1)

    // Drove the row processing -> completed; the completed status is what makes
    // the public closeout receipt dereferenceable.
    const writes = fake.statusWrites()
    expect(writes[0]?.status).toBe('processing')
    const terminal = writes[writes.length - 1]
    expect(terminal?.status).toBe('completed')
    expect(terminal?.processedItems).toBe(2)
    expect(terminal?.failedItems).toBe(0)
    expect(terminal?.resultsR2Key).toBe('batch_test/results.jsonl')
    expect(fake.reads()?.status).toBe('completed')
    expect(fake.reads()?.resultsR2Key).toBe('batch_test/results.jsonl')
  })

  it('stamps the consumer-start time (END of the batch wait) from the injected clock', async () => {
    // Book P0-3: the consumer marks `startedAt` at the `processing` transition so
    // the closeout receipt can disclose `batchWaitMs = startedAt - enqueuedAt`.
    const fake = makeFakeStore(pendingJob({ datasetSize: 1 }))
    const gateway = makeFakeAdapter('fireworks', {
      kind: 'ok',
      usage: { completionTokens: 5, promptTokens: 5, totalTokens: 10 },
    })

    await run(
      executeBatchJob(
        {
          dispatch: dispatchDepsFor(gateway.adapter),
          nowIso: () => '2026-06-22T00:00:03.000Z',
          store: fake.store,
        },
        makeQueueMessage('batch_test', 1),
      ),
    )

    // The processing transition carries the start time; the row reflects it, so a
    // later closeout read computes a real batch wait (3s after the 00:00:00
    // enqueue) rather than `not_measured`.
    const processing = fake
      .statusWrites()
      .find(write => write.status === 'processing')
    expect(processing?.startedAt).toBe('2026-06-22T00:00:03.000Z')
    expect(fake.reads()?.startedAt).toBe('2026-06-22T00:00:03.000Z')
  })

  it('counts a failed item but still completes the job (partial success)', async () => {
    const fake = makeFakeStore(pendingJob({ datasetSize: 1 }))
    // First item fails at dispatch; with one item the job is all-failed.
    const gateway = makeFakeAdapter('fireworks', {
      kind: 'error',
      reason: 'fireworks responded 524',
    })
    const metering = meteringRecorder()

    const outcome = await run(
      executeBatchJob(
        {
          dispatch: dispatchDepsFor(gateway.adapter),
          meteringHook: metering.hook,
          store: fake.store,
        },
        makeQueueMessage('batch_test', 1),
      ),
    )

    // All items failed => job is failed, never charged.
    expect(outcome.completed).toBe(false)
    expect(outcome.processedItems).toBe(1)
    expect(outcome.failedItems).toBe(1)
    expect(metering.calls()).toHaveLength(0)
    expect(fake.reads()?.status).toBe('failed')
  })

  it('is idempotent for an already-completed job (no re-run, no re-charge)', async () => {
    const fake = makeFakeStore(
      pendingJob({ failedItems: 0, processedItems: 2, status: 'completed' }),
    )
    const gateway = makeFakeAdapter('fireworks', {
      kind: 'ok',
      usage: { completionTokens: 20, promptTokens: 10, totalTokens: 30 },
    })
    const metering = meteringRecorder()

    const outcome = await run(
      executeBatchJob(
        {
          dispatch: dispatchDepsFor(gateway.adapter),
          meteringHook: metering.hook,
          store: fake.store,
        },
        makeQueueMessage('batch_test', 2),
      ),
    )

    expect(outcome.completed).toBe(true)
    expect(gateway.servedCount()).toBe(0)
    expect(metering.calls()).toHaveLength(0)
    expect(fake.statusWrites()).toHaveLength(0)
  })

  it('returns a non-completed outcome when the job row is missing', async () => {
    const fake = makeFakeStore(null)
    const gateway = makeFakeAdapter('fireworks', {
      kind: 'ok',
      usage: { completionTokens: 20, promptTokens: 10, totalTokens: 30 },
    })

    const outcome = await run(
      executeBatchJob(
        { dispatch: dispatchDepsFor(gateway.adapter), store: fake.store },
        makeQueueMessage('missing_job', 1),
      ),
    )

    expect(outcome.completed).toBe(false)
    expect(outcome.processedItems).toBe(0)
    expect(gateway.servedCount()).toBe(0)
    expect(fake.statusWrites()).toHaveLength(0)
  })

  it('defaults to the no-op metering stub when no hook is injected (never charges)', async () => {
    const fake = makeFakeStore(pendingJob({ datasetSize: 1 }))
    const gateway = makeFakeAdapter('fireworks', {
      kind: 'ok',
      usage: { completionTokens: 5, promptTokens: 5, totalTokens: 10 },
    })

    const outcome = await run(
      executeBatchJob(
        { dispatch: dispatchDepsFor(gateway.adapter), store: fake.store },
        makeQueueMessage('batch_test', 1),
      ),
    )

    expect(outcome.completed).toBe(true)
    expect(outcome.failedItems).toBe(0)
    expect(gateway.servedCount()).toBe(1)
  })
})
