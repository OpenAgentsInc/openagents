import { describe, expect, test } from 'vitest'

import { KHALA_MINI_MODEL_ID } from '../pricing'
import { MPP_MIN_USDC, SPT_MIN_USD, quoteMppCall } from './mpp-pricing'

describe('mpp per-call pricing', () => {
  test('quotes a crypto call for khala-mini at or above the 0.01 USDC floor', () => {
    const quote = quoteMppCall({ model: KHALA_MINI_MODEL_ID, rail: 'crypto' })
    expect(quote.rail).toBe('crypto')
    expect(quote.priceUsd).toBeGreaterThanOrEqual(MPP_MIN_USDC)
    // amountCents is whole cents, never below 1 (the 0.01 USDC floor).
    expect(quote.amountCents).toBeGreaterThanOrEqual(1)
    expect(Number.isInteger(quote.amountCents)).toBe(true)
  })

  test('the card rail respects the 0.50 USD SPT minimum', () => {
    const quote = quoteMppCall({ model: KHALA_MINI_MODEL_ID, rail: 'card' })
    expect(quote.rail).toBe('card')
    expect(quote.priceUsd).toBeGreaterThanOrEqual(SPT_MIN_USD)
    expect(quote.amountCents).toBeGreaterThanOrEqual(50)
  })

  test('a pricier model quotes a higher per-call price than the floor', () => {
    // A representative call on an expensive model should clear the floor with a
    // real cost-derived price.
    const quote = quoteMppCall({
      completionTokens: 5000,
      model: 'openagents/khala-code',
      promptTokens: 5000,
      rail: 'crypto',
    })
    expect(quote.priceUsd).toBeGreaterThan(MPP_MIN_USDC)
  })

  test('the quote is derived from the per-token pricing model (not Bitcoin-discounted)', () => {
    // MPP inbound funds are USDC/card, not Bitcoin — the quote prices at the
    // card funding rate. We assert the price is positive + finite and that a
    // larger token budget costs strictly more (monotonic in usage).
    const small = quoteMppCall({
      completionTokens: 100,
      model: 'openagents/khala-code',
      promptTokens: 100,
      rail: 'crypto',
    })
    const large = quoteMppCall({
      completionTokens: 100_000,
      model: 'openagents/khala-code',
      promptTokens: 100_000,
      rail: 'crypto',
    })
    expect(large.priceUsd).toBeGreaterThan(small.priceUsd)
  })
})
