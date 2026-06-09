import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { OpenAgentsPaidEndpointProductRecord } from './paid-endpoint-product-catalog'
import type { OpenAgentsUnifiedPaymentDecisionProjection } from './unified-payment-decision'
import type { OpenAgentsSpendCapPreviewInput } from './agent-spend-cap-preview'
import {
  OpenAgentsSpendCapPreviewProjection,
  OpenAgentsSpendCapPreviewUnsafe,
  openAgentsSpendCapPreviewHasPrivateMaterial,
  previewOpenAgentsSpendCap,
} from './agent-spend-cap-preview'

const bitcoinPrice = (amountMinorUnits: number) => ({
  amountMinorUnits,
  asset: 'bitcoin' as const,
  denomination: 'bitcoin_millisatoshi' as const,
})

const creditPrice = (amountMinorUnits: number) => ({
  amountMinorUnits,
  asset: 'credits' as const,
  denomination: 'credit' as const,
})

const paidProduct: OpenAgentsPaidEndpointProductRecord = {
  binding: {
    actionRef: 'action.site_otec.agent_summary',
    kind: 'site_paid_action',
    method: 'POST',
    pathTemplate: '/api/sites/otec/actions/summary',
    resourceRef: 'resource.site_otec.summary',
  },
  displayName: 'OTEC summary action',
  entitlement: {
    durationSeconds: 86_400,
    kind: 'duration_quota',
    quotaUnits: 5,
    scopeRefs: ['entitlement.site_otec.summary.day'],
  },
  internalEconomicsRefs: [],
  operatorNoteRefs: [],
  price: bitcoinPrice(1_000),
  productId: 'product.site_otec.summary.day',
  projectionPolicy: 'agent_visible',
  providerBindingRefs: [],
  publicAgentDocRefs: ['docs.site_otec.summary.payment'],
  publicSummaryRef: 'summary.site_otec.summary.payment',
  spendCapHintRefs: ['spend_cap.site_otec.summary.day'],
  status: 'active',
  surface: 'site_checkout',
}

const paymentDecision: OpenAgentsUnifiedPaymentDecisionProjection = {
  actorRef: 'agent:buyer_123',
  audience: 'agent',
  creditDebit: null,
  decisionRef: 'decision.unified_payment.site_otec.summary',
  entitlementDecision: null,
  freeBetaAllowanceRef: null,
  l402MdkReceipt: null,
  l402MdkRedemption: null,
  nextActions: ['pay_l402_mdk'],
  paymentSource: 'none',
  policyDecision: {
    audience: 'agent',
    decisionStatus: 'recoverable',
    entitlementScopeRefs: paidProduct.entitlement.scopeRefs,
    limitClass: 'economic_usage',
    operatorCostRefs: [],
    privateAccountRefs: [],
    publicSummaryRef: 'summary.unified_payment.site_otec.summary',
    reasonRefs: ['reason.payment_policy.economic_limit_recoverable'],
    recoveryActions: ['l402_mdk'],
    requiredEndpointRefs: ['/api/sites/otec/actions/summary'],
    requiredProductRefs: [paidProduct.productId],
    spendCapCaveatRefs: ['spend_cap.site_otec.summary.day'],
    statusRefs: ['status.payment_policy.payment_recovery_available'],
    surface: 'site_checkout',
  },
  productRef: paidProduct.productId,
  reasonRefs: ['reason.unified_payment.recoverable_by_l402_mdk'],
  safeBody: {
    action: 'unified_payment_decision',
    status: 'recoverable_by_l402_mdk',
  },
  sourceRefs: {
    creditLedgerRefs: [],
    entitlementRefs: [],
    l402RedemptionRef: null,
    mdkCheckoutReceiptRef: null,
    policyRefs: ['policy.unified_payment.site_otec.summary'],
    spendCapRefs: ['spend_cap.site_otec.summary.day'],
    stripeTopUpRefs: [],
  },
  spendLimit: null,
  status: 'recoverable_by_l402_mdk',
  statusCode: 402,
  surface: 'site_checkout',
}

const baseInput = (
  overrides: Partial<OpenAgentsSpendCapPreviewInput> = {},
): OpenAgentsSpendCapPreviewInput => ({
  actionRef: 'action.site_otec.agent_summary',
  actorRef: 'agent:buyer_123',
  agentAuthenticated: true,
  audience: 'agent',
  availableCreditAllowanceMinorUnits: 0,
  freeAllowanceUses: null,
  idempotencyKeyHintRef: 'idempotency.site_otec.summary.preview',
  idempotencyKeyRequired: true,
  l402MdkRecoveryAvailable: true,
  maxPerCall: bitcoinPrice(2_000),
  maxPerWindow: bitcoinPrice(5_000),
  nowIso: '2026-06-07T14:00:00.000Z',
  paymentDecision,
  price: bitcoinPrice(1_000),
  product: paidProduct,
  requestedRail: 'bitcoin_l402_mdk',
  retryBehaviorRefs: ['retry.spend_cap_preview.reuse_failed_credential'],
  route: {
    method: 'POST',
    ownerGrantOnly: false,
    path: '/api/sites/otec/actions/summary',
    privateRoute: false,
    routeRef: 'route.site_otec.summary',
  },
  settlementMode: 'deferred_until_success',
  supportedRails: ['bitcoin_l402_mdk', 'credits'],
  surface: 'site_checkout',
  windowSpent: bitcoinPrice(1_000),
  ...overrides,
})

describe('OpenAgents agent spend-cap preview', () => {
  test('returns an under-cap dry run with idempotency and side-effect guidance', () => {
    const projection = previewOpenAgentsSpendCap(baseInput())

    expect(S.decodeUnknownSync(OpenAgentsSpendCapPreviewProjection)(projection))
      .toEqual(projection)
    expect(projection.status).toBe('under_cap')
    expect(projection.statusCode).toBe(200)
    expect(projection.dryRun).toBe(true)
    expect(projection.nextActions).toEqual(['pay_l402_mdk'])
    expect(projection.idempotencyGuidanceRefs).toEqual([
      'idempotency.spend_cap_preview.required',
      'idempotency.site_otec.summary.preview',
    ])
    expect(projection.sideEffectSummary).toEqual({
      callsMdk: false,
      createsEntitlement: false,
      createsPaymentArtifact: false,
      debitsCredits: false,
      mutatesPayout: false,
      redeemsCredentials: false,
    })
    expect(openAgentsSpendCapPreviewHasPrivateMaterial(projection)).toBe(false)
  })

  test('classifies exact cap and over cap without spending', () => {
    const exact = previewOpenAgentsSpendCap(baseInput({
      maxPerCall: bitcoinPrice(1_000),
      maxPerWindow: bitcoinPrice(5_000),
    }))
    const over = previewOpenAgentsSpendCap(baseInput({
      maxPerCall: bitcoinPrice(999),
    }))

    expect(exact.status).toBe('exact_cap')
    expect(exact.nextActions).toEqual(['pay_l402_mdk'])
    expect(over.status).toBe('over_cap')
    expect(over.statusCode).toBe(402)
    expect(over.nextActions).toEqual(['lower_spend_or_raise_cap'])
    expect(over.sideEffectSummary.callsMdk).toBe(false)
  })

  test('rejects unsupported rail, missing catalog, unauthenticated agent, and owner-grant-only route', () => {
    const unsupported = previewOpenAgentsSpendCap(baseInput({
      requestedRail: 'free_beta',
      supportedRails: ['bitcoin_l402_mdk'],
    }))
    const missing = previewOpenAgentsSpendCap(baseInput({
      actionRef: null,
      product: null,
    }))
    const unauthenticated = previewOpenAgentsSpendCap(baseInput({
      agentAuthenticated: false,
    }))
    const ownerOnly = previewOpenAgentsSpendCap(baseInput({
      route: {
        ...baseInput().route,
        ownerGrantOnly: true,
      },
    }))

    expect(unsupported.status).toBe('unsupported_rail')
    expect(missing.status).toBe('catalog_missing')
    expect(unauthenticated.status).toBe('unauthenticated_agent')
    expect(unauthenticated.nextActions).toEqual(['provide_agent_token'])
    expect(ownerOnly.status).toBe('owner_grant_required')
    expect(ownerOnly.nextActions).toEqual(['ask_owner_for_grant'])
  })

  test('rejects malformed amount, wrong currency, private route, and stale catalog entry', () => {
    const malformed = previewOpenAgentsSpendCap(baseInput({
      price: bitcoinPrice(0),
    }))
    const wrongCurrency = previewOpenAgentsSpendCap(baseInput({
      maxPerCall: creditPrice(2_000),
    }))
    const privateRoute = previewOpenAgentsSpendCap(baseInput({
      route: {
        ...baseInput().route,
        privateRoute: true,
      },
    }))
    const stale = previewOpenAgentsSpendCap(baseInput({
      product: {
        ...paidProduct,
        status: 'paused',
      },
    }))

    expect(malformed.status).toBe('malformed_amount')
    expect(wrongCurrency.status).toBe('wrong_currency')
    expect(privateRoute.status).toBe('private_route')
    expect(stale.status).toBe('stale_catalog_entry')
  })

  test('projects free, credit, and entitlement next actions from the unified decision', () => {
    const free = previewOpenAgentsSpendCap(baseInput({
      paymentDecision: {
        ...paymentDecision,
        nextActions: ['use_free_beta'],
        paymentSource: 'free_beta',
        status: 'allow',
        statusCode: 200,
      },
      requestedRail: 'free_beta',
      supportedRails: ['free_beta'],
    }))
    const credits = previewOpenAgentsSpendCap(baseInput({
      maxPerCall: creditPrice(2_000),
      maxPerWindow: creditPrice(5_000),
      paymentDecision: {
        ...paymentDecision,
        nextActions: ['spend_internal_credits'],
        paymentSource: 'credit_balance',
        status: 'allow',
        statusCode: 200,
      },
      price: creditPrice(1_000),
      requestedRail: 'credits',
      supportedRails: ['credits'],
      windowSpent: creditPrice(0),
    }))
    const entitlement = previewOpenAgentsSpendCap(baseInput({
      paymentDecision: {
        ...paymentDecision,
        nextActions: ['use_entitlement'],
        paymentSource: 'product_entitlement',
        status: 'allow',
        statusCode: 200,
      },
      requestedRail: 'existing_entitlement',
      supportedRails: ['existing_entitlement'],
    }))

    expect(free.nextActions).toEqual(['use_free_beta'])
    expect(credits.nextActions).toEqual(['spend_internal_credits'])
    expect(entitlement.nextActions).toEqual(['use_entitlement'])
  })

  test('rejects raw payment material before projection', () => {
    expect(() =>
      previewOpenAgentsSpendCap(baseInput({
        actorRef: 'agent:buyer@example.com',
      }))
    ).toThrow(OpenAgentsSpendCapPreviewUnsafe)
  })
})
