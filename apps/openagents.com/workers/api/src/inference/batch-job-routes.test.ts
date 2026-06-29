import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { inferenceBatchJobChargeReceiptRef } from './batch-job-metering'
import type {
  BatchJobResultStore,
  BatchJobResultsPayload,
} from './batch-job-results'
import {
  type BatchJobRoutesDeps,
  handleBatchJobReceiptRead,
  handleBatchJobResultsRead,
  handleBatchJobStatusRead,
  handleBatchJobsSubmit,
} from './batch-job-routes'

const run = <A>(effect: Effect.Effect<A, never, never>): Promise<A> =>
  Effect.runPromise(effect)

const makeMockDb = (ok: boolean): D1Database =>
  ({
    batch: async () => [],
    prepare: () => ({
      bind: () => ({
        first: async () => (ok ? null : { id: 'fail' }),
        run: async () => ({ success: true }),
      }),
    }),
  }) as unknown as D1Database

const baseDeps = (
  overrides: Partial<BatchJobRoutesDeps> = {},
): BatchJobRoutesDeps => ({
  authenticate: async () => ({ accountRef: 'agent:123' }),
  db: makeMockDb(true),
  enabled: true,
  nowIso: () => '2026-06-20T12:00:00.000Z',
  ...overrides,
})

const makeRequest = (body: unknown): Request =>
  new Request('https://openagents.com/v1/inference/batches', {
    body: JSON.stringify(body),
    method: 'POST',
  })

const resultStoreFor = (
  payloads: Readonly<Record<string, BatchJobResultsPayload>>,
): BatchJobResultStore => ({
  getResults: key => Effect.succeed(payloads[key] ?? null),
  putResults: payload => Effect.succeed(`memory://${payload.jobId}`),
})

describe('handleBatchJobsSubmit', () => {
  it('returns 404 when disabled', async () => {
    const response = await run(
      handleBatchJobsSubmit(makeRequest({}), baseDeps({ enabled: false })),
    )
    expect(response.status).toBe(404)
  })

  it('returns 405 for non-POST requests', async () => {
    const request = new Request('https://openagents.com/v1/inference/batches', {
      method: 'GET',
    })
    const response = await run(handleBatchJobsSubmit(request, baseDeps()))
    expect(response.status).toBe(405)
  })

  it('returns 401 when unauthenticated', async () => {
    const deps = baseDeps({ authenticate: async () => undefined })
    const response = await run(handleBatchJobsSubmit(makeRequest({}), deps))
    expect(response.status).toBe(401)
  })

  it('returns 400 for invalid JSON', async () => {
    const request = new Request('https://openagents.com/v1/inference/batches', {
      body: 'invalid-json',
      method: 'POST',
    })
    const response = await run(handleBatchJobsSubmit(request, baseDeps()))
    expect(response.status).toBe(400)
  })

  it('returns 400 for invalid schema', async () => {
    const response = await run(
      handleBatchJobsSubmit(
        makeRequest({ dataset: [{ foo: 'bar' }] }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
  })

  it('accepts a valid request and returns a receipt', async () => {
    const payload = {
      dataset: [
        {
          completionTokens: 50,
          model: 'gemini-3.5-flash',
          promptTokens: 100,
        },
      ],
    }

    const response = await run(
      handleBatchJobsSubmit(makeRequest(payload), baseDeps()),
    )

    expect(response.status).toBe(202)

    const body = (await response.json()) as any
    expect(body.status).toBe('accepted')
    expect(body.jobId).toMatch(/^batch_/)
    expect(body.receiptRef).toBe(inferenceBatchJobChargeReceiptRef(body.jobId))
    expect(typeof body.totalCostMsat).toBe('number')
  })

  it('returns 402 if charge fails (insufficient funds)', async () => {
    const deps = baseDeps({
      db: makeMockDb(false),
    })
    // Let's force an error in makeMockDb's batch to simulate ledger run failure
    const mockDbFail = {
      batch: async () => {
        throw new Error('ledger error')
      },
      prepare: () => ({
        bind: () => ({
          first: async () => null, // so already null
        }),
      }),
    } as unknown as D1Database

    const payload = {
      dataset: [
        {
          completionTokens: 500,
          model: 'gemini-3.5-flash',
          promptTokens: 1000,
        },
      ],
    }

    const response = await run(
      handleBatchJobsSubmit(makeRequest(payload), { ...deps, db: mockDbFail }),
    )

    expect(response.status).toBe(402)
    const body = (await response.json()) as any
    expect(body.error).toBe('insufficient_funds')
  })
})

describe('handleBatchJobReceiptRead', () => {
  const makeReadDb = (jobStatus: string | null): D1Database => {
    return {
      prepare: (sql: string) => ({
        bind: () => ({
          first: async () => {
            if (sql.includes('inference_batch_jobs')) {
              if (jobStatus === null) return null
              return {
                job_id: 'batch_123',
                account_ref: 'agent:123',
                status: jobStatus,
                charge_receipt_ref:
                  'receipt.inference.batch_job_charge.batch_123',
                dataset_size: 100,
                processed_items: 99,
                failed_items: 1,
                results_r2_key: 'batch_123/results.jsonl',
                created_at: '2026-06-20T12:00:00.000Z',
                updated_at: '2026-06-20T12:05:00.000Z',
                // Book P0-3: enqueue -> consumer-start timing for the batch wait.
                enqueued_at: '2026-06-20T12:00:00.000Z',
                started_at: '2026-06-20T12:00:30.000Z',
              }
            }
            if (sql.includes('pay_ins')) {
              return { cost_msat: 50000 }
            }
            return null
          },
        }),
      }),
    } as unknown as D1Database
  }

  const makeReadRequest = (receiptRef: string): Request =>
    new Request(
      `https://openagents.com/api/public/inference/batch-job-receipts/${receiptRef}`,
      {
        method: 'GET',
      },
    )

  it('returns 404 when disabled', async () => {
    const response = await run(
      handleBatchJobReceiptRead(
        makeReadRequest('receipt.inference.batch_job.closeout.batch_123'),
        baseDeps({ enabled: false }),
      ),
    )
    expect(response.status).toBe(404)
  })

  it('returns 405 for non-GET requests', async () => {
    const request = new Request(
      'https://openagents.com/api/public/inference/batch-job-receipts/receipt.inference.batch_job.closeout.batch_123',
      {
        method: 'POST',
      },
    )
    const response = await run(handleBatchJobReceiptRead(request, baseDeps()))
    expect(response.status).toBe(405)
  })

  it('returns 404 for invalid prefix', async () => {
    const response = await run(
      handleBatchJobReceiptRead(
        makeReadRequest('invalid-prefix.batch_123'),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(404)
  })

  it('returns 404 if job not found', async () => {
    const deps = baseDeps({ db: makeReadDb(null) })
    const response = await run(
      handleBatchJobReceiptRead(
        makeReadRequest('receipt.inference.batch_job.closeout.batch_123'),
        deps,
      ),
    )
    expect(response.status).toBe(404)
  })

  it('returns 404 if job is pending', async () => {
    const deps = baseDeps({ db: makeReadDb('pending') })
    const response = await run(
      handleBatchJobReceiptRead(
        makeReadRequest('receipt.inference.batch_job.closeout.batch_123'),
        deps,
      ),
    )
    expect(response.status).toBe(404)
  })

  it('returns projected closeout receipt for completed job', async () => {
    const deps = baseDeps({ db: makeReadDb('completed') })
    const response = await run(
      handleBatchJobReceiptRead(
        makeReadRequest('receipt.inference.batch_job.closeout.batch_123'),
        deps,
      ),
    )

    expect(response.status).toBe(200)

    const body = (await response.json()) as any
    expect(body.receipt.jobId).toBe('batch_123')
    expect(body.receipt.totalItems).toBe(100)
    expect(body.receipt.successfulItems).toBe(98)
    expect(body.receipt.failedItems).toBe(1)
    expect(body.receipt.totalCostMsat).toBe(50000)
    expect(body.staleness.composition).toBe('live_at_read')

    // Book P0-3: the route attaches the TERMINAL `openagents` telemetry record so
    // the detached job is auditable — distinguishable from an interactive stream
    // (`requestClass: batch`), with a measured zero edge wait and the real
    // in-queue batch wait (12:00:00 enqueue -> 12:00:30 start = 30000ms).
    expect(body.receipt.openagents.requestClass).toBe('batch')
    expect(body.receipt.openagents.queueWaitMs).toBe(0)
    expect(body.receipt.openagents.batchWaitMs).toBe(30000)
  })

  it('reports batchWaitMs not_measured when timing is unavailable (no fabrication)', async () => {
    // A completed job with no enqueue/start timing (e.g. submitted before the
    // timing columns existed) must honestly report not_measured, never a fake 0.
    const db = {
      prepare: (sql: string) => ({
        bind: () => ({
          first: async () => {
            if (sql.includes('inference_batch_jobs')) {
              return {
                job_id: 'batch_123',
                account_ref: 'agent:123',
                status: 'completed',
                charge_receipt_ref:
                  'receipt.inference.batch_job_charge.batch_123',
                dataset_size: 100,
                processed_items: 99,
                failed_items: 1,
                results_r2_key: 'batch_123/results.jsonl',
                created_at: '2026-06-20T12:00:00.000Z',
                updated_at: '2026-06-20T12:05:00.000Z',
                enqueued_at: null,
                started_at: null,
              }
            }
            if (sql.includes('pay_ins')) {
              return { cost_msat: 50000 }
            }
            return null
          },
        }),
      }),
    } as unknown as D1Database
    const response = await run(
      handleBatchJobReceiptRead(
        makeReadRequest('receipt.inference.batch_job.closeout.batch_123'),
        baseDeps({ db }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as any
    expect(body.receipt.openagents.requestClass).toBe('batch')
    expect(body.receipt.openagents.batchWaitMs).toBe('not_measured')
    expect(body.receipt.openagents.blockerRefs).toContain(
      'batch_wait_not_measured',
    )
  })
})

describe('handleBatchJobStatusRead', () => {
  const makeReadDb = (
    jobStatus: string | null,
    accountRef = 'agent:123',
  ): D1Database => {
    return {
      prepare: (sql: string) => ({
        bind: () => ({
          first: async () => {
            if (sql.includes('inference_batch_jobs')) {
              if (jobStatus === null) return null
              return {
                job_id: 'batch_123',
                account_ref: accountRef,
                status: jobStatus,
                charge_receipt_ref:
                  'receipt.inference.batch_job_charge.batch_123',
                dataset_size: 100,
                processed_items: 99,
                failed_items: 1,
                results_r2_key: 'batch_123/results.jsonl',
                created_at: '2026-06-20T12:00:00.000Z',
                updated_at: '2026-06-20T12:05:00.000Z',
                enqueued_at: '2026-06-20T12:00:00.000Z',
                started_at: '2026-06-20T12:00:45.000Z',
              }
            }
            return null
          },
        }),
      }),
    } as unknown as D1Database
  }

  const makeStatusRequest = (jobId: string): Request =>
    new Request(`https://openagents.com/v1/inference/batches/${jobId}`, {
      method: 'GET',
    })

  it('returns 404 when disabled', async () => {
    const response = await run(
      handleBatchJobStatusRead(
        makeStatusRequest('batch_123'),
        baseDeps({ enabled: false }),
      ),
    )
    expect(response.status).toBe(404)
  })

  it('returns 405 for non-GET requests', async () => {
    const request = new Request(
      'https://openagents.com/v1/inference/batches/batch_123',
      {
        method: 'POST',
      },
    )
    const response = await run(handleBatchJobStatusRead(request, baseDeps()))
    expect(response.status).toBe(405)
  })

  it('returns 401 when unauthenticated', async () => {
    const deps = baseDeps({ authenticate: async () => undefined })
    const response = await run(
      handleBatchJobStatusRead(makeStatusRequest('batch_123'), deps),
    )
    expect(response.status).toBe(401)
  })

  it('returns 404 if job not found', async () => {
    const deps = baseDeps({ db: makeReadDb(null) })
    const response = await run(
      handleBatchJobStatusRead(makeStatusRequest('batch_123'), deps),
    )
    expect(response.status).toBe(404)
  })

  it('returns 404 if job accountRef does not match session', async () => {
    const deps = baseDeps({ db: makeReadDb('pending', 'agent:other') })
    const response = await run(
      handleBatchJobStatusRead(makeStatusRequest('batch_123'), deps),
    )
    expect(response.status).toBe(404)
  })

  it('returns job status for pending job', async () => {
    const deps = baseDeps({ db: makeReadDb('pending') })
    const response = await run(
      handleBatchJobStatusRead(makeStatusRequest('batch_123'), deps),
    )

    expect(response.status).toBe(200)

    const body = (await response.json()) as any
    expect(body.jobId).toBe('batch_123')
    expect(body.status).toBe('pending')
    expect(body.datasetSize).toBe(100)
    expect(body.processedItems).toBe(99)
    expect(body.failedItems).toBe(1)
    // Book P0-3: the status poll exposes the batch wait so long-running detached
    // work is auditable without waiting for the closeout receipt (12:00:00
    // enqueue -> 12:00:45 start = 45000ms).
    expect(body.enqueuedAt).toBe('2026-06-20T12:00:00.000Z')
    expect(body.startedAt).toBe('2026-06-20T12:00:45.000Z')
    expect(body.batchWaitMs).toBe(45000)
  })
})

describe('handleBatchJobResultsRead', () => {
  const makeReadDb = (
    jobStatus: string | null,
    accountRef = 'agent:123',
    resultsR2Key: string | null = 'batch_123/results.json',
  ): D1Database => {
    return {
      prepare: (sql: string) => ({
        bind: () => ({
          first: async () => {
            if (sql.includes('inference_batch_jobs')) {
              if (jobStatus === null) return null
              return {
                account_ref: accountRef,
                charge_receipt_ref:
                  'receipt.inference.batch_job_charge.batch_123',
                created_at: '2026-06-20T12:00:00.000Z',
                dataset_size: 2,
                enqueued_at: '2026-06-20T12:00:00.000Z',
                failed_items: 0,
                job_id: 'batch_123',
                processed_items: 2,
                results_r2_key: resultsR2Key,
                started_at: '2026-06-20T12:00:45.000Z',
                status: jobStatus,
                updated_at: '2026-06-20T12:05:00.000Z',
              }
            }
            return null
          },
        }),
      }),
    } as unknown as D1Database
  }

  const makeResultsRequest = (jobId: string): Request =>
    new Request(
      `https://openagents.com/v1/inference/batches/${jobId}/results`,
      {
        method: 'GET',
      },
    )

  const payload: BatchJobResultsPayload = {
    jobId: 'batch_123',
    results: [
      {
        content: 'classified as urgent',
        finishReason: 'stop',
        index: 0,
        model: 'gemini-3.5-flash',
        servedModel: 'served/fireworks',
        status: 'succeeded',
        usage: {
          completionTokens: 8,
          promptTokens: 12,
          totalTokens: 20,
        },
      },
      {
        content: 'summary text',
        finishReason: 'stop',
        index: 1,
        model: 'gemini-3.5-flash',
        servedModel: 'served/fireworks',
        status: 'succeeded',
        usage: {
          completionTokens: 11,
          promptTokens: 15,
          totalTokens: 26,
        },
      },
    ],
    schemaVersion: 'openagents.inference.batch_job.results.v1',
  }

  it('returns authenticated batch results for the owning account', async () => {
    const deps = baseDeps({
      db: makeReadDb('completed'),
      resultStore: resultStoreFor({ 'batch_123/results.json': payload }),
    })
    const response = await run(
      handleBatchJobResultsRead(makeResultsRequest('batch_123'), deps),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as any
    expect(body.jobId).toBe('batch_123')
    expect(body.resultsR2Key).toBe('batch_123/results.json')
    expect(body.results).toHaveLength(2)
    expect(body.results[0].content).toBe('classified as urgent')
  })

  it('returns 409 before results are written', async () => {
    const deps = baseDeps({
      db: makeReadDb('processing', 'agent:123', null),
      resultStore: resultStoreFor({}),
    })
    const response = await run(
      handleBatchJobResultsRead(makeResultsRequest('batch_123'), deps),
    )

    expect(response.status).toBe(409)
    const body = (await response.json()) as any
    expect(body.error).toBe('batch_results_not_ready')
    expect(body.status).toBe('processing')
  })

  it('returns 404 when the job belongs to another account', async () => {
    const deps = baseDeps({
      db: makeReadDb('completed', 'agent:other'),
      resultStore: resultStoreFor({ 'batch_123/results.json': payload }),
    })
    const response = await run(
      handleBatchJobResultsRead(makeResultsRequest('batch_123'), deps),
    )

    expect(response.status).toBe(404)
  })
})
