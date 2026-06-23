import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type FineTuningAuth,
  type FineTuningMeteringHook,
  type FineTuningRuntimeAdapter,
  type FineTuningServiceDeps,
  FineTuningAdapterError,
  fineTuningJobReceiptRef,
  handleFineTuningJobGet,
  handleFineTuningJobSubmit,
  isFineTuningServiceEnabled,
  makeLedgerFineTuningMeteringHook,
  stubFineTuningAdapter,
} from './fine-tuning-service-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const authOk: FineTuningAuth = async () => ({ accountRef: 'agent:test-user' })
const authNone: FineTuningAuth = async () => undefined

const baseDeps = (
  overrides: Partial<FineTuningServiceDeps> = {},
): FineTuningServiceDeps => ({
  authenticate: authOk,
  enabled: true,
  newId: () => 'ftjob_fixed',
  ...overrides,
})

const jobRequest = (body: unknown, init: RequestInit = {}): Request =>
  new Request('https://openagents.com/v1/fine_tuning/jobs', {
    body: JSON.stringify(body),
    method: 'POST',
    ...init,
  })

const validBody = {
  baseModel: 'gemini-3.5-flash',
  datasetRef: 'dataset:abc123',
  suffix: 'my-tune',
}

describe('fine-tuning service feature flag', () => {
  test('defaults off and only enables on explicit truthy tokens', () => {
    expect(isFineTuningServiceEnabled(undefined)).toBe(false)
    expect(isFineTuningServiceEnabled('')).toBe(false)
    expect(isFineTuningServiceEnabled('false')).toBe(false)
    expect(isFineTuningServiceEnabled('0')).toBe(false)
    expect(isFineTuningServiceEnabled('true')).toBe(true)
    expect(isFineTuningServiceEnabled('TRUE')).toBe(true)
    expect(isFineTuningServiceEnabled('1')).toBe(true)
    expect(isFineTuningServiceEnabled('on')).toBe(true)
    expect(isFineTuningServiceEnabled('yes')).toBe(true)
  })
})

describe('POST /v1/fine_tuning/jobs', () => {
  test('is inert (404) when the flag is disabled', async () => {
    const response = await run(
      handleFineTuningJobSubmit(jobRequest(validBody), baseDeps({ enabled: false })),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('fine_tuning_service_disabled')
  })

  test('rejects an unauthenticated request with 401 + WWW-Authenticate', async () => {
    const response = await run(
      handleFineTuningJobSubmit(jobRequest(validBody), baseDeps({ authenticate: authNone })),
    )
    expect(response.status).toBe(401)
    expect(response.headers.get('www-authenticate')).toBe('Bearer')
  })

  test('rejects non-POST with 405', async () => {
    const response = await run(
      handleFineTuningJobSubmit(
        new Request('https://openagents.com/v1/fine_tuning/jobs', { method: 'GET' }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(405)
  })

  test('rejects invalid JSON with 400', async () => {
    const response = await run(
      handleFineTuningJobSubmit(
        new Request('https://openagents.com/v1/fine_tuning/jobs', {
          body: 'not json',
          method: 'POST',
        }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe('invalid_json')
  })

  test('rejects a missing baseModel/datasetRef with 400', async () => {
    const response = await run(
      handleFineTuningJobSubmit(jobRequest({ baseModel: '', datasetRef: '' }), baseDeps()),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe('invalid_request')
  })

  test('accepts a valid job through the stub adapter as queued, never servable', async () => {
    const response = await run(
      handleFineTuningJobSubmit(jobRequest(validBody), baseDeps()),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.object).toBe('fine_tuning.job')
    expect(body.id).toBe('ftjob_fixed')
    expect(body.model).toBe('gemini-3.5-flash')
    expect(body.status).toBe('queued')
    // Scaffold never produces a servable model.
    expect(body.fine_tuned_model).toBeNull()
  })

  test('stub metering reports metered:false / null receipt (honest, not live)', async () => {
    const response = await run(
      handleFineTuningJobSubmit(jobRequest(validBody), baseDeps()),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.metered).toBe(false)
    expect(body.receipt_ref).toBeNull()
  })

  test('maps a runtime adapter failure to 502', async () => {
    const failing: FineTuningRuntimeAdapter = {
      id: 'failing',
      submit: () =>
        Effect.fail(new FineTuningAdapterError({ adapterId: 'failing', reason: 'down' })),
      get: () =>
        Effect.fail(new FineTuningAdapterError({ adapterId: 'failing', reason: 'down' })),
    }
    const response = await run(
      handleFineTuningJobSubmit(jobRequest(validBody), baseDeps({ adapter: failing })),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('runtime_error')
    expect(body.reason).toBe('down')
  })

  test('a live metering hook can project a receipt ref', async () => {
    const liveHook: FineTuningMeteringHook = context =>
      Effect.succeed({
        metered: true,
        receiptRef: fineTuningJobReceiptRef(context.jobId),
      })
    const response = await run(
      handleFineTuningJobSubmit(jobRequest(validBody), baseDeps({ meteringHook: liveHook })),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.metered).toBe(true)
    // The advertised receipt ref is the SAME ref the ledger writes and the
    // public receipt route dereferences (cloudChargeReceiptRef shape).
    expect(body.receipt_ref).toBe(
      'receipt.cloud.fine_tuning.job.charge.ftjob_fixed',
    )
  })

  test('the stub adapter never returns a servable model', async () => {
    const job = await run(
      Effect.orDie(
        stubFineTuningAdapter.submit({
          jobId: 'j1',
          accountRef: 'agent:x',
          request: {
            baseModel: 'm',
            datasetRef: 'd',
            suffix: undefined,
            hyperparameters: {},
          },
        }),
      ),
    )
    expect(job.status).toBe('queued')
    expect(job.fineTunedModel).toBeNull()
  })
})

const jobGetRequest = (): Request =>
  new Request('https://openagents.com/v1/fine_tuning/jobs/ftjob_fixed', {
    method: 'GET',
  })

describe('GET /v1/fine_tuning/jobs/:jobId', () => {
  test('is inert (404) when the flag is disabled', async () => {
    const response = await run(
      handleFineTuningJobGet(jobGetRequest(), 'ftjob_fixed', baseDeps({ enabled: false })),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('fine_tuning_service_disabled')
  })

  test('rejects an unauthenticated read with 401', async () => {
    const response = await run(
      handleFineTuningJobGet(jobGetRequest(), 'ftjob_fixed', baseDeps({ authenticate: authNone })),
    )
    expect(response.status).toBe(401)
  })

  test('the stub adapter has no persistence, so a read is 404 not_found', async () => {
    const response = await run(
      handleFineTuningJobGet(jobGetRequest(), 'ftjob_fixed', baseDeps()),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('not_found')
  })

  test('projects a resolved job for the owning account', async () => {
    const adapter: FineTuningRuntimeAdapter = {
      id: 'persisted',
      submit: () => Effect.fail(new FineTuningAdapterError({ adapterId: 'persisted', reason: 'n/a' })),
      get: ({ jobId, accountRef }) =>
        Effect.succeed(
          accountRef === 'agent:test-user'
            ? {
                jobId,
                accountRef,
                baseModel: 'gemini-3.5-flash',
                datasetRef: 'dataset:abc',
                suffix: undefined,
                status: 'running' as const,
                fineTunedModel: null,
                createdAt: '2026-06-19T00:00:00.000Z',
              }
            : undefined,
        ),
    }
    const response = await run(
      handleFineTuningJobGet(jobGetRequest(), 'ftjob_fixed', baseDeps({ adapter })),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.object).toBe('fine_tuning.job')
    expect(body.id).toBe('ftjob_fixed')
    expect(body.status).toBe('running')
  })

  test('enforces cross-account isolation (a job is 404 for a different account)', async () => {
    const adapter: FineTuningRuntimeAdapter = {
      id: 'persisted',
      submit: () => Effect.fail(new FineTuningAdapterError({ adapterId: 'persisted', reason: 'n/a' })),
      // Only the submitting account sees the job; everyone else gets undefined.
      get: ({ accountRef }) =>
        Effect.succeed(accountRef === 'agent:owner' ? ({} as never) : undefined),
    }
    const response = await run(
      handleFineTuningJobGet(jobGetRequest(), 'ftjob_fixed', baseDeps({ adapter })),
    )
    expect(response.status).toBe(404)
  })
})

describe('makeLedgerFineTuningMeteringHook', () => {
  test('reports metered:false at intake (no runtime usage yet)', async () => {
    const hook = makeLedgerFineTuningMeteringHook({
      db: {} as D1Database,
      priceUsd: () => 1,
      usdToMsat: usd => Math.ceil(usd * 1000),
    })
    const outcome = await run(
      hook({ accountRef: 'agent:x', jobId: 'j1', baseModel: 'm' }),
    )
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBeNull()
  })

  test('a zero-usd charge is metered with a zeroCharge receipt and no debit', async () => {
    // db is never touched because the charge rounds to 0 msat.
    const hook = makeLedgerFineTuningMeteringHook({
      db: {} as D1Database,
      priceUsd: () => 0,
      usdToMsat: usd => Math.ceil(usd * 1000),
    })
    const outcome = await run(
      hook({
        accountRef: 'agent:x',
        jobId: 'j1',
        baseModel: 'm',
        usage: { trainedTokens: 1000 },
      }),
    )
    expect(outcome.metered).toBe(true)
    expect(outcome.receiptRef).toBe(fineTuningJobReceiptRef('j1'))
  })
})
