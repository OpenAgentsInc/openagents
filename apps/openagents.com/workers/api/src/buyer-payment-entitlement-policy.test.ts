import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentEntitlementRecord,
  BuyerPaymentReceiptRecord,
  BuyerPaymentRedemptionRecord,
} from './buyer-payment-ledger'
import type { OpenAgentsPaidEndpointProductRecord } from './paid-endpoint-product-catalog'
import {
  OpenAgentsBuyerPaymentEntitlementPolicyProjection,
  OpenAgentsBuyerPaymentEntitlementPolicyUnsafe,
  evaluateOpenAgentsBuyerPaymentEntitlementPolicy,
  openAgentsBuyerPaymentEntitlementPolicyFromProduct,
  openAgentsBuyerPaymentEntitlementPolicyHasPrivateMaterial,
} from './buyer-payment-entitlement-policy'

const now = '2026-06-07T12:00:00.000Z'

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

const product = (
  input: Readonly<{
    durationSeconds: number | null
    kind: OpenAgentsPaidEndpointProductRecord['entitlement']['kind']
    pathTemplate?: string | null
    productId: string
    quotaUnits: number | null
    resourceRef?: string
    scopeRefs: ReadonlyArray<string>
    status?: OpenAgentsPaidEndpointProductRecord['status']
  }>,
): OpenAgentsPaidEndpointProductRecord => ({
  binding: {
    actionRef: 'action.otec.report',
    kind: 'site_paid_action',
    method: 'GET',
    pathTemplate: input.pathTemplate ?? '/api/sites/otec/report',
    resourceRef: input.resourceRef ?? 'resource.site_otec.report',
  },
  displayName: 'OTEC report',
  entitlement: {
    durationSeconds: input.durationSeconds,
    kind: input.kind,
    quotaUnits: input.quotaUnits,
    scopeRefs: [...input.scopeRefs],
  },
  internalEconomicsRefs: [],
  operatorNoteRefs: [],
  price: {
    amountMinorUnits: 25_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  productId: input.productId,
  projectionPolicy: 'customer_visible',
  providerBindingRefs: [],
  publicAgentDocRefs: ['docs.sites.payment_policy'],
  publicSummaryRef: 'summary.site_otec.report',
  spendCapHintRefs: ['spend_cap.site_otec.report'],
  status: input.status ?? 'active',
  surface: 'site_checkout',
})

const receiptFor = (
  paidProduct: OpenAgentsPaidEndpointProductRecord,
): BuyerPaymentReceiptRecord => ({
  actorRef: 'agent:buyer_123',
  amount: paidProduct.price,
  archivedAt: null,
  challengeRef: `challenge.${paidProduct.productId}`,
  createdAt: now,
  entitlementRef: `entitlement.${paidProduct.productId}`,
  id: `receipt_row_${paidProduct.productId.replaceAll('.', '_')}`,
  metadataRefs: ['metadata.receipt.redacted'],
  ownerUserId: 'user_owner_123',
  productId: paidProduct.productId,
  publicProjectionJson: '{}',
  receiptRef: `receipt.${paidProduct.productId}`,
  redactedPaymentRef: `payment_ref.redacted.${paidProduct.productId}`,
  status: 'issued',
  surface: paidProduct.surface,
})

const redemptionFor = (
  paidProduct: OpenAgentsPaidEndpointProductRecord,
  input: Partial<BuyerPaymentRedemptionRecord> = {},
): BuyerPaymentRedemptionRecord => ({
  actorRef: 'agent:buyer_123',
  archivedAt: null,
  challengeRef: `challenge.${paidProduct.productId}`,
  createdAt: now,
  entitlementRef: `entitlement.${paidProduct.productId}`,
  id: `redemption_row_${paidProduct.productId.replaceAll('.', '_')}`,
  idempotencyKeyHash: 'hash.redemption.otec_report',
  metadataRefs: ['metadata.redemption.redacted'],
  proofRef: `proof.redacted.${paidProduct.productId}`,
  receiptRef: `receipt.${paidProduct.productId}`,
  redemptionRef: `redemption.${paidProduct.productId}`,
  replayed: 0,
  status: 'redeemed',
  ...input,
})

const entitlementFor = (
  paidProduct: OpenAgentsPaidEndpointProductRecord,
  input: Partial<BuyerPaymentEntitlementRecord> = {},
): BuyerPaymentEntitlementRecord => ({
  actorRef: 'agent:buyer_123',
  archivedAt: null,
  challengeRef: `challenge.${paidProduct.productId}`,
  consumedAt: null,
  createdAt: now,
  entitlementRef: `entitlement.${paidProduct.productId}`,
  expiresAt: null,
  id: `entitlement_row_${paidProduct.productId.replaceAll('.', '_')}`,
  ownerUserId: 'user_owner_123',
  productId: paidProduct.productId,
  receiptRef: `receipt.${paidProduct.productId}`,
  scopeRefs: [...paidProduct.entitlement.scopeRefs],
  status: 'active',
  surface: paidProduct.surface,
  ...input,
})

const baseInputFor = (
  paidProduct: OpenAgentsPaidEndpointProductRecord,
  input: Partial<Parameters<
    typeof evaluateOpenAgentsBuyerPaymentEntitlementPolicy
  >[0]> = {},
) => {
  const shapeOverride =
    paidProduct.entitlement.kind === 'quota' &&
      paidProduct.entitlement.quotaUnits === 1
      ? { shape: 'one_shot' as const }
      : {}
  const policy = openAgentsBuyerPaymentEntitlementPolicyFromProduct(
    paidProduct,
    {
      actorRef: 'agent:buyer_123',
      policyRef: `policy.${paidProduct.productId}`,
      siteRef: 'site.otec',
      ...shapeOverride,
    },
  )

  return {
    actorRef: 'agent:buyer_123',
    audience: 'agent',
    entitlement: null,
    externalAuthority,
    idempotencyKeyHash: 'hash.policy.otec_report',
    nowIso: now,
    policy,
    priorIdempotencyKeyHashes: [],
    product: paidProduct,
    receipt: null,
    redemption: null,
    requestedResourceRef: paidProduct.binding.resourceRef,
    requestedRouteRef: paidProduct.binding.pathTemplate,
    requestedScopeRefs: paidProduct.entitlement.scopeRefs,
    requestedSiteRef: 'site.otec',
    usageCount: 0,
    ...input,
  } as Parameters<typeof evaluateOpenAgentsBuyerPaymentEntitlementPolicy>[0]
}

describe('OpenAgents buyer payment entitlement policy', () => {
  test('consumes one-shot credentials once and rejects a consumed entitlement', () => {
    const paidProduct = product({
      durationSeconds: null,
      kind: 'quota',
      productId: 'product.site_otec.one_shot_report',
      quotaUnits: 1,
      scopeRefs: ['entitlement.site_otec.report.once'],
    })
    const projection = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        receipt: receiptFor(paidProduct),
        redemption: redemptionFor(paidProduct),
      }),
    )
    const exhausted = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        entitlement: entitlementFor(paidProduct, {
          consumedAt: '2026-06-07T12:01:00.000Z',
          status: 'consumed',
        }),
        receipt: receiptFor(paidProduct),
        usageCount: 1,
      }),
    )

    expect(S.decodeUnknownSync(
      OpenAgentsBuyerPaymentEntitlementPolicyProjection,
    )(projection)).toEqual(projection)
    expect(projection.status).toBe('consume_one_shot')
    expect(projection.createEntitlement).toBe(true)
    expect(projection.useEntitlement).toBe(true)
    expect(projection.remainingQuotaUnits).toBe(0)
    expect(openAgentsBuyerPaymentEntitlementPolicyHasPrivateMaterial(projection))
      .toBe(false)
    expect(exhausted.status).toBe('exhausted')
    expect(exhausted.useEntitlement).toBe(false)
  })

  test('preserves idempotency for duplicate redemption attempts', () => {
    const paidProduct = product({
      durationSeconds: null,
      kind: 'quota',
      productId: 'product.site_otec.duplicate_report',
      quotaUnits: 1,
      scopeRefs: ['entitlement.site_otec.report.once'],
    })
    const projection = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        priorIdempotencyKeyHashes: ['hash.policy.otec_report'],
        redemption: redemptionFor(paidProduct, {
          replayed: 1,
          status: 'replayed',
        }),
      }),
    )

    expect(projection.status).toBe('duplicate_replay')
    expect(projection.createEntitlement).toBe(false)
    expect(projection.useEntitlement).toBe(false)
    expect(projection.nextAction).toBe('stop')
  })

  test('decrements quota and rejects exhausted quota', () => {
    const paidProduct = product({
      durationSeconds: null,
      kind: 'quota',
      productId: 'product.site_otec.quota_report',
      quotaUnits: 3,
      scopeRefs: ['entitlement.site_otec.report.quota'],
    })
    const usable = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        entitlement: entitlementFor(paidProduct),
        receipt: receiptFor(paidProduct),
        usageCount: 1,
      }),
    )
    const exhausted = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        entitlement: entitlementFor(paidProduct),
        receipt: receiptFor(paidProduct),
        usageCount: 3,
      }),
    )

    expect(usable.status).toBe('decrement_quota')
    expect(usable.decrementQuota).toBe(true)
    expect(usable.remainingQuotaUnits).toBe(1)
    expect(exhausted.status).toBe('exhausted')
    expect(exhausted.remainingQuotaUnits).toBe(0)
  })

  test('expires and renews time-window entitlements', () => {
    const paidProduct = product({
      durationSeconds: 86_400,
      kind: 'duration',
      productId: 'product.site_otec.day_report',
      quotaUnits: null,
      scopeRefs: ['entitlement.site_otec.report.day'],
    })
    const expiredEntitlement = entitlementFor(paidProduct, {
      expiresAt: '2026-06-07T11:59:00.000Z',
      status: 'active',
    })
    const expired = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        entitlement: expiredEntitlement,
        receipt: receiptFor(paidProduct),
      }),
    )
    const renewed = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        entitlement: expiredEntitlement,
        receipt: receiptFor(paidProduct),
        redemption: redemptionFor(paidProduct),
      }),
    )

    expect(expired.status).toBe('expired')
    expect(expired.statusCode).toBe(402)
    expect(expired.expiryLabelRef).toBe(
      'expiry.buyer_payment_entitlement.duration_seconds.86400',
    )
    expect(renewed.status).toBe('renew_entitlement')
    expect(renewed.renewEntitlement).toBe(true)
  })

  test.each([
    ['mismatched_resource', { requestedResourceRef: 'resource.site_otec.other' }],
    ['mismatched_route', { requestedRouteRef: '/api/sites/otec/other' }],
    ['mismatched_site', { requestedSiteRef: 'site.other' }],
    ['mismatched_actor', { actorRef: 'agent:other' }],
  ] as const)('rejects %s on hybrid scoped products', (status, patch) => {
    const paidProduct = product({
      durationSeconds: null,
      kind: 'resource',
      productId: `product.site_otec.${status}`,
      quotaUnits: null,
      scopeRefs: [`entitlement.site_otec.${status}`],
    })
    const policy = openAgentsBuyerPaymentEntitlementPolicyFromProduct(
      paidProduct,
      {
        actorRef: 'agent:buyer_123',
        shape: 'hybrid',
        siteRef: 'site.otec',
      },
    )
    const projection = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        policy,
        redemption: redemptionFor(paidProduct),
        ...patch,
      }),
    )

    expect(projection.status).toBe(status)
    expect(projection.statusCode).toBe(403)
    expect(projection.useEntitlement).toBe(false)
  })

  test('creates scoped product entitlements from redeemed payments', () => {
    const paidProduct = product({
      durationSeconds: null,
      kind: 'resource',
      productId: 'product.site_otec.resource_report',
      quotaUnits: null,
      scopeRefs: ['entitlement.site_otec.report.resource'],
    })
    const projection = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        redemption: redemptionFor(paidProduct),
      }),
    )

    expect(projection.status).toBe('create_entitlement')
    expect(projection.createEntitlement).toBe(true)
    expect(projection.entitlementRef).toBe(`entitlement.${paidProduct.productId}`)
    expect(projection.scopeRefs).toEqual(paidProduct.entitlement.scopeRefs)
  })

  test('rejects retired product policy and unsafe payment material', () => {
    const retiredProduct = product({
      durationSeconds: null,
      kind: 'resource',
      productId: 'product.site_otec.retired_report',
      quotaUnits: null,
      scopeRefs: ['entitlement.site_otec.report.retired'],
      status: 'retired',
    })
    const retired = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(retiredProduct, {
        redemption: redemptionFor(retiredProduct),
      }),
    )

    expect(retired.status).toBe('blocked')
    expect(() =>
      evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
        baseInputFor(retiredProduct, {
          policy: {
            ...openAgentsBuyerPaymentEntitlementPolicyFromProduct(
              retiredProduct,
            ),
            metadataRefs: ['lnbc10n1unsafeinvoice'],
          },
        }),
      )
    ).toThrow(OpenAgentsBuyerPaymentEntitlementPolicyUnsafe)
  })

  test('payment proof cannot satisfy authorization, moderation, owner, deploy, data, or payout policy', () => {
    const paidProduct = product({
      durationSeconds: null,
      kind: 'resource',
      productId: 'product.site_otec.authority_report',
      quotaUnits: null,
      scopeRefs: ['entitlement.site_otec.report.authority'],
    })
    const projection = evaluateOpenAgentsBuyerPaymentEntitlementPolicy(
      baseInputFor(paidProduct, {
        externalAuthority: {
          ...externalAuthority,
          ownerWriteRequired: true,
          siteDeployRequired: true,
        },
        redemption: redemptionFor(paidProduct),
      }),
    )

    expect(projection.status).toBe('blocked')
    expect(projection.reasonRefs).toContain(
      'reason.buyer_payment_entitlement.owner_write_required',
    )
    expect(projection.authorityEffects).toEqual({
      authorizesConfidentialData: false,
      authorizesModerationBypass: false,
      authorizesOwnerWrite: false,
      authorizesPayout: false,
      authorizesSiteDeploy: false,
      authorizesUserAccess: false,
    })
    expect(projection.safeBody).toMatchObject({
      action: 'buyer_payment_entitlement_policy',
      status: 'blocked',
    })
  })
})
