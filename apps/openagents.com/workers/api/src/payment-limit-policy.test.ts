import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsPaymentLimitPolicyProjection,
  classifyOpenAgentsPaymentLimitPolicy,
  openAgentsPaymentLimitPolicyProjectionHasPrivateMaterial,
  projectOpenAgentsPaymentLimitPolicyDecision,
} from './payment-limit-policy'

describe('OpenAgents payment limit policy classifier', () => {
  test.each([
    ['safety', 'reason.payment_policy.safety_not_payable'],
    ['abuse', 'reason.payment_policy.abuse_not_payable'],
    ['private_authority', 'reason.payment_policy.private_authority_not_payable'],
  ] as const)('hard-blocks %s limits and offers no paid recovery', (
    limitClass,
    reasonRef,
  ) => {
    const decision = classifyOpenAgentsPaymentLimitPolicy({
      creditsAvailable: true,
      l402MdkAvailable: true,
      limitClass,
      requiredProductRefs: ['product.agent_api.extra_calls'],
      surface: 'agent_api',
    })

    expect(decision).toMatchObject({
      decisionStatus: 'blocked',
      limitClass,
      reasonRefs: [reasonRef],
      recoveryActions: [],
      statusRefs: ['status.payment_policy.not_payable'],
    })
  })

  test('allows free-beta and credit-balance limits when allowance is available', () => {
    expect(
      classifyOpenAgentsPaymentLimitPolicy({
        freeBetaAvailable: true,
        limitClass: 'free_beta_allowance',
        surface: 'site_checkout',
      }),
    ).toMatchObject({
      decisionStatus: 'allow',
      recoveryActions: ['free_beta'],
      statusRefs: ['status.payment_policy.allowed_free_beta'],
    })
    expect(
      classifyOpenAgentsPaymentLimitPolicy({
        creditsAvailable: true,
        limitClass: 'credits',
        surface: 'runner',
      }),
    ).toMatchObject({
      decisionStatus: 'allow',
      recoveryActions: ['credit_balance'],
      statusRefs: ['status.payment_policy.allowed_credit_balance'],
    })
  })

  test('marks economic usage recoverable through credits and L402/MDK when available', () => {
    const decision = classifyOpenAgentsPaymentLimitPolicy({
      creditsAvailable: true,
      entitlementScopeRefs: ['entitlement.agent_api.extra_calls.24h'],
      l402MdkAvailable: true,
      limitClass: 'economic_usage',
      operatorCostRefs: ['internal_cost.runner.container.preview'],
      privateAccountRefs: ['provider_account.codex.1'],
      publicSummaryRef: 'summary.payment_policy.agent_api.recoverable',
      requiredEndpointRefs: ['endpoint.agent_api.proposal_intake'],
      requiredProductRefs: ['product.agent_api.extra_calls'],
      spendCapCaveatRefs: ['spend_cap.bitcoin.max_small_test_amount'],
      surface: 'agent_api',
    })

    expect(decision).toMatchObject({
      decisionStatus: 'recoverable',
      entitlementScopeRefs: ['entitlement.agent_api.extra_calls.24h'],
      limitClass: 'economic_usage',
      recoveryActions: ['credit_balance', 'l402_mdk'],
      requiredEndpointRefs: ['endpoint.agent_api.proposal_intake'],
      requiredProductRefs: ['product.agent_api.extra_calls'],
      spendCapCaveatRefs: ['spend_cap.bitcoin.max_small_test_amount'],
      statusRefs: ['status.payment_policy.payment_recovery_available'],
    })
  })

  test('keeps provider capacity and no-recovery economic limits in manual review', () => {
    expect(
      classifyOpenAgentsPaymentLimitPolicy({
        limitClass: 'provider_capacity',
        surface: 'runner',
      }),
    ).toMatchObject({
      decisionStatus: 'manual_review',
      recoveryActions: ['manual_review'],
    })
    expect(
      classifyOpenAgentsPaymentLimitPolicy({
        limitClass: 'l402_mdk_recoverable',
        surface: 'forum_paid_action',
      }),
    ).toMatchObject({
      decisionStatus: 'manual_review',
      reasonRefs: ['reason.payment_policy.no_payment_recovery_available'],
      recoveryActions: ['manual_review'],
    })
  })

  test('redacts customer and agent projections while preserving safe operator cost refs', () => {
    const decision = classifyOpenAgentsPaymentLimitPolicy({
      creditsAvailable: true,
      l402MdkAvailable: true,
      limitClass: 'economic_usage',
      operatorCostRefs: [
        'internal_cost.runner.container.preview',
        'raw_invoice_should_be_filtered',
      ],
      privateAccountRefs: ['provider_account.codex.1'],
      reasonRefs: [
        'reason.payment_policy.safe',
        'ben@example.com',
      ],
      requiredProductRefs: [
        'product.site_checkout.preview',
        'wallet_secret',
      ],
      surface: 'site_checkout',
    })
    const customer = projectOpenAgentsPaymentLimitPolicyDecision(
      decision,
      'customer',
    )
    const agent = projectOpenAgentsPaymentLimitPolicyDecision(decision, 'agent')
    const operator = projectOpenAgentsPaymentLimitPolicyDecision(
      decision,
      'operator',
    )

    expect(S.decodeUnknownSync(OpenAgentsPaymentLimitPolicyProjection)(customer))
      .toEqual(customer)
    expect(customer.operatorCostRefs).toEqual([])
    expect(customer.privateAccountRefs).toEqual([])
    expect(customer.reasonRefs).toEqual([
      'reason.payment_policy.economic_limit_recoverable',
      'reason.payment_policy.safe',
    ])
    expect(customer.requiredProductRefs).toEqual([
      'product.site_checkout.preview',
    ])
    expect(agent).toMatchObject({
      audience: 'agent',
      operatorCostRefs: [],
      privateAccountRefs: [],
    })
    expect(operator.operatorCostRefs).toEqual([
      'internal_cost.runner.container.preview',
    ])
    expect(operator.privateAccountRefs).toEqual([])
    expect(openAgentsPaymentLimitPolicyProjectionHasPrivateMaterial(customer))
      .toBe(false)
    expect(openAgentsPaymentLimitPolicyProjectionHasPrivateMaterial(agent))
      .toBe(false)
    expect(openAgentsPaymentLimitPolicyProjectionHasPrivateMaterial(operator))
      .toBe(false)
  })
})
