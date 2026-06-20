import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type BatchJobRoutesDeps,
  handleBatchJobReceiptRead,
  handleBatchJobsSubmit,
} from './batch-job-routes'
import { inferenceBatchJobChargeReceiptRef } from './batch-job-metering'

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
      handleBatchJobsSubmit(makeRequest({ dataset: [{ foo: 'bar' }] }), baseDeps()),
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

    expect(response.status).toBe(200)

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
                charge_receipt_ref: 'receipt.inference.batch_job_charge.batch_123',
                dataset_size: 100,
                processed_items: 99,
                failed_items: 1,
                results_r2_key: 'batch_123/results.jsonl',
                created_at: '2026-06-20T12:00:00.000Z',
                updated_at: '2026-06-20T12:05:00.000Z',
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
    new Request(`https://openagents.com/api/public/inference/batch-job-receipts/${receiptRef}`, {
      method: 'GET',
    })

  it('returns 404 when disabled', async () => {
    const response = await run(
      handleBatchJobReceiptRead(makeReadRequest('receipt.inference.batch_job.closeout.batch_123'), baseDeps({ enabled: false })),
    )
    expect(response.status).toBe(404)
  })

  it('returns 405 for non-GET requests', async () => {
    const request = new Request('https://openagents.com/api/public/inference/batch-job-receipts/receipt.inference.batch_job.closeout.batch_123', {
      method: 'POST',
    })
    const response = await run(handleBatchJobReceiptRead(request, baseDeps()))
    expect(response.status).toBe(405)
  })

  it('returns 404 for invalid prefix', async () => {
    const response = await run(
      handleBatchJobReceiptRead(makeReadRequest('invalid-prefix.batch_123'), baseDeps()),
    )
    expect(response.status).toBe(404)
  })

  it('returns 404 if job not found', async () => {
    const deps = baseDeps({ db: makeReadDb(null) })
    const response = await run(
      handleBatchJobReceiptRead(makeReadRequest('receipt.inference.batch_job.closeout.batch_123'), deps),
    )
    expect(response.status).toBe(404)
  })

  it('returns 404 if job is pending', async () => {
    const deps = baseDeps({ db: makeReadDb('pending') })
    const response = await run(
      handleBatchJobReceiptRead(makeReadRequest('receipt.inference.batch_job.closeout.batch_123'), deps),
    )
    expect(response.status).toBe(404)
  })

  it('returns projected closeout receipt for completed job', async () => {
    const deps = baseDeps({ db: makeReadDb('completed') })
    const response = await run(
      handleBatchJobReceiptRead(makeReadRequest('receipt.inference.batch_job.closeout.batch_123'), deps),
    )

    expect(response.status).toBe(200)

    const body = (await response.json()) as any
    expect(body.receipt.jobId).toBe('batch_123')
    expect(body.receipt.totalItems).toBe(100)
    expect(body.receipt.successfulItems).toBe(98)
    expect(body.receipt.failedItems).toBe(1)
    expect(body.receipt.totalCostMsat).toBe(50000)
    expect(body.staleness.composition).toBe('live_at_read')
  })
})
