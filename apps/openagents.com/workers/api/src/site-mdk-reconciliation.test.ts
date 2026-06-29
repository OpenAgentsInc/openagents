import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentReceiptRecord,
} from './buyer-payment-ledger'
import type { OpenAgentsHostedMdkCheckoutProjection } from './hosted-mdk-client'
import {
  OpenAgentsSiteMdkReconciliationProjection,
  OpenAgentsSiteMdkReconciliationUnsafe,
  openAgentsSiteMdkReconciliationHasPrivateMaterial,
  projectOpenAgentsSiteMdkReconciliation,
} from './site-mdk-reconciliation'
import { projectOpenAgentsSiteCheckoutReturn } from './site-checkout-return'

const price = {
  amountMinorUnits: 2500,
  asset: 'usd',
  denomination: 'usd_cent',
} as const

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'site.site_otec.checkout',
  archivedAt: null,
  challengeRef: 'challenge.site_checkout.site_otec.checkout_1',
  createdAt: '2026-06-06T12:05:00.000Z',
  expiresAt: '2026-06-06T13:10:00.000Z',
  id: 'buyer_payment_challenge_site_otec_checkout_1',
  idempotencyKeyHash: 'hash.site_checkout.site_otec.checkout_1',
  metadataRefs: ['metadata.site_checkout_intent.site_otec'],
  method: 'POST',
  ownerUserId: null,
  path: '/checkout/consultation-deposit',
  price,
  productId:
    'site_payment:site_otec:version_site_otec_v5:product:consultation_deposit',
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:site_checkout:site_otec:checkout_1',
  spendCap: price,
  status: 'issued',
  surface: 'site_checkout',
}

const hostedCheckout: OpenAgentsHostedMdkCheckoutProjection = {
  acceptedWorkSettlementAuthority: false,
  amount: price,
  audience: 'agent',
  challengeRef: challenge.challengeRef,
  checkoutRef: 'mdk_checkout.site_otec.checkout_1',
  checkoutUrlRef: 'mdk_checkout_url.site_otec.checkout_1',
  environment: 'sandbox',
  invoiceRef: null,
  paymentHashRef: null,
  productId: challenge.productId,
  provider: 'mdk_hosted',
  providerPayoutAuthority: false,
  providerRef: 'provider.openagents.hosted_mdk.fake',
  sandbox: true,
  settlementAuthority: 'buyer_payment_evidence_only',
  siteRef: 'site_otec',
  status: 'payment_received',
}

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: challenge.actorRef,
  amount: price,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: '2026-06-06T13:01:00.000Z',
  entitlementRef: 'entitlement.site_payment.site_otec.consultation_deposit',
  id: 'buyer_payment_receipt_site_otec_checkout_1',
  metadataRefs: ['metadata.receipt.site_otec.checkout_1'],
  ownerUserId: null,
  productId: challenge.productId,
  publicProjectionJson: '{}',
  receiptRef: 'receipt.site_payment.site_otec.checkout_1',
  redactedPaymentRef: 'redacted_payment.site_otec.checkout_1',
  status: 'issued',
  surface: 'site_checkout',
}

const entitlement: BuyerPaymentEntitlementRecord = {
  actorRef: challenge.actorRef,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  consumedAt: null,
  createdAt: '2026-06-06T13:01:10.000Z',
  entitlementRef: receipt.entitlementRef,
  expiresAt: '2026-06-07T13:01:10.000Z',
  id: 'buyer_payment_entitlement_site_otec_checkout_1',
  ownerUserId: null,
  productId: challenge.productId,
  receiptRef: receipt.receiptRef,
  scopeRefs: [
    'entitlement.site_payment.site_otec.version_site_otec_v5.product.consultation_deposit',
  ],
  status: 'active',
  surface: 'site_checkout',
}

const returnProjection = projectOpenAgentsSiteCheckoutReturn({
  audience: 'agent',
  buyerPaymentChallenge: challenge,
  entitlement,
  hostedCheckout,
  nowEpochMillis: Date.parse('2026-06-06T13:02:00.000Z'),
  observedReturnPath: '/checkout/thanks',
  receipt,
  returnAction: 'success',
  route: {
    cancelPath: '/pricing',
    checkoutIntentRef: 'site_checkout_intent.site_otec.checkout_1',
    checkoutRef: hostedCheckout.checkoutRef,
    siteId: 'site_otec',
    siteVersionId: 'version_site_otec_v5',
    successPath: '/checkout/thanks',
  },
  uiPrimitives: null,
})

const providerEvent = {
  challengeRef: challenge.challengeRef,
  checkoutRef: hostedCheckout.checkoutRef,
  checkoutStatus: hostedCheckout.status,
  environment: hostedCheckout.environment,
  eventBodyDigestRef: 'sha256:provider_event:site_otec:checkout_1',
  eventKind: 'payment_received',
  eventRef: 'event.site_mdk.site_otec.checkout_1.payment_received',
  fakeProvider: true,
  metadataRefs: ['metadata.site_mdk_reconciliation.checkout_1'],
  occurredAt: '2026-06-06T13:01:30.000Z',
  productId: challenge.productId,
  providerEventRef: 'provider_event.mdk.fake.checkout_1.payment_received',
  providerRef: hostedCheckout.providerRef,
  sandbox: hostedCheckout.sandbox,
  signatureBindingRef: null,
  signatureVerified: false,
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v5',
} as const

const baseInput = {
  audience: 'agent',
  entitlement,
  hostedCheckout,
  previousEventRef: null,
  providerEvent,
  receipt,
  returnProjection,
} as const

describe('OpenAgents Site MDK reconciliation', () => {
  test('projects fake-provider payment receipts into matched buyer reconciliation events', () => {
    const projection = projectOpenAgentsSiteMdkReconciliation(baseInput)

    expect(S.decodeUnknownSync(OpenAgentsSiteMdkReconciliationProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      acceptedWorkSettlementAuthority: false,
      fakeProviderOnly: true,
      implementationState: 'fake_provider_only',
      payoutAuthority: false,
    })
    expect(projection.buyerPaymentReconciliationEvent).toMatchObject({
      audience: 'agent',
      challengeRef: challenge.challengeRef,
      productId: challenge.productId,
      receiptRef: receipt.receiptRef,
      recordKind: 'reconciliation_event',
      status: 'matched',
    })
    expect(projection.receipt?.redactedPaymentRef).toBe(
      receipt.redactedPaymentRef,
    )
    expect(projection.entitlement?.entitlementRef).toBe(
      entitlement.entitlementRef,
    )
    expect(projection.returnProjection?.returnState).toBe('entitled')
    expect(openAgentsSiteMdkReconciliationHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps provider refs out of agent projections but exposes safe refs to operators', () => {
    const agentProjection = projectOpenAgentsSiteMdkReconciliation(baseInput)
    const operatorProjection = projectOpenAgentsSiteMdkReconciliation({
      ...baseInput,
      audience: 'operator',
    })

    expect(agentProjection.operatorRefs).toEqual([])
    expect(
      agentProjection.buyerPaymentReconciliationEvent.operatorRefs,
    ).toEqual([])
    expect(operatorProjection.operatorRefs).toEqual(
      expect.arrayContaining([
        providerEvent.providerRef,
        providerEvent.providerEventRef,
        providerEvent.eventBodyDigestRef,
      ]),
    )
    expect(
      operatorProjection.buyerPaymentReconciliationEvent.operatorRefs,
    ).toEqual(
      expect.arrayContaining([
        providerEvent.providerRef,
        providerEvent.providerEventRef,
      ]),
    )
  })

  test('marks duplicate provider events as replayed', () => {
    const projection = projectOpenAgentsSiteMdkReconciliation({
      ...baseInput,
      previousEventRef: 'event.site_mdk.previous',
    })

    expect(projection.buyerPaymentReconciliationEvent.status).toBe('replayed')
    expect(projection.buyerPaymentReconciliationEvent.publicProjectionJson)
      .toContain('provider_event.mdk.fake.checkout_1.payment_received')
  })

  test('rejects unverified non-fake provider events and mismatched checkout refs', () => {
    const unverified = projectOpenAgentsSiteMdkReconciliation({
      ...baseInput,
      providerEvent: {
        ...providerEvent,
        fakeProvider: false,
        providerRef: 'provider.openagents.hosted_mdk.production',
      },
    })
    const mismatched = projectOpenAgentsSiteMdkReconciliation({
      ...baseInput,
      providerEvent: {
        ...providerEvent,
        checkoutRef: 'mdk_checkout.site_otec.other',
      },
    })

    expect(unverified.buyerPaymentReconciliationEvent.status).toBe('rejected')
    expect(unverified.implementationState).toBe('verification_config_gated')
    expect(mismatched.buyerPaymentReconciliationEvent.status).toBe('rejected')
  })

  test('rejects raw provider payloads, invoices, and customer private data', () => {
    expect(() =>
      projectOpenAgentsSiteMdkReconciliation({
        ...baseInput,
        providerEvent: {
          ...providerEvent,
          eventBodyDigestRef: 'raw_webhook_payload.site_otec',
        },
      }),
    ).toThrow(OpenAgentsSiteMdkReconciliationUnsafe)
    expect(() =>
      projectOpenAgentsSiteMdkReconciliation({
        ...baseInput,
        hostedCheckout: {
          ...hostedCheckout,
          invoiceRef: 'lnbc2500n1rawinvoice',
        },
      }),
    ).toThrow(OpenAgentsSiteMdkReconciliationUnsafe)
    expect(() =>
      projectOpenAgentsSiteMdkReconciliation({
        ...baseInput,
        providerEvent: {
          ...providerEvent,
          metadataRefs: ['ben@example.com'],
        },
      }),
    ).toThrow(OpenAgentsSiteMdkReconciliationUnsafe)
  })
})
