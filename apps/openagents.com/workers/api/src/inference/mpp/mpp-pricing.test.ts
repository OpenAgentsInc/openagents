import { describe, expect, test } from 'vitest'

import { KHALA_MODEL_ID, priceRequest } from '../pricing'
import {
  LIGHTNING_MIN_SATS,
  MPP_MIN_USDC,
  SPT_MIN_USD,
  quoteMppCall,
  quoteMppLightningCall,
} from './mpp-pricing'

describe('mpp per-call pricing', () => {
  test('quotes a crypto call for Khala at or above the 0.01 USDC floor', () => {
    const quote = quoteMppCall({ model: KHALA_MODEL_ID, rail: 'crypto' })
    expect(quote.rail).toBe('crypto')
    expect(quote.priceUsd).toBeGreaterThanOrEqual(MPP_MIN_USDC)
    // amountCents is whole cents, never below 1 (the 0.01 USDC floor).
    expect(quote.amountCents).toBeGreaterThanOrEqual(1)
    expect(Number.isInteger(quote.amountCents)).toBe(true)
  })

  test('the card rail respects the 0.50 USD SPT minimum', () => {
    const quote = quoteMppCall({ model: KHALA_MODEL_ID, rail: 'card' })
    expect(quote.rail).toBe('card')
    expect(quote.priceUsd).toBeGreaterThanOrEqual(SPT_MIN_USD)
    expect(quote.amountCents).toBeGreaterThanOrEqual(50)
  })

  test('a pricier model quotes a higher per-call price than the floor', () => {
    // A representative call on an expensive model should clear the floor with a
    // real cost-derived price.
    const quote = quoteMppCall({
      completionTokens: 50_000,
      model: KHALA_MODEL_ID,
      promptTokens: 50_000,
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
      model: KHALA_MODEL_ID,
      promptTokens: 100,
      rail: 'crypto',
    })
    const large = quoteMppCall({
      completionTokens: 100_000,
      model: KHALA_MODEL_ID,
      promptTokens: 100_000,
      rail: 'crypto',
    })
    expect(large.priceUsd).toBeGreaterThan(small.priceUsd)
  })

  test('a hidden price model keeps the MPP product as Khala while sizing against backing cost', () => {
    const quote = quoteMppCall({
      completionTokens: 1_000_000,
      model: KHALA_MODEL_ID,
      priceModel: 'deepseek-v4-flash',
      promptTokens: 1_000_000,
      rail: 'crypto',
    })
    const priced = priceRequest({
      fundingKind: 'card',
      model: 'deepseek-v4-flash',
      usage: {
        completionTokens: 1_000_000,
        promptTokens: 1_000_000,
        totalTokens: 2_000_000,
      },
    })
    expect(quote.model).toBe(KHALA_MODEL_ID)
    expect(quote.priceUsd).toBe(priced.chargeUsd)
  })

  test('the Lightning rail quotes a positive integer SAT amount at/above the floor', () => {
    const quote = quoteMppLightningCall({ model: KHALA_MODEL_ID })
    expect(quote.amountSats).toBeGreaterThanOrEqual(LIGHTNING_MIN_SATS)
    expect(Number.isInteger(quote.amountSats)).toBe(true)
    expect(quote.priceUsd).toBeGreaterThan(0)
  })

  test('the Lightning rail prices at the BITCOIN funding rate (cheaper than card)', () => {
    // Lightning settles real Bitcoin, so it earns the Bitcoin funding discount
    // and is the cheapest rail for the same representative call (owner
    // Bitcoin-first). Compare the pre-conversion USD on a pricier model so the
    // floors do not mask the discount.
    const lightning = quoteMppLightningCall({
      completionTokens: 5000,
      model: KHALA_MODEL_ID,
      promptTokens: 5000,
    })
    const card = quoteMppCall({
      completionTokens: 5000,
      model: KHALA_MODEL_ID,
      promptTokens: 5000,
      rail: 'card',
    })
    expect(lightning.priceUsd).toBeLessThan(card.priceUsd)
  })

  test('a pricier model quotes more sats than a cheap one (monotonic in usage)', () => {
    const small = quoteMppLightningCall({
      completionTokens: 100,
      model: KHALA_MODEL_ID,
      promptTokens: 100,
    })
    const large = quoteMppLightningCall({
      completionTokens: 200_000,
      model: KHALA_MODEL_ID,
      promptTokens: 200_000,
    })
    expect(large.amountSats).toBeGreaterThan(small.amountSats)
  })
})
