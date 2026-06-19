import { describe, expect, test } from 'vitest'

import {
  DEFAULT_INFERENCE_SPLIT_WEIGHTS,
  computeInferenceSplit,
  usdToSatsFloor,
} from './inference-referral-split'
import { type PriceResult } from './pricing'

// A minimal priced result: $1.00 charge, $0.50 cost => $0.50 margin.
const priced = (overrides: Partial<PriceResult> = {}): PriceResult => ({
  chargeUsd: 1,
  costUsd: 0.5,
  credits: 100,
  discountUsd: 0,
  fundingKind: 'card',
  grossChargeUsd: 1,
  isUnknownModel: false,
  margin: 0.4,
  model: 'sonnet',
  ...overrides,
})

describe('usdToSatsFloor', () => {
  test('floors USD to integer sats at the reference rate', () => {
    // $1 at $100k/BTC = 1e-5 BTC = 1000 sats.
    expect(usdToSatsFloor(1, 100_000)).toBe(1000)
    // A sub-sat amount floors to 0 (never rounds up past what was earned).
    expect(usdToSatsFloor(0.00005, 100_000)).toBe(0)
    expect(usdToSatsFloor(0)).toBe(0)
    expect(usdToSatsFloor(-1)).toBe(0)
    expect(usdToSatsFloor(Number.NaN)).toBe(0)
  })
})

describe('computeInferenceSplit', () => {
  test('splits the MARGIN three ways; cost is never revshare', () => {
    const split = computeInferenceSplit(
      { priced: priced(), servedByContributor: true },
      100_000,
    )

    expect(split.chargeUsd).toBe(1)
    expect(split.costUsd).toBe(0.5)
    expect(split.marginUsd).toBeCloseTo(0.5, 10)

    // Default weights: referrer 5% of margin (aligned to the RL-1 ledger 500
    // bps policy), serving node 30% of margin.
    // margin 0.5 => referrer 0.025, serving 0.15, openagents 0.325.
    expect(split.referrer.usd).toBeCloseTo(0.025, 10)
    expect(split.servingNode.usd).toBeCloseTo(0.15, 10)
    expect(split.openagents.usd).toBeCloseTo(0.325, 10)

    // Sats are floored. $0.025 @ $100k/BTC = 25 sats; $0.15 => 150; the
    // openagents remainder ($0.325 with binary-float dust) floors to 324.
    expect(split.referrer.sats).toBe(25)
    expect(split.servingNode.sats).toBe(150)
    expect(split.openagents.sats).toBe(324)

    // The whole margin in sats is exposed for the ledger feed: $0.50 => 500 sats.
    expect(split.marginSats).toBe(500)

    // The three shares of margin reconcile (within rounding).
    expect(
      split.referrer.usd + split.servingNode.usd + split.openagents.usd,
    ).toBeCloseTo(split.marginUsd, 10)
  })

  test('zeroes the serving-node share when no contributor served it', () => {
    const split = computeInferenceSplit(
      { priced: priced(), servedByContributor: false },
      100_000,
    )

    // Referrer still earns (referral is on ALL inference); serving-node margin
    // stays with OpenAgents.
    expect(split.referrer.usd).toBeCloseTo(0.025, 10)
    expect(split.servingNode.usd).toBe(0)
    expect(split.servingNode.sats).toBe(0)
    expect(split.openagents.usd).toBeCloseTo(0.475, 10)
  })

  test('zero/negative margin yields all-zero shares (no revshare on a loss)', () => {
    const split = computeInferenceSplit(
      { priced: priced({ chargeUsd: 0.5, costUsd: 0.5 }), servedByContributor: true },
      100_000,
    )
    expect(split.marginUsd).toBe(0)
    expect(split.referrer.sats).toBe(0)
    expect(split.servingNode.sats).toBe(0)
    expect(split.openagents.sats).toBe(0)
  })

  test('clamps so OpenAgents never goes negative when weights exceed 100%', () => {
    const split = computeInferenceSplit(
      {
        priced: priced(),
        servedByContributor: true,
        weights: { referrerMarginBps: 8000, servingNodeMarginBps: 8000 },
      },
      100_000,
    )
    // Combined 160% of margin is scaled to 100%; OpenAgents keeps >= 0.
    expect(split.openagents.usd).toBeGreaterThanOrEqual(0)
    expect(
      split.referrer.usd + split.servingNode.usd,
    ).toBeCloseTo(split.marginUsd, 8)
  })

  test('default weights are 5% referrer (RL-1 policy) / 30% serving node of margin', () => {
    expect(DEFAULT_INFERENCE_SPLIT_WEIGHTS.referrerMarginBps).toBe(500)
    expect(DEFAULT_INFERENCE_SPLIT_WEIGHTS.servingNodeMarginBps).toBe(3000)
  })
})
