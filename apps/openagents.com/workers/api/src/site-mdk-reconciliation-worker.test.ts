import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
} from './buyer-payment-ledger'
import type { OpenAgentsSiteMdkCheckoutIntentRecord } from './site-mdk-checkout-intents'
import type { OpenAgentsSiteMdkProviderEvent } from './site-mdk-reconciliation'
import type { OpenAgentsSiteMdkReconciliationWorkerInput } from './site-mdk-reconciliation-worker'
import {
  OpenAgentsSiteMdkReconciliationWorkerProjection,
  OpenAgentsSiteMdkReconciliationWorkerUnsafe,
  openAgentsSiteMdkReconciliationWorkerHasPrivateMaterial,
  planOpenAgentsSiteMdkReconciliationWorker,
} from './site-mdk-reconciliation-worker'

const amount = {
  amountMinorUnits: 2500,
  asset: 'usd',
  denomination: 'usd_cent',
} as const

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'site.site_otec.checkout',
  archivedAt: null,
  challengeRef: 'challenge.site_checkout.site_otec.checkout_1',
  createdAt: '2026-06-07T11:00:00.000Z',
  expiresAt: '2026-06-07T13:00:00.000Z',
  id: 'buyer_payment_challenge_site_otec_checkout_1',
  idempotencyKeyHash: 'hash.site_checkout.site_otec.checkout_1',
  metadataRefs: ['metadata.site_checkout_intent.site_otec'],
  method: 'POST',
  ownerUserId: null,
  path: '/checkout/consultation-deposit',
  price: amount,
  productId:
    'site_payment:site_otec:version_site_otec_v5:product:consultation_deposit',
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:site_checkout:site_otec:checkout_1',
  spendCap: amount,
  status: 'issued',
  surface: 'site_checkout',
}

const checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord = {
  amount,
  archivedAt: null,
  cancelReturnPath: '/pricing',
  catalogRef: 'catalog.site_otec.payments.v1',
  challengeRef: challenge.challengeRef,
  checkoutIntentRef: 'site_checkout_intent.site_otec.checkout_1',
  checkoutLaunchPath: '/checkout/consultation-deposit',
  checkoutRef: 'mdk_checkout.site_otec.checkout_1',
  checkoutUrlRef: 'mdk_checkout_url.site_otec.checkout_1',
  createdAt: '2026-06-07T11:00:00.000Z',
  environment: 'sandbox',
  hostedCheckoutProjectionJson: '{}',
  id: 'site_mdk_checkout_intent_site_otec_checkout_1',
  idempotencyKeyHash: 'hash.site_mdk_checkout_intent.site_otec.checkout_1',
  metadataRefs: ['metadata.site_mdk_checkout_intent.site_otec'],
  productId: challenge.productId,
  providerRef: 'provider.openagents.hosted_mdk.fake',
  publicProjectionJson: '{}',
  sandbox: true,
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v5',
  status: 'pending_payment',
  successReturnPath: '/checkout/thanks',
  updatedAt: '2026-06-07T11:10:00.000Z',
}

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: challenge.actorRef,
  amount,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: '2026-06-07T11:30:00.000Z',
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
  createdAt: '2026-06-07T11:31:00.000Z',
  entitlementRef: receipt.entitlementRef,
  expiresAt: '2026-06-08T11:31:00.000Z',
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

const providerEvent: OpenAgentsSiteMdkProviderEvent = {
  challengeRef: challenge.challengeRef,
  checkoutRef: checkoutIntent.checkoutRef,
  checkoutStatus: 'payment_received',
  environment: checkoutIntent.environment,
  eventBodyDigestRef: 'sha256:provider_event:site_otec:checkout_1',
  eventKind: 'payment_received',
  eventRef: 'event.site_mdk.site_otec.checkout_1.payment_received',
  fakeProvider: true,
  metadataRefs: ['metadata.site_mdk_reconciliation.checkout_1'],
  occurredAt: '2026-06-07T11:30:00.000Z',
  productId: challenge.productId,
  providerEventRef: 'provider_event.mdk.fake.checkout_1.payment_received',
  providerRef: checkoutIntent.providerRef,
  sandbox: checkoutIntent.sandbox,
  signatureBindingRef: null,
  signatureVerified: false,
  siteId: checkoutIntent.siteId,
  siteVersionId: checkoutIntent.siteVersionId,
}

const existingReconciliation: BuyerPaymentReconciliationEventRecord = {
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: '2026-06-07T11:30:20.000Z',
  eventRef: 'event.site_mdk.existing.checkout_1.payment_received',
  externalEventRef: providerEvent.providerEventRef,
  id: 'buyer_payment_reconciliation_existing_checkout_1',
  idempotencyKeyHash: providerEvent.eventBodyDigestRef,
  metadataRefs: ['metadata.site_mdk_reconciliation.matched'],
  productId: challenge.productId,
  providerRef: checkoutIntent.providerRef,
  publicProjectionJson: '{}',
  receiptRef: receipt.receiptRef,
  resultRef: 'result.site_mdk_reconciliation.matched',
  status: 'matched',
}

const baseInput = (
  overrides: Partial<OpenAgentsSiteMdkReconciliationWorkerInput> = {},
): OpenAgentsSiteMdkReconciliationWorkerInput => ({
  audience: 'agent',
  buyerPaymentChallenge: challenge,
  checkoutIntent,
  entitlement: null,
  existingReconciliationEvents: [],
  incomingProviderEvent: null,
  nowIso: '2026-06-07T11:40:00.000Z',
  providerStatusCheck: null,
  receipt: null,
  retryPlan: {
    attempt: 1,
    backoffSeconds: 60,
    maxAttempts: 5,
    nextAttemptAt: '2026-06-07T11:41:00.000Z',
  },
  source: 'scheduled',
  staleAfterSeconds: 3600,
  ...overrides,
})

describe('OpenAgents Site MDK reconciliation worker', () => {
  test('plans receipt repair once when verified payment is seen without a receipt', () => {
    const projection = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      incomingProviderEvent: providerEvent,
      source: 'webhook',
    }))

    expect(S.decodeUnknownSync(
      OpenAgentsSiteMdkReconciliationWorkerProjection,
    )(projection)).toEqual(projection)
    expect(projection.status).toBe('receipt_created')
    expect(projection.actionRefs).toEqual([
      'record_reconciliation_event_once',
      'create_receipt_once',
    ])
    expect(projection.sideEffectSummary).toMatchObject({
      createsReceipt: true,
      createsEntitlement: false,
      mutatesPayout: false,
      recordsReconciliationEvent: true,
    })
    expect(openAgentsSiteMdkReconciliationWorkerHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('plans entitlement repair once when receipt exists but entitlement is missing', () => {
    const projection = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      incomingProviderEvent: providerEvent,
      receipt,
      source: 'queue',
    }))

    expect(projection.status).toBe('entitlement_created')
    expect(projection.actionRefs).toEqual([
      'record_reconciliation_event_once',
      'create_entitlement_once',
    ])
    expect(projection.receiptRef).toBe(receipt.receiptRef)
    expect(projection.sideEffectSummary.createsEntitlement).toBe(true)
  })

  test('treats duplicate and replayed settled provider events as replayed', () => {
    const duplicate = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      existingReconciliationEvents: [existingReconciliation],
      incomingProviderEvent: providerEvent,
      receipt,
    }))
    const settledReplay = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      checkoutIntent: {
        ...checkoutIntent,
        status: 'payment_received',
      },
      entitlement,
      existingReconciliationEvents: [existingReconciliation],
      incomingProviderEvent: providerEvent,
      receipt,
    }))

    expect(duplicate.status).toBe('replayed')
    expect(duplicate.duplicateRefs).toEqual(expect.arrayContaining([
      existingReconciliation.eventRef,
      existingReconciliation.externalEventRef,
    ]))
    expect(duplicate.actionRefs).toEqual([])
    expect(settledReplay.status).toBe('replayed')
    expect(settledReplay.sideEffectSummary.createsEntitlement).toBe(false)
  })

  test('detects out-of-order provider status as conflict', () => {
    const projection = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      checkoutIntent: {
        ...checkoutIntent,
        status: 'payment_received',
      },
      entitlement,
      providerStatusCheck: {
        checkedAt: '2026-06-07T11:40:00.000Z',
        checkoutRef: checkoutIntent.checkoutRef,
        checkoutStatus: 'pending_payment',
        eventBodyDigestRef: 'sha256:provider_status:site_otec:checkout_1',
        providerAvailable: true,
        providerEventRef: 'provider_status.mdk.fake.checkout_1.pending',
        providerRef: checkoutIntent.providerRef,
        statusCheckRef: 'status_check.site_otec.checkout_1.pending',
        statusCheckSupported: true,
      },
      receipt,
    }))

    expect(projection.status).toBe('conflict')
    expect(projection.statusCode).toBe(409)
    expect(projection.conflictRefs).toContain(
      'conflict.site_mdk_worker.out_of_order_provider_status',
    )
    expect(projection.actionRefs).toEqual(['request_operator_review'])
  })

  test('classifies stale pending checkouts, expired challenges, and unavailable provider status', () => {
    const stale = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      checkoutIntent: {
        ...checkoutIntent,
        updatedAt: '2026-06-07T09:00:00.000Z',
      },
    }))
    const expired = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      buyerPaymentChallenge: {
        ...challenge,
        expiresAt: '2026-06-07T10:00:00.000Z',
      },
    }))
    const unavailable = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      providerStatusCheck: {
        checkedAt: '2026-06-07T11:40:00.000Z',
        checkoutRef: checkoutIntent.checkoutRef,
        checkoutStatus: 'pending_payment',
        eventBodyDigestRef: null,
        providerAvailable: false,
        providerEventRef: null,
        providerRef: checkoutIntent.providerRef,
        statusCheckRef: 'status_check.site_otec.checkout_1.unavailable',
        statusCheckSupported: true,
      },
    }))

    expect(stale.status).toBe('stale')
    expect(stale.retryAllowed).toBe(true)
    expect(expired.status).toBe('expired')
    expect(expired.actionRefs).toEqual([
      'expire_payment_challenge',
      'expire_checkout_intent',
    ])
    expect(unavailable.status).toBe('provider_unavailable')
    expect(unavailable.statusCode).toBe(503)
    expect(unavailable.retryAllowed).toBe(true)
  })

  test('keeps public projection redacted while exposing safe operator refs', () => {
    const agentProjection = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      incomingProviderEvent: providerEvent,
      receipt,
    }))
    const operatorProjection = planOpenAgentsSiteMdkReconciliationWorker(baseInput({
      audience: 'operator',
      incomingProviderEvent: providerEvent,
      receipt,
    }))

    expect(agentProjection.operatorRefs).toEqual([])
    expect(operatorProjection.operatorRefs).toEqual(expect.arrayContaining([
      checkoutIntent.checkoutIntentRef,
      providerEvent.providerEventRef,
      providerEvent.eventBodyDigestRef,
    ]))
    expect(
      openAgentsSiteMdkReconciliationWorkerHasPrivateMaterial(operatorProjection),
    ).toBe(false)
  })

  test('rejects raw provider payloads and payment secrets before planning', () => {
    expect(() =>
      planOpenAgentsSiteMdkReconciliationWorker(baseInput({
        checkoutIntent: {
          ...checkoutIntent,
          hostedCheckoutProjectionJson: '{"invoice":"lnbc2500n1raw"}',
        },
      })),
    ).toThrow(OpenAgentsSiteMdkReconciliationWorkerUnsafe)
    expect(() =>
      planOpenAgentsSiteMdkReconciliationWorker(baseInput({
        incomingProviderEvent: {
          ...providerEvent,
          metadataRefs: ['ben@example.com'],
        },
      })),
    ).toThrow(OpenAgentsSiteMdkReconciliationWorkerUnsafe)
  })
})
