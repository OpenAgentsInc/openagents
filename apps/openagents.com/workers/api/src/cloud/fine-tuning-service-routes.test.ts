import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type FineTuningAuth,
  type FineTuningMeteringHook,
  type FineTuningRuntimeAdapter,
  type FineTuningServiceDeps,
  FineTuningAdapterError,
  fineTuningJobReceiptRef,
  handleFineTuningJobSubmit,
  isFineTuningServiceEnabled,
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
    expect(body.receipt_ref).toBe('receipt.cloud.fine_tuning.job.ftjob_fixed')
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
