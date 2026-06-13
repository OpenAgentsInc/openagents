import { describe, expect, test } from 'vitest'

import {
  INFERENCE_RESALE_REQUIRED_REFS,
  INFERENCE_RESALE_REQUIRES_API_KEY_REF,
  INFERENCE_RESALE_SUBSCRIPTION_FORBIDDEN_REF,
  authorizeInferenceMonetization,
  type InferenceResaleRefs,
} from './inference-resale-authorization'

const fullRefs: InferenceResaleRefs = {
  providerGrantRef: 'provider.grant.codex.abc',
  routePolicyRef: 'route.policy.abc',
  meteringReceiptRef: 'metering.receipt.abc',
  pricingPolicyRef: 'pricing.policy.cost_plus_10',
  tosBoundaryRef: 'tos.boundary.abc',
  dispatchRef: 'dispatch.abc',
  assignmentReceiptRef: 'assignment.receipt.abc',
  settlementReceiptRef: 'settlement.receipt.abc',
}

describe('inference resale authorization', () => {
  test('subscription capacity resale is blocked unconditionally, even with a full ref chain', () => {
    const decision = authorizeInferenceMonetization({
      kind: 'subscription_capacity_resale',
      accountAuthMode: 'api_key',
      refs: fullRefs,
    })
    expect(decision.authorized).toBe(false)
    expect(decision.blockerRefs).toContain(INFERENCE_RESALE_SUBSCRIPTION_FORBIDDEN_REF)
  })

  test('agentic work / accepted outcomes stay authorized', () => {
    const decision = authorizeInferenceMonetization({ kind: 'agentic_work' })
    expect(decision.authorized).toBe(true)
    expect(decision.blockerRefs).toEqual([])
  })

  test('API-inference gateway resale on an API-key account with the full ref chain is authorized', () => {
    const decision = authorizeInferenceMonetization({
      kind: 'api_inference_gateway_resale',
      accountAuthMode: 'api_key',
      refs: fullRefs,
    })
    expect(decision.authorized).toBe(true)
    expect(decision.blockerRefs).toEqual([])
    expect(decision.schema).toBe('openagents.inference_resale_authorization.v1')
  })

  test('a missing ref blocks API-inference gateway resale with that typed blocker', () => {
    const { meteringReceiptRef: _omit, ...rest } = fullRefs
    const decision = authorizeInferenceMonetization({
      kind: 'api_inference_gateway_resale',
      accountAuthMode: 'api_key',
      refs: { ...rest, meteringReceiptRef: null },
    })
    expect(decision.authorized).toBe(false)
    expect(decision.blockerRefs).toContain('blocker.inference_resale.missing.metering_receipt')
  })

  test('API-inference gateway resale on a subscription account is blocked even with all refs', () => {
    const decision = authorizeInferenceMonetization({
      kind: 'api_inference_gateway_resale',
      accountAuthMode: 'subscription',
      refs: fullRefs,
    })
    expect(decision.authorized).toBe(false)
    expect(decision.blockerRefs).toContain(INFERENCE_RESALE_REQUIRES_API_KEY_REF)
  })

  test('every required ref has a distinct missing-blocker and an empty ref set lists them all', () => {
    const decision = authorizeInferenceMonetization({
      kind: 'api_inference_gateway_resale',
      accountAuthMode: 'api_key',
      refs: {},
    })
    expect(decision.authorized).toBe(false)
    for (const required of INFERENCE_RESALE_REQUIRED_REFS) {
      expect(decision.blockerRefs).toContain(required.missingRef)
    }
    // The decision is refs-only — no raw key/secret/payload material in it.
    const serialized = JSON.stringify(decision)
    expect(serialized).not.toMatch(/sk-/)
    expect(serialized).not.toMatch(/bearer/i)
  })
})
