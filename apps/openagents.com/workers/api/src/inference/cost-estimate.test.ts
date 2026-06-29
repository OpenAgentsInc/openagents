import { describe, expect, it } from 'vitest'

import { estimateRequestCost } from './cost-estimate'
import {
  BASE_CREDIT_USD,
  BITCOIN_DISCOUNT,
  priceRequest,
  sellPricePerMtok,
} from './pricing'
import { usdToMsatCeil } from './usd-msat-conversion'

describe('estimateRequestCost', () => {
  it('matches the metering pricing engine for the same token usage', () => {
    const usage = {
      cachedPromptTokens: 0,
      completionTokens: 500,
      promptTokens: 1000,
      totalTokens: 1500,
    }
    const priced = priceRequest({
      fundingKind: 'card',
      model: 'sonnet',
      usage,
    })
    const estimate = estimateRequestCost({
      completionTokens: 500,
      fundingKind: 'card',
      model: 'sonnet',
      promptTokens: 1000,
    })
    // The published estimate cannot drift from the billed charge.
    expect(estimate.estimatedChargeUsd).toBeCloseTo(priced.chargeUsd, 6)
    expect(estimate.estimatedCredits).toBeCloseTo(priced.credits, 4)
    expect(estimate.model).toBe('sonnet')
    expect(estimate.isEstimate).toBe(true)
  })

  it('derives credits from charge at the base credit unit', () => {
    const estimate = estimateRequestCost({
      completionTokens: 1000,
      fundingKind: 'card',
      model: 'gpt-oss-120b',
      promptTokens: 4000,
    })
    expect(estimate.estimatedCredits).toBeCloseTo(
      estimate.estimatedChargeUsd / BASE_CREDIT_USD,
      3,
    )
  })

  it('charges the msat the metering hook would decrement (ceiling)', () => {
    const estimate = estimateRequestCost({
      completionTokens: 800,
      fundingKind: 'card',
      model: 'kimi-k2p6',
      promptTokens: 2000,
    })
    expect(estimate.estimatedChargeMsat).toBe(
      usdToMsatCeil(estimate.estimatedChargeUsd),
    )
    expect(Number.isInteger(estimate.estimatedChargeMsat)).toBe(true)
  })

  it('quotes the Bitcoin rail cheaper and surfaces the exact saving', () => {
    const common = {
      completionTokens: 1000,
      model: 'sonnet',
      promptTokens: 4000,
    } as const
    const card = estimateRequestCost({ ...common, fundingKind: 'card' })
    const bitcoin = estimateRequestCost({ ...common, fundingKind: 'bitcoin' })

    expect(card.fundingDiscountUsd).toBe(0)
    // Bitcoin charge is the card charge minus the funding discount.
    expect(bitcoin.estimatedChargeUsd).toBeLessThan(card.estimatedChargeUsd)
    expect(bitcoin.fundingDiscountUsd).toBeCloseTo(
      card.estimatedChargeUsd * BITCOIN_DISCOUNT,
      6,
    )
    expect(bitcoin.estimatedChargeUsd).toBeCloseTo(
      card.estimatedChargeUsd - bitcoin.fundingDiscountUsd,
      6,
    )
  })

  it('flags the free-tier-eligible Gemini lane while still pricing paid', () => {
    const estimate = estimateRequestCost({
      completionTokens: 1000,
      fundingKind: 'card',
      model: 'gemini-3.5-flash',
      promptTokens: 4000,
    })
    expect(estimate.freeTierEligible).toBe(true)
    // The estimate is the PAID price (for when the free pool is exhausted).
    expect(estimate.estimatedChargeUsd).toBeGreaterThan(0)
  })

  it('flags unknown models and prices them at the conservative fallback', () => {
    const estimate = estimateRequestCost({
      completionTokens: 100,
      fundingKind: 'card',
      model: 'no-such-model-xyz',
      promptTokens: 100,
    })
    expect(estimate.isUnknownModel).toBe(true)
    expect(estimate.freeTierEligible).toBe(false)
    expect(estimate.estimatedChargeUsd).toBeGreaterThan(0)
  })

  it('clamps negative / fractional / NaN token counts to non-negative ints', () => {
    const estimate = estimateRequestCost({
      completionTokens: Number.NaN,
      fundingKind: 'card',
      model: 'sonnet',
      promptTokens: -50,
    })
    expect(estimate.promptTokens).toBe(0)
    expect(estimate.completionTokens).toBe(0)
    expect(estimate.cachedPromptTokens).toBe(0)
    expect(estimate.estimatedChargeUsd).toBe(0)
    expect(estimate.estimatedCredits).toBe(0)
    expect(estimate.estimatedChargeMsat).toBe(0)
  })

  it('never counts more cached tokens than prompt tokens', () => {
    const estimate = estimateRequestCost({
      cachedPromptTokens: 5000,
      completionTokens: 0,
      fundingKind: 'card',
      model: 'gpt-oss-120b',
      promptTokens: 1000,
    })
    expect(estimate.cachedPromptTokens).toBe(1000)
  })

  it('cached input is cheaper than uncached for the same prompt size', () => {
    const uncached = estimateRequestCost({
      completionTokens: 0,
      fundingKind: 'card',
      model: 'gpt-oss-120b',
      promptTokens: 10_000,
    })
    const cached = estimateRequestCost({
      cachedPromptTokens: 10_000,
      completionTokens: 0,
      fundingKind: 'card',
      model: 'gpt-oss-120b',
      promptTokens: 10_000,
    })
    expect(cached.estimatedChargeUsd).toBeLessThan(uncached.estimatedChargeUsd)
  })

  it('applies the batch discount to the estimate', () => {
    const standard = estimateRequestCost({
      completionTokens: 1000,
      fundingKind: 'card',
      model: 'minimax',
      promptTokens: 4000,
    })
    const batch = estimateRequestCost({
      batch: true,
      completionTokens: 1000,
      fundingKind: 'card',
      model: 'minimax',
      promptTokens: 4000,
    })
    expect(batch.estimatedChargeUsd).toBeLessThan(standard.estimatedChargeUsd)
  })

  it('input-only estimate tracks the published per-Mtok sell price', () => {
    const estimate = estimateRequestCost({
      completionTokens: 0,
      fundingKind: 'card',
      model: 'sonnet',
      promptTokens: 1_000_000,
    })
    const sell = sellPricePerMtok('sonnet', 'input')!
    expect(estimate.estimatedChargeUsd).toBeCloseTo(sell, 6)
  })
})
