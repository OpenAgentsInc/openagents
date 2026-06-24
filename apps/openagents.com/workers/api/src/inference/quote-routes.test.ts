import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { estimateBudgetCapacity } from './budget-estimate'
import { estimateRequestCost } from './cost-estimate'
import {
  ALL_LANES_UNARMED,
  type SupplyLaneArming,
  resolveSupplyLaneArming,
} from './model-serving-policy'
import { KHALA_MODEL_ID } from './pricing'
import { handleQuote } from './quote-routes'

const run = (request: Request, deps: Parameters<typeof handleQuote>[1]) =>
  Effect.runPromise(handleQuote(request, deps))

const servedModel = KHALA_MODEL_ID

const post = (body: unknown): Request =>
  new Request('https://x/v1/quote', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('handleQuote', () => {
  it('404s when the gateway is disabled (inert posture)', async () => {
    const response = await run(
      post({ completionTokens: 1, model: servedModel, promptTokens: 1 }),
      { enabled: false },
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'inference_gateway_disabled',
    })
  })

  it('405s on non-POST methods', async () => {
    const response = await run(
      new Request('https://x/v1/quote', { method: 'GET' }),
      { enabled: true },
    )
    expect(response.status).toBe(405)
  })

  it('400s on invalid JSON', async () => {
    const response = await run(
      new Request('https://x/v1/quote', {
        body: 'not json',
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      { enabled: true },
    )
    expect(response.status).toBe(400)
    expect(await response.json()).toEqual({ error: 'invalid_json' })
  })

  it('400s on a missing required field', async () => {
    const response = await run(post({ model: servedModel }), { enabled: true })
    expect(response.status).toBe(400)
    expect(await response.json()).toEqual({ error: 'invalid_request' })
  })

  it('400s on an out-of-range fundingKind', async () => {
    const response = await run(
      post({
        completionTokens: 1,
        fundingKind: 'paypal',
        model: servedModel,
        promptTokens: 1,
      }),
      { enabled: true },
    )
    expect(response.status).toBe(400)
  })

  it('serves a no-store quote matching the pure estimator exactly', async () => {
    const input = {
      completionTokens: 500,
      fundingKind: 'card' as const,
      model: servedModel,
      promptTokens: 1_000,
    }
    const response = await run(post(input), { enabled: true })
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = await response.json()
    // The route is a thin surface over the pure estimator: byte-for-byte equal.
    expect(body).toEqual(estimateRequestCost(input))
    expect((body as { isEstimate: boolean }).isEstimate).toBe(true)
  })

  it('defaults an omitted fundingKind to the conservative card rail', async () => {
    const response = await run(
      post({ completionTokens: 500, model: servedModel, promptTokens: 1_000 }),
      { enabled: true },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { fundingKind: string }
    expect(body.fundingKind).toBe('card')
  })

  it('surfaces the Bitcoin-rail saving versus card on the same request', async () => {
    const base = {
      completionTokens: 100_000,
      model: servedModel,
      promptTokens: 100_000,
    }
    const btc = (await (
      await run(post({ ...base, fundingKind: 'bitcoin' }), { enabled: true })
    ).json()) as { fundingDiscountUsd: number; estimatedChargeUsd: number }
    const card = (await (
      await run(post({ ...base, fundingKind: 'card' }), { enabled: true })
    ).json()) as { fundingDiscountUsd: number; estimatedChargeUsd: number }
    expect(card.fundingDiscountUsd).toBe(0)
    expect(btc.fundingDiscountUsd).toBeGreaterThan(0)
    expect(btc.estimatedChargeUsd).toBeLessThan(card.estimatedChargeUsd)
  })

  it('serves a budget (affordability) quote when budgetCredits is supplied', async () => {
    const input = {
      budgetCredits: 5_000,
      completionTokens: 500,
      fundingKind: 'card' as const,
      model: servedModel,
      promptTokens: 1_000,
    }
    const response = await run(post(input), { enabled: true })
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = await response.json()
    // Thin surface over the pure budget estimator: byte-for-byte equal.
    expect(body).toEqual(estimateBudgetCapacity(input))
    const typed = body as {
      affordableRequests: number
      perRequest: { isEstimate: boolean }
      isEstimate: boolean
    }
    expect(typed.isEstimate).toBe(true)
    // The per-request estimate is still embedded for token-mode clients.
    expect(typed.perRequest.isEstimate).toBe(true)
    expect(typed.affordableRequests).toBeGreaterThan(0)
  })

  it('omitting budgetCredits keeps the per-request quote shape (backward compatible)', async () => {
    const input = {
      completionTokens: 500,
      fundingKind: 'card' as const,
      model: servedModel,
      promptTokens: 1_000,
    }
    const response = await run(post(input), { enabled: true })
    const body = (await response.json()) as Record<string, unknown>
    expect(body).toEqual(estimateRequestCost(input))
    expect(body.affordableRequests).toBeUndefined()
  })

  it('clamps sloppy (negative/fractional) token inputs to a safe quote', async () => {
    const response = await run(
      post({
        completionTokens: -5,
        model: servedModel,
        promptTokens: 10.7,
      }),
      { enabled: true },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      promptTokens: number
      completionTokens: number
      estimatedChargeUsd: number
    }
    expect(body.promptTokens).toBe(10)
    expect(body.completionTokens).toBe(0)
    expect(body.estimatedChargeUsd).toBeGreaterThanOrEqual(0)
  })
})

describe('handleQuote provider serving policy gate', () => {
  const hydraliskArmed: SupplyLaneArming = {
    ...ALL_LANES_UNARMED,
    hydralisk: true,
  }

  it('quotes Khala when its Hydralisk backing lane is armed (200)', async () => {
    const response = await run(
      post({
        completionTokens: 100,
        model: KHALA_MODEL_ID,
        promptTokens: 100,
      }),
      { enabled: true, laneArming: hydraliskArmed },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { isEstimate: boolean }
    expect(body.isEstimate).toBe(true)
  })

  it('quotes Khala against the Fireworks DeepSeek backing when that lane is selected', async () => {
    const laneArming = resolveSupplyLaneArming({
      FIREWORKS_API_KEY: 'fw',
      KHALA_BACKING_MODEL: 'deepseek-v4-flash',
    })
    const input = {
      completionTokens: 1_000_000,
      fundingKind: 'card' as const,
      model: KHALA_MODEL_ID,
      promptTokens: 1_000_000,
    }
    const response = await run(post(input), { enabled: true, laneArming })
    expect(response.status).toBe(200)
    const body = await response.json()
    expect(body).toEqual(
      estimateRequestCost({
        ...input,
        priceModel: 'deepseek-v4-flash',
      }),
    )
    expect((body as { model: string }).model).toBe(KHALA_MODEL_ID)
  })

  it('refuses a KNOWN model whose lane is NOT armed (404 model_unavailable)', async () => {
    const response = await run(
      post({ completionTokens: 100, model: 'gpt-oss-20b', promptTokens: 100 }),
      { enabled: true, laneArming: hydraliskArmed },
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'model_unavailable',
      model: 'gpt-oss-20b',
    })
  })

  it('refuses regardless of casing (gate cannot be bypassed by model-id case)', async () => {
    const response = await run(
      post({ completionTokens: 1, model: 'GPT-OSS-20B', promptTokens: 1 }),
      { enabled: true, laneArming: hydraliskArmed },
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'model_unavailable',
      model: 'gpt-oss-20b',
    })
  })

  it('rejects an unknown model id instead of falling back to generic pricing', async () => {
    const response = await run(
      post({
        completionTokens: 100,
        model: 'totally-made-up-model',
        promptTokens: 100,
      }),
      { enabled: true, laneArming: ALL_LANES_UNARMED },
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'model_unavailable',
      model: 'totally-made-up-model',
    })
  })

  it('omitting laneArming still refuses raw GPT-OSS', async () => {
    const response = await run(
      post({ completionTokens: 100, model: 'gpt-oss-20b', promptTokens: 100 }),
      { enabled: true },
    )
    expect(response.status).toBe(404)
  })

  it('normalizes the internal khala slug to the external quote id', async () => {
    const response = await run(
      post({ completionTokens: 100, model: 'khala', promptTokens: 100 }),
      { enabled: true },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { model: string }
    expect(body.model).toBe(KHALA_MODEL_ID)
  })
})
