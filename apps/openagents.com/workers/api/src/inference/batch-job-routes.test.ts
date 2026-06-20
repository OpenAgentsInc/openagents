import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type BatchJobRoutesDeps,
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
