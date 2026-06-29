import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentCreditDebitRecord,
  BuyerPaymentReceiptRecord,
  BuyerPaymentRedemptionRecord,
  BuyerPaymentSpendLimitRecord,
} from './buyer-payment-ledger'
import type { OpenAgentsPaidEndpointProductRecord } from './paid-endpoint-product-catalog'
import {
  evaluateOpenAgentsBuyerPaymentEntitlementPolicy,
  openAgentsBuyerPaymentEntitlementPolicyFromProduct,
} from './buyer-payment-entitlement-policy'
import {
  OpenAgentsUnifiedPaymentDecisionProjection,
  OpenAgentsUnifiedPaymentDecisionUnsafe,
  evaluateOpenAgentsUnifiedPaymentDecision,
  openAgentsUnifiedPaymentDecisionHasPrivateMaterial,
} from './unified-payment-decision'

const now = '2026-06-07T13:00:00.000Z'

const externalAuthority = {
  authorizationRequired: false,
  authorizationSatisfied: false,
  confidentialDataRequired: false,
  confidentialDataSatisfied: false,
  moderationRequired: false,
  moderationSatisfied: false,
  ownerWriteRequired: false,
  ownerWriteSatisfied: false,
  payoutRequired: false,
  payoutSatisfied: false,
  siteDeployRequired: false,
  siteDeploySatisfied: false,
}

const paidProduct: OpenAgentsPaidEndpointProductRecord = {
  binding: {
    actionRef: 'action.agent_api.search',
    kind: 'agent_api_endpoint',
    method: 'POST',
    pathTemplate: '/api/agents/search',
    resourceRef: 'resource.agent_api.search',
  },
  displayName: 'Hosted search',
  entitlement: {
    durationSeconds: 86_400,
    kind: 'duration_quota',
    quotaUnits: 10,
    scopeRefs: ['entitlement.agent_api.search.day'],
  },
  internalEconomicsRefs: [],
  operatorNoteRefs: [],
  price: {
    amountMinorUnits: 500,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  productId: 'product.agent_api.search.day',
  projectionPolicy: 'agent_visible',
  providerBindingRefs: [],
  publicAgentDocRefs: ['docs.agent_api.search.payment'],
  publicSummaryRef: 'summary.agent_api.search.payment',
  spendCapHintRefs: ['spend_cap.agent_api.search.day'],
  status: 'active',
  surface: 'agent_api',
}

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: 'agent:buyer_123',
  amount: paidProduct.price,
  archivedAt: null,
  challengeRef: 'challenge.agent_api.search.day',
  createdAt: now,
  entitlementRef: 'entitlement.agent_api.search.day.1',
  id: 'buyer_payment_receipt_search_day',
  metadataRefs: ['metadata.receipt.redacted'],
  ownerUserId: 'user_owner_123',
  productId: paidProduct.productId,
  publicProjectionJson: '{}',
  receiptRef: 'receipt.agent_api.search.day.1',
  redactedPaymentRef: 'payment_ref.redacted.agent_api.search.day',
  status: 'issued',
  surface: paidProduct.surface,
}

const redemption: BuyerPaymentRedemptionRecord = {
  actorRef: 'agent:buyer_123',
  archivedAt: null,
  challengeRef: receipt.challengeRef,
  createdAt: now,
  entitlementRef: receipt.entitlementRef,
  id: 'buyer_payment_redemption_search_day',
  idempotencyKeyHash: 'hash.redemption.search_day',
  metadataRefs: ['metadata.redemption.redacted'],
  proofRef: 'proof.redacted.search_day',
  receiptRef: receipt.receiptRef,
  redemptionRef: 'redemption.agent_api.search.day.1',
  replayed: 0,
  status: 'redeemed',
}

const creditDebit: BuyerPaymentCreditDebitRecord = {
  actorRef: 'agent:buyer_123',
  amount: {
    amountMinorUnits: 500,
    asset: 'credits',
    denomination: 'credit',
  },
  archivedAt: null,
  billingLedgerEntryRef: 'billing_ledger.credit.search_day.1',
  createdAt: now,
  debitRef: 'credit_debit.agent_api.search.day.1',
  id: 'buyer_payment_credit_debit_search_day',
  idempotencyKeyHash: 'hash.credit_debit.search_day',
  metadataRefs: ['metadata.credit_debit.reserved'],
  ownerUserId: 'user_owner_123',
  productId: paidProduct.productId,
  publicProjectionJson: '{}',
  receiptRef: null,
  status: 'reserved',
}

const spendLimit: BuyerPaymentSpendLimitRecord = {
  actorRef: 'agent:buyer_123',
  amount: {
    amountMinorUnits: 1_000,
    asset: 'credits',
    denomination: 'credit',
  },
  archivedAt: null,
  createdAt: now,
  id: 'buyer_payment_spend_limit_search_day',
  metadataRefs: ['metadata.spend_limit.daily'],
  ownerUserId: 'user_owner_123',
  productId: paidProduct.productId,
  scopeRef: 'scope.agent_api.search',
  spendLimitRef: 'spend_limit.agent_api.search.day',
  status: 'active',
  updatedAt: now,
  windowRef: 'window.day.2026_06_07',
}

const entitlementDecision = evaluateOpenAgentsBuyerPaymentEntitlementPolicy({
  actorRef: 'agent:buyer_123',
  audience: 'agent',
  entitlement: null,
  externalAuthority,
  idempotencyKeyHash: 'hash.entitlement.search_day',
  nowIso: now,
  policy: openAgentsBuyerPaymentEntitlementPolicyFromProduct(paidProduct, {
    actorRef: 'agent:buyer_123',
    policyRef: 'policy.agent_api.search.day',
    shape: 'hybrid',
    siteRef: null,
  }),
  priorIdempotencyKeyHashes: [],
  product: paidProduct,
  receipt,
  redemption,
  requestedResourceRef: paidProduct.binding.resourceRef,
  requestedRouteRef: paidProduct.binding.pathTemplate,
  requestedScopeRefs: paidProduct.entitlement.scopeRefs,
  requestedSiteRef: null,
  usageCount: 0,
})

const baseInput = {
  actorRef: 'agent:buyer_123',
  audience: 'agent',
  creditState: {
    balanceMinorUnits: 0,
    creditDebit: null,
    creditLedgerRefs: ['billing_ledger.search_day.available'],
    currency: 'USD',
    requiredMinorUnits: 500,
    stripeTopUpRefs: ['stripe_topup.search_day.available'],
    stripeTopUpState: 'available',
  },
  entitlementDecision: null,
  freeBeta: {
    allowanceRef: null,
    available: false,
    remainingUses: null,
  },
  idempotencyKeyHash: 'hash.unified.search_day',
  l402Mdk: {
    entitlementDecision: null,
    providerState: 'available',
    receipt: null,
    redemption: null,
  },
  limitClass: 'economic_usage',
  nowIso: now,
  policyRefs: ['policy.unified_payment.search_day'],
  productRef: paidProduct.productId,
  publicSummaryRef: 'summary.unified_payment.search_day',
  requiredEndpointRefs: ['/api/agents/search'],
  requiredScopeRefs: paidProduct.entitlement.scopeRefs,
  sourceRefs: {
    creditLedgerRefs: ['billing_ledger.search_day.available'],
    entitlementRefs: [],
    l402RedemptionRef: null,
    mdkCheckoutReceiptRef: null,
    policyRefs: ['policy.unified_payment.search_day'],
    spendCapRefs: ['spend_cap.agent_api.search.day'],
    stripeTopUpRefs: ['stripe_topup.search_day.available'],
  },
  spendLimit,
  surface: 'agent_api',
} as const

describe('OpenAgents unified payment decision', () => {
  test('treats credit-paid and L402-paid access as equivalent entitlement decisions', () => {
    const creditPaid = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      creditState: {
        ...baseInput.creditState,
        balanceMinorUnits: 1_000,
        creditDebit,
        stripeTopUpState: 'not_configured',
      },
      entitlementDecision,
      l402Mdk: {
        ...baseInput.l402Mdk,
        providerState: 'missing_config',
      },
    })
    const l402Paid = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      creditState: {
        ...baseInput.creditState,
        balanceMinorUnits: 0,
        stripeTopUpRefs: [],
        stripeTopUpState: 'not_configured',
      },
      l402Mdk: {
        entitlementDecision,
        providerState: 'available',
        receipt,
        redemption,
      },
    })

    expect(S.decodeUnknownSync(
      OpenAgentsUnifiedPaymentDecisionProjection,
    )(creditPaid)).toEqual(creditPaid)
    expect(creditPaid.status).toBe('allow')
    expect(l402Paid.status).toBe('allow')
    expect(creditPaid.statusCode).toBe(l402Paid.statusCode)
    expect(creditPaid.entitlementDecision?.status).toBe('create_entitlement')
    expect(l402Paid.entitlementDecision?.status).toBe('create_entitlement')
    expect(creditPaid.sourceRefs.entitlementRefs).toEqual(
      l402Paid.sourceRefs.entitlementRefs,
    )
    expect(openAgentsUnifiedPaymentDecisionHasPrivateMaterial(creditPaid))
      .toBe(false)
    expect(openAgentsUnifiedPaymentDecisionHasPrivateMaterial(l402Paid))
      .toBe(false)
  })

  test('allows free-beta fallback when paid recovery is unavailable', () => {
    const projection = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      creditState: {
        ...baseInput.creditState,
        stripeTopUpRefs: [],
        stripeTopUpState: 'not_configured',
      },
      freeBeta: {
        allowanceRef: 'allowance.free_beta.search_day',
        available: true,
        remainingUses: 1,
      },
      l402Mdk: {
        ...baseInput.l402Mdk,
        providerState: 'missing_config',
      },
    })

    expect(projection.status).toBe('allow')
    expect(projection.paymentSource).toBe('free_beta')
    expect(projection.nextActions).toEqual(['use_free_beta'])
    expect(projection.freeBetaAllowanceRef).toBe('allowance.free_beta.search_day')
  })

  test('offers the correct recovery action when credits or L402 are missing', () => {
    const either = evaluateOpenAgentsUnifiedPaymentDecision(baseInput)
    const creditsOnly = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      l402Mdk: {
        ...baseInput.l402Mdk,
        providerState: 'missing_config',
      },
    })
    const l402Only = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      creditState: {
        ...baseInput.creditState,
        stripeTopUpRefs: [],
        stripeTopUpState: 'not_configured',
      },
    })

    expect(either.status).toBe('recoverable_by_either')
    expect(either.nextActions).toEqual(['add_credits', 'pay_l402_mdk'])
    expect(creditsOnly.status).toBe('recoverable_by_credits')
    expect(creditsOnly.nextActions).toEqual(['add_credits'])
    expect(l402Only.status).toBe('recoverable_by_l402_mdk')
    expect(l402Only.nextActions).toEqual(['pay_l402_mdk'])
  })

  test('blocks safety policy before considering credits or L402 evidence', () => {
    const projection = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      creditState: {
        ...baseInput.creditState,
        balanceMinorUnits: 1_000,
        creditDebit,
      },
      l402Mdk: {
        entitlementDecision,
        providerState: 'available',
        receipt,
        redemption,
      },
      limitClass: 'safety',
    })

    expect(projection.status).toBe('hard_blocked')
    expect(projection.statusCode).toBe(403)
    expect(projection.paymentSource).toBe('none')
    expect(projection.nextActions).toEqual(['stop'])
  })

  test('returns manual review and provider unavailable states', () => {
    const manual = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      limitClass: 'manual_review',
    })
    const unavailable = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      creditState: {
        ...baseInput.creditState,
        stripeTopUpRefs: [],
        stripeTopUpState: 'not_configured',
      },
      l402Mdk: {
        ...baseInput.l402Mdk,
        providerState: 'provider_unavailable',
      },
      limitClass: 'provider_capacity',
    })

    expect(manual.status).toBe('manual_review')
    expect(manual.nextActions).toEqual(['request_manual_review'])
    expect(unavailable.status).toBe('provider_unavailable')
    expect(unavailable.statusCode).toBe(503)
    expect(unavailable.nextActions).toEqual(['retry_later'])
  })

  test('returns exhausted when neither credits, free-beta, nor L402 are available', () => {
    const projection = evaluateOpenAgentsUnifiedPaymentDecision({
      ...baseInput,
      creditState: {
        ...baseInput.creditState,
        stripeTopUpRefs: [],
        stripeTopUpState: 'not_configured',
      },
      l402Mdk: {
        ...baseInput.l402Mdk,
        providerState: 'missing_config',
      },
    })

    expect(projection.status).toBe('exhausted')
    expect(projection.statusCode).toBe(402)
    expect(projection.nextActions).toEqual(['stop'])
  })

  test('rejects Stripe, webhook, invoice, and raw credit material in public decisions', () => {
    expect(() =>
      evaluateOpenAgentsUnifiedPaymentDecision({
        ...baseInput,
        sourceRefs: {
          ...baseInput.sourceRefs,
          stripeTopUpRefs: ['cus_123456789'],
        },
      })
    ).toThrow(OpenAgentsUnifiedPaymentDecisionUnsafe)

    expect(() =>
      evaluateOpenAgentsUnifiedPaymentDecision({
        ...baseInput,
        creditState: {
          ...baseInput.creditState,
          creditLedgerRefs: ['raw_credit_ledger:{\"balance\":100}'],
        },
      })
    ).toThrow(OpenAgentsUnifiedPaymentDecisionUnsafe)
  })
})
