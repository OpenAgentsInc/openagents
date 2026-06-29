import { describe, expect, test } from 'vitest'

import {
  BASE_CREDIT_USD,
  BATCH_DISCOUNT,
  BITCOIN_DISCOUNT,
  CACHED_INPUT_FRACTION,
  DEFAULT_MARGIN,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  MODEL_PRICING_TABLE,
  UNKNOWN_MODEL_COST,
  VERTEX_COST_IS_LIST_TODO,
  blendedCostPerMtok,
  costProportionalMultiplier,
  lookupModel,
  normalizePricingModelId,
  priceRequest,
  sellPricePerMtok,
} from './pricing'
import { type InferenceUsage } from './provider-adapter'

// Tolerant float compare for USD/credit math.
const closeTo = (actual: number, expected: number, digits = 9): void =>
  expect(actual).toBeCloseTo(expected, digits)

const usage = (
  promptTokens: number,
  completionTokens: number,
  cachedPromptTokens?: number,
): InferenceUsage => ({
  promptTokens,
  completionTokens,
  totalTokens: promptTokens + completionTokens,
  ...(cachedPromptTokens === undefined ? {} : { cachedPromptTokens }),
})

describe('config constants', () => {
  test('base credit, margin, discounts match the pricing doc', () => {
    expect(BASE_CREDIT_USD).toBe(0.01)
    expect(DEFAULT_MARGIN).toBe(0.4)
    expect(BITCOIN_DISCOUNT).toBe(0.05)
    expect(CACHED_INPUT_FRACTION).toBe(0.5)
    expect(BATCH_DISCOUNT).toBe(0.5)
    // The Vertex Claude cost is a published-list placeholder pending billing.
    expect(VERTEX_COST_IS_LIST_TODO).toBe(true)
  })
})

describe('multiplier table', () => {
  test('Sonnet is the 1.0× baseline; Opus ≈ 5×, Haiku ≈ 0.33× (cost-proportional)', () => {
    const opus = lookupModel('opus')!
    const sonnet = lookupModel('sonnet')!
    const haiku = lookupModel('haiku')!
    expect(sonnet.multiplier).toBe(1)
    // Opus blended 27.0 / Sonnet blended 5.4 = 5.0×.
    expect(opus.multiplier).toBe(5)
    // Haiku blended 1.8 / 5.4 = 0.333 -> rounded 0.33×.
    expect(haiku.multiplier).toBe(0.33)
    // Cost-proportional ordering: Opus > Sonnet > Haiku.
    expect(opus.multiplier).toBeGreaterThan(sonnet.multiplier)
    expect(sonnet.multiplier).toBeGreaterThan(haiku.multiplier)
  })

  test('blended cost uses the 4:1 coding mix from the doc', () => {
    // Sonnet: (4*3 + 1*15)/5 = 5.4
    closeTo(blendedCostPerMtok(lookupModel('sonnet')!.cost), 5.4)
    // Opus: (4*15 + 1*75)/5 = 27.0
    closeTo(blendedCostPerMtok(lookupModel('opus')!.cost), 27.0)
    // Haiku: (4*1 + 1*5)/5 = 1.8
    closeTo(blendedCostPerMtok(lookupModel('haiku')!.cost), 1.8)
  })

  test('costProportionalMultiplier is exact (unrounded)', () => {
    closeTo(costProportionalMultiplier(lookupModel('opus')!.cost), 5)
    closeTo(costProportionalMultiplier(lookupModel('haiku')!.cost), 1.8 / 5.4)
  })

  test('Claude rows are list placeholders; Fireworks rows are real', () => {
    expect(lookupModel('opus')!.costIsListPlaceholder).toBe(true)
    expect(lookupModel('sonnet')!.costIsListPlaceholder).toBe(true)
    expect(lookupModel('haiku')!.costIsListPlaceholder).toBe(true)
    expect(lookupModel('gpt-oss-120b')!.costIsListPlaceholder).toBe(false)
    expect(lookupModel('glm-5p2')!.costIsListPlaceholder).toBe(false)
  })

  test('Fireworks real cost numbers are encoded verbatim from the provider doc', () => {
    const gptoss = lookupModel('gpt-oss-120b')!.cost
    expect(gptoss.inputUsdPerMtok).toBe(0.15)
    expect(gptoss.outputUsdPerMtok).toBe(0.6)
    expect(gptoss.cachedInputUsdPerMtok).toBe(0.015)

    const minimax = lookupModel('minimax')!.cost
    expect(minimax.inputUsdPerMtok).toBe(0.3)
    expect(minimax.outputUsdPerMtok).toBe(1.2)

    const dsPro = lookupModel('deepseek-v4-pro')!.cost
    expect(dsPro.inputUsdPerMtok).toBe(1.74)
    expect(dsPro.outputUsdPerMtok).toBe(3.48)
    expect(dsPro.cachedInputUsdPerMtok).toBe(0.145)

    const glm = lookupModel('glm-5p2')!.cost
    expect(glm.inputUsdPerMtok).toBe(1.4)
    expect(glm.outputUsdPerMtok).toBe(4.4)
  })

  test('Hydralisk publishes raw OpenAI GPT-OSS model ids as owned serving lanes', () => {
    expect(lookupModel(HYDRALISK_GPT_OSS_20B_MODEL_ID)).toMatchObject({
      lane: 'hydralisk',
      model: HYDRALISK_GPT_OSS_20B_MODEL_ID,
    })
    expect(lookupModel(HYDRALISK_GPT_OSS_120B_MODEL_ID)).toMatchObject({
      lane: 'hydralisk',
      model: HYDRALISK_GPT_OSS_120B_MODEL_ID,
    })
  })

  test('lookup is case-insensitive and trims; every table entry resolves', () => {
    expect(lookupModel('OPUS')!.model).toBe('opus')
    expect(lookupModel('  Sonnet  ')!.model).toBe('sonnet')
    for (const e of MODEL_PRICING_TABLE) {
      expect(lookupModel(e.model)).toBe(e)
    }
  })

  test('Fireworks provider-native receipt ids normalize to canonical pricing rows', () => {
    expect(
      normalizePricingModelId('accounts/fireworks/models/deepseek-v4-flash'),
    ).toBe('deepseek-v4-flash')
    expect(lookupModel('fireworks/deepseek-v4-flash')!.model).toBe(
      'deepseek-v4-flash',
    )
    const providerNative = priceRequest({
      fundingKind: 'card',
      model: 'accounts/fireworks/models/deepseek-v4-flash',
      usage: usage(1_000_000, 1_000_000),
    })
    const canonical = priceRequest({
      fundingKind: 'card',
      model: 'deepseek-v4-flash',
      usage: usage(1_000_000, 1_000_000),
    })
    expect(providerNative).toEqual(canonical)
  })
})

describe('per-model charge math', () => {
  test('Sonnet worked example from the doc (200k in + 50k out @40%)', () => {
    const result = priceRequest({
      model: 'sonnet',
      usage: usage(200_000, 50_000),
      fundingKind: 'card',
    })
    // Cost (list): 0.2*3 + 0.05*15 = 1.35
    closeTo(result.costUsd, 1.35)
    // Sell @40%: 1.35 * 1.4 = 1.89
    closeTo(result.grossChargeUsd, 1.89)
    closeTo(result.chargeUsd, 1.89)
    // 1.89 / 0.01 = 189 credits
    closeTo(result.credits, 189)
    expect(result.isUnknownModel).toBe(false)
    expect(result.discountUsd).toBe(0)
  })

  test('Opus worked example from the doc (200k in + 50k out @40%)', () => {
    const result = priceRequest({
      model: 'opus',
      usage: usage(200_000, 50_000),
      fundingKind: 'card',
    })
    // Cost: 0.2*15 + 0.05*75 = 3 + 3.75 = 6.75
    closeTo(result.costUsd, 6.75)
    // Sell @40% ≈ 9.45
    closeTo(result.grossChargeUsd, 9.45)
    closeTo(result.credits, 945)
  })

  test('gpt-oss-120b charge uses real Fireworks cost', () => {
    const result = priceRequest({
      model: 'gpt-oss-120b',
      usage: usage(1_000_000, 1_000_000),
      fundingKind: 'card',
    })
    // Cost: 1*0.15 + 1*0.60 = 0.75
    closeTo(result.costUsd, 0.75)
    // Sell @40%: 0.75 * 1.4 = 1.05
    closeTo(result.grossChargeUsd, 1.05)
  })

  test('charge is monotonic in token count', () => {
    const small = priceRequest({
      model: 'haiku',
      usage: usage(1_000, 1_000),
      fundingKind: 'card',
    })
    const big = priceRequest({
      model: 'haiku',
      usage: usage(10_000, 10_000),
      fundingKind: 'card',
    })
    expect(big.chargeUsd).toBeGreaterThan(small.chargeUsd)
  })

  test('sell rate always clears cost + margin (margin invariant)', () => {
    for (const e of MODEL_PRICING_TABLE) {
      const r = priceRequest({
        model: e.model,
        usage: usage(123_456, 65_432),
        fundingKind: 'card',
      })
      // gross charge = cost * (1 + margin) exactly (card, no batch).
      closeTo(r.grossChargeUsd, r.costUsd * (1 + DEFAULT_MARGIN))
      expect(r.grossChargeUsd).toBeGreaterThan(r.costUsd)
    }
  })
})

describe('cached-input discount', () => {
  test('cached prompt tokens billed at provider cached rate (Fireworks)', () => {
    // gpt-oss-120b: input 0.15, cached 0.015, output 0.60.
    // 1M prompt of which 1M cached, 0 output.
    const allCached = priceRequest({
      model: 'gpt-oss-120b',
      usage: usage(1_000_000, 0, 1_000_000),
      fundingKind: 'card',
    })
    closeTo(allCached.costUsd, 0.015)

    const noneCached = priceRequest({
      model: 'gpt-oss-120b',
      usage: usage(1_000_000, 0, 0),
      fundingKind: 'card',
    })
    closeTo(noneCached.costUsd, 0.15)
    expect(allCached.costUsd).toBeLessThan(noneCached.costUsd)
  })

  test('cached fraction fallback when provider has no distinct cached rate (Claude)', () => {
    // Sonnet input 3.0 -> cached fallback = 3.0 * 0.5 = 1.5 per Mtok.
    const allCached = priceRequest({
      model: 'sonnet',
      usage: usage(1_000_000, 0, 1_000_000),
      fundingKind: 'card',
    })
    closeTo(allCached.costUsd, 1.5)
  })

  test('cached tokens are clamped to promptTokens', () => {
    // cachedPromptTokens > promptTokens must not over-discount or go negative.
    const r = priceRequest({
      model: 'sonnet',
      usage: usage(1_000, 0, 5_000),
      fundingKind: 'card',
    })
    // All 1000 prompt tokens treated as cached (3.0 * 0.5 = 1.5 /Mtok).
    closeTo(r.costUsd, (1_000 * 1.5) / 1_000_000)
    expect(r.costUsd).toBeGreaterThan(0)
  })
})

describe('batch discount', () => {
  test('batch halves both cost and charge', () => {
    const standard = priceRequest({
      model: 'glm-5p2',
      usage: usage(500_000, 200_000),
      fundingKind: 'card',
    })
    const batch = priceRequest({
      model: 'glm-5p2',
      usage: usage(500_000, 200_000),
      fundingKind: 'card',
      batch: true,
    })
    closeTo(batch.costUsd, standard.costUsd * BATCH_DISCOUNT)
    closeTo(batch.grossChargeUsd, standard.grossChargeUsd * BATCH_DISCOUNT)
  })
})

describe('bitcoin funding discount', () => {
  test('bitcoin applies ~5% off the gross charge; card does not', () => {
    const card = priceRequest({
      model: 'sonnet',
      usage: usage(200_000, 50_000),
      fundingKind: 'card',
    })
    const btc = priceRequest({
      model: 'sonnet',
      usage: usage(200_000, 50_000),
      fundingKind: 'bitcoin',
    })
    // Same gross; BTC charge is 95% of it.
    closeTo(btc.grossChargeUsd, card.grossChargeUsd)
    closeTo(btc.chargeUsd, card.grossChargeUsd * (1 - BITCOIN_DISCOUNT))
    closeTo(btc.discountUsd, card.grossChargeUsd * BITCOIN_DISCOUNT)
    // Doc worked example: card $1.89 -> bitcoin ≈ $1.7955 (~$1.80).
    closeTo(btc.chargeUsd, 1.7955)
  })

  test('bitcoin discount does not touch our cost basis (margin preserved)', () => {
    const card = priceRequest({
      model: 'opus',
      usage: usage(100_000, 100_000),
      fundingKind: 'card',
    })
    const btc = priceRequest({
      model: 'opus',
      usage: usage(100_000, 100_000),
      fundingKind: 'bitcoin',
    })
    closeTo(btc.costUsd, card.costUsd)
  })
})

describe('unknown + edge cases', () => {
  test('unknown model falls back to UNKNOWN_MODEL_COST and flags isUnknownModel', () => {
    const r = priceRequest({
      model: 'some-model-we-do-not-carry',
      usage: usage(1_000_000, 1_000_000),
      fundingKind: 'card',
    })
    expect(r.isUnknownModel).toBe(true)
    expect(r.model).toBe('some-model-we-do-not-carry')
    // Cost: 1*1.0 + 1*4.0 = 5.0
    closeTo(
      r.costUsd,
      UNKNOWN_MODEL_COST.inputUsdPerMtok + UNKNOWN_MODEL_COST.outputUsdPerMtok,
    )
  })

  test('zero usage => zero charge, not unknown error', () => {
    const r = priceRequest({
      model: 'sonnet',
      usage: usage(0, 0),
      fundingKind: 'card',
    })
    expect(r.costUsd).toBe(0)
    expect(r.grossChargeUsd).toBe(0)
    expect(r.chargeUsd).toBe(0)
    expect(r.credits).toBe(0)
    expect(r.isUnknownModel).toBe(false)
  })

  test('negative / NaN token counts clamp to zero (never negative charge)', () => {
    const r = priceRequest({
      model: 'sonnet',
      usage: {
        promptTokens: -100,
        completionTokens: Number.NaN,
        totalTokens: 0,
      },
      fundingKind: 'bitcoin',
    })
    expect(r.chargeUsd).toBe(0)
    expect(r.credits).toBe(0)
  })

  test('margin override changes the sell rate', () => {
    const at30 = priceRequest({
      model: 'sonnet',
      usage: usage(1_000_000, 0),
      fundingKind: 'card',
      margin: 0.3,
    })
    // Sonnet input cost 3.0 -> sell @30% = 3.9 /Mtok.
    closeTo(at30.grossChargeUsd, 3.9)
    expect(at30.margin).toBe(0.3)
  })
})

describe('sellPricePerMtok helper', () => {
  test('returns cost × (1 + margin) per dimension; undefined for unknown', () => {
    // Sonnet output: 15.0 * 1.4 = 21.0 (doc §3).
    closeTo(sellPricePerMtok('sonnet', 'output')!, 21.0)
    // Sonnet input: 3.0 * 1.4 = 4.2.
    closeTo(sellPricePerMtok('sonnet', 'input')!, 4.2)
    // Haiku input: 1.0 * 1.4 = 1.4.
    closeTo(sellPricePerMtok('haiku', 'input')!, 1.4)
    expect(sellPricePerMtok('nope', 'input')).toBeUndefined()
  })
})
