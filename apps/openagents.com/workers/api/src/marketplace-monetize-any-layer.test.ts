import { describe, expect, test } from 'vitest'

import { INFERENCE_RESALE_SUBSCRIPTION_FORBIDDEN_REF } from './inference-resale-authorization'
import {
  MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE,
  MONETIZE_LAYER_SELF_REFERRAL_REF,
  type LayerMonetizationDefinition,
  buildLayerMonetizationDefinition,
  planLayerMonetizationAccrual,
} from './marketplace-monetize-any-layer'

const okDefinition = (
  overrides: Partial<Parameters<typeof buildLayerMonetizationDefinition>[0]> = {},
): LayerMonetizationDefinition => {
  const result = buildLayerMonetizationDefinition({
    offerId: 'offer_inference_resale',
    sellerRef: 'agent:seller',
    layer: 'inference',
    monetizationKind: 'agentic_work',
    unitPriceMsat: 1000,
    priceAsset: 'bitcoin',
    referralBps: 500,
    referrerRef: 'agent:referrer',
    createdAt: '2026-06-19T00:00:00.000Z',
    ...overrides,
  })
  if (!result.ok) {
    throw new Error(`expected ok definition: ${result.error.reason}`)
  }
  return result.definition
}

describe('monetize-any-layer per-layer offer model (#5518)', () => {
  test('builds a typed offer pinned to the planned promise', () => {
    const definition = okDefinition()
    expect(definition.promiseId).toBe(MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE)
    expect(definition.layer).toBe('inference')
    expect(definition.referralBps).toBe(500)
  })

  test('rejects invalid price and out-of-range referral bps', () => {
    for (const overrides of [
      { unitPriceMsat: -1 },
      { unitPriceMsat: 1.5 },
      { referralBps: -1 },
      { referralBps: 10001 },
      { sellerRef: '' },
      { referrerRef: '  ' },
    ] as const) {
      const result = buildLayerMonetizationDefinition({
        offerId: 'o',
        sellerRef: 'agent:seller',
        layer: 'data',
        monetizationKind: 'agentic_work',
        unitPriceMsat: 100,
        priceAsset: 'credit',
        referralBps: 100,
        referrerRef: 'agent:referrer',
        ...overrides,
      })
      expect(result.ok).toBe(false)
    }
  })
})

describe('monetize-any-layer referral accrual seam (#5518)', () => {
  test('computes the referral cut in basis points off metered spend (inert/planned)', () => {
    const plan = planLayerMonetizationAccrual({
      definition: okDefinition({ referralBps: 500 }),
      meteredSpendMsat: 1_000_000,
    })
    expect(plan.inert).toBe(true)
    expect(plan.promiseState).toBe('planned')
    expect(plan.authorized).toBe(true)
    // 5% of 1_000_000 msat == 50_000 msat
    expect(plan.referralAccrualMsat).toBe(50_000)
    expect(plan.referrerRef).toBe('agent:referrer')
  })

  test('subscription-account resale is non-waivably blocked, accrual is zero', () => {
    const plan = planLayerMonetizationAccrual({
      definition: okDefinition({
        monetizationKind: 'subscription_capacity_resale',
      }),
      meteredSpendMsat: 1_000_000,
      accountAuthMode: 'api_key',
    })
    expect(plan.authorized).toBe(false)
    expect(plan.blockerRefs).toContain(INFERENCE_RESALE_SUBSCRIPTION_FORBIDDEN_REF)
    expect(plan.referralAccrualMsat).toBe(0)
  })

  test('api-inference resale authorizes only with the full ref chain + api_key', () => {
    const blocked = planLayerMonetizationAccrual({
      definition: okDefinition({
        monetizationKind: 'api_inference_gateway_resale',
      }),
      meteredSpendMsat: 1_000_000,
      accountAuthMode: 'subscription',
    })
    expect(blocked.authorized).toBe(false)
    expect(blocked.referralAccrualMsat).toBe(0)

    const authorized = planLayerMonetizationAccrual({
      definition: okDefinition({
        monetizationKind: 'api_inference_gateway_resale',
        referralBps: 1000,
      }),
      meteredSpendMsat: 1_000_000,
      accountAuthMode: 'api_key',
      resaleRefs: {
        providerGrantRef: 'provider.grant.abc',
        routePolicyRef: 'route.policy.abc',
        meteringReceiptRef: 'metering.receipt.abc',
        pricingPolicyRef: 'pricing.policy.abc',
        tosBoundaryRef: 'tos.boundary.abc',
        dispatchRef: 'dispatch.abc',
        assignmentReceiptRef: 'assignment.receipt.abc',
        settlementReceiptRef: 'settlement.receipt.abc',
      },
    })
    expect(authorized.authorized).toBe(true)
    // 10% of 1_000_000 msat == 100_000 msat
    expect(authorized.referralAccrualMsat).toBe(100_000)
  })

  test('self-referral is blocked and accrues nothing', () => {
    const plan = planLayerMonetizationAccrual({
      definition: okDefinition({
        sellerRef: 'agent:same',
        referrerRef: 'agent:same',
      }),
      meteredSpendMsat: 1_000_000,
    })
    expect(plan.authorized).toBe(false)
    expect(plan.blockerRefs).toContain(MONETIZE_LAYER_SELF_REFERRAL_REF)
    expect(plan.referralAccrualMsat).toBe(0)
  })

  test('non-positive metered spend yields zero accrual even when authorized', () => {
    const plan = planLayerMonetizationAccrual({
      definition: okDefinition(),
      meteredSpendMsat: 0,
    })
    expect(plan.authorized).toBe(true)
    expect(plan.referralAccrualMsat).toBe(0)
  })
})
