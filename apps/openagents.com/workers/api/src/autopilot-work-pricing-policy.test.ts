import { describe, expect, test } from 'vitest'

import {
  autopilotWorkPricingPolicy,
  pricingLaneForRunnerKind,
  pricingReasonRefsForRunnerKind,
} from './autopilot-work-pricing-policy'

describe('autopilot work pricing policy — hosted Gemini gateway lane', () => {
  test('defines a buyer-funded, usd_credits-metered lane for hosted_gemini', () => {
    const lane = pricingLaneForRunnerKind('hosted_gemini')

    expect(lane).not.toBeNull()
    expect(lane).toMatchObject({
      buyerDebitRequired: true,
      laneRef: 'lane.autopilot_work.hosted_gemini_gateway',
      meterKind: 'usd_credits',
      runnerKind: 'hosted_gemini',
    })
    // A paid gateway lane MUST charge per metered unit.
    expect(lane?.unitAmountCents ?? 0).toBeGreaterThan(0)
  })

  test('surfaces hosted Gemini metering reason refs for placement decisions', () => {
    expect(pricingReasonRefsForRunnerKind('hosted_gemini')).toEqual([
      'pricing.autopilot_work.hosted_gemini_metered',
      'placement.reason.hosted_gemini_gateway_metered',
    ])
  })

  test('keeps every metered lane buyer-funded and free lanes unmetered', () => {
    for (const lane of autopilotWorkPricingPolicy.laneMeters) {
      if (lane.meterKind === 'usd_credits') {
        expect(lane.buyerDebitRequired).toBe(true)
        expect(lane.unitAmountCents).toBeGreaterThan(0)
      } else {
        expect(lane.buyerDebitRequired).toBe(false)
        expect(lane.unitAmountCents).toBe(0)
      }
    }
  })

  test('exposes exactly one lane per runner kind', () => {
    const laneRefs = autopilotWorkPricingPolicy.laneMeters.map(
      lane => lane.runnerKind,
    )

    expect(new Set(laneRefs).size).toBe(laneRefs.length)
  })
})
