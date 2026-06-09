import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
} from './buyer-payment-ledger'
import type { OpenAgentsSiteMdkCheckoutIntentRecord } from './site-mdk-checkout-intents'
import {
  OpenAgentsSitePaymentProofProjection,
  OpenAgentsSitePaymentProofUnsafe,
  openAgentsSitePaymentProofHasPrivateMaterial,
  projectOpenAgentsSitePaymentProof,
} from './site-payment-proof'

const now = '2026-06-07T13:00:00.000Z'
const amount = {
  amountMinorUnits: 1_000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
} as const

const checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord = {
  amount,
  archivedAt: null,
  cancelReturnPath: '/checkout/cancel',
  catalogRef: 'site_payment:site_demo:v1:product:deposit',
  challengeRef: 'challenge.site_checkout.site_demo.proof_demo',
  checkoutIntentRef: 'site_checkout_intent_site_demo_proof_demo',
  checkoutLaunchPath: '/checkout/site_demo_proof_demo',
  checkoutRef: 'mdk_checkout.site_demo.proof_demo',
  checkoutUrlRef: 'mdk_checkout_url.site_demo.proof_demo',
  createdAt: now,
  environment: 'sandbox',
  hostedCheckoutProjectionJson: '{}',
  id: 'site_checkout_intent_site_demo_proof_demo',
  idempotencyKeyHash: 'hash.site_checkout.site_demo.proof_demo',
  metadataRefs: ['metadata.site_payment.site_demo'],
  productId: 'deposit',
  providerRef: 'provider.openagents.hosted_mdk.fake',
  publicProjectionJson: '{}',
  sandbox: true,
  siteId: 'site_demo',
  siteVersionId: 'version_site_demo_v1',
  status: 'payment_received',
  successReturnPath: '/checkout/success',
  updatedAt: now,
}

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'site.site_demo.checkout',
  archivedAt: null,
  challengeRef: checkoutIntent.challengeRef,
  createdAt: now,
  expiresAt: '2026-06-07T13:10:00.000Z',
  id: 'buyer_payment_challenge_site_demo_proof_demo',
  idempotencyKeyHash: checkoutIntent.idempotencyKeyHash,
  metadataRefs: ['metadata.site_payment.challenge.site_demo'],
  method: 'POST',
  ownerUserId: null,
  path: '/checkout/deposit',
  price: amount,
  productId: checkoutIntent.productId,
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:site_checkout:site_demo:proof_demo',
  spendCap: amount,
  status: 'issued',
  surface: 'site_checkout',
}

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: challenge.actorRef,
  amount,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: now,
  entitlementRef: 'entitlement.site_payment.site_demo.proof_demo',
  id: 'receipt_site_payment_site_demo_proof_demo',
  metadataRefs: ['metadata.site_payment.receipt.site_demo'],
  ownerUserId: null,
  productId: checkoutIntent.productId,
  publicProjectionJson: '{}',
  receiptRef: 'receipt.site_payment.site_demo.proof_demo',
  redactedPaymentRef: 'redacted_payment.site_payment.site_demo.proof_demo',
  status: 'issued',
  surface: 'site_checkout',
}

const entitlement: BuyerPaymentEntitlementRecord = {
  actorRef: challenge.actorRef,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  consumedAt: null,
  createdAt: now,
  entitlementRef: receipt.entitlementRef,
  expiresAt: null,
  id: receipt.entitlementRef,
  ownerUserId: null,
  productId: checkoutIntent.productId,
  receiptRef: receipt.receiptRef,
  scopeRefs: ['entitlement_scope.site_payment.product'],
  status: 'active',
  surface: 'site_checkout',
}

const reconciliationEvent: BuyerPaymentReconciliationEventRecord = {
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: now,
  eventRef: 'event.site_mdk.site_demo.proof_demo',
  externalEventRef: 'evt.public.site_demo.proof_demo',
  id: 'event.site_mdk.site_demo.proof_demo',
  idempotencyKeyHash: 'sha256:site_mdk:site_demo:proof_demo',
  metadataRefs: ['metadata.site_mdk.reconciliation.site_demo'],
  productId: checkoutIntent.productId,
  providerRef: checkoutIntent.providerRef,
  publicProjectionJson: '{}',
  receiptRef: receipt.receiptRef,
  resultRef: 'result.site_mdk_reconciliation.matched',
  status: 'matched',
}

const projection = (
  overrides: Partial<Parameters<typeof projectOpenAgentsSitePaymentProof>[0]> = {},
) =>
  projectOpenAgentsSitePaymentProof({
    audience: 'public',
    buyerPaymentChallenge: challenge,
    checkoutIntent,
    entitlement,
    receipt,
    reconciliationEvent,
    ...overrides,
  })

describe('OpenAgents Site payment proof', () => {
  test('projects verified checkout, receipt, reconciliation, and entitlement evidence', () => {
    const proof = projection()

    expect(S.decodeUnknownSync(OpenAgentsSitePaymentProofProjection)(proof))
      .toEqual(proof)
    expect(proof).toMatchObject({
      acceptedWorkPayoutAuthority: false,
      checkoutIntentRef: checkoutIntent.checkoutIntentRef,
      checkoutStatus: 'payment_received',
      claimState: 'entitlement_active',
      entitlementState: 'active',
      finalSettlementClaim: false,
      implementationState: 'sandbox',
      payoutClaimAllowed: false,
      proofState: 'verified_entitlement',
      providerPayoutAuthority: false,
      settlementClaimAllowed: false,
      siteId: 'site_demo',
    })
    expect(proof.proofRefs).toEqual(
      expect.arrayContaining([
        challenge.challengeRef,
        receipt.receiptRef,
        entitlement.entitlementRef,
        reconciliationEvent.eventRef,
      ]),
    )
    expect(openAgentsSitePaymentProofHasPrivateMaterial(proof)).toBe(false)
  })

  test('reports pending checkout and pending reconciliation without overclaiming', () => {
    const pendingCheckout = projection({
      checkoutIntent: {
        ...checkoutIntent,
        status: 'pending_payment',
      },
      entitlement: null,
      receipt: null,
      reconciliationEvent: null,
    })
    const pendingReconciliation = projection({
      entitlement: null,
      receipt,
      reconciliationEvent: null,
    })

    expect(pendingCheckout).toMatchObject({
      claimState: 'checkout_intent_recorded',
      entitlementState: 'none',
      proofState: 'pending_checkout',
      settlementClaimAllowed: false,
    })
    expect(pendingReconciliation).toMatchObject({
      claimState: 'buyer_payment_observed',
      entitlementState: 'pending_reconciliation',
      proofState: 'pending_reconciliation',
      settlementClaimAllowed: false,
    })
  })

  test('distinguishes sandbox from live provider classification', () => {
    const liveProof = projection({
      checkoutIntent: {
        ...checkoutIntent,
        environment: 'production',
        providerRef: 'provider.openagents.hosted_mdk.live',
        sandbox: false,
      },
    })

    expect(liveProof).toMatchObject({
      environment: 'production',
      implementationState: 'live_provider',
      sandbox: false,
    })
  })

  test('splits public, customer, and operator redaction without leaking settlement authority', () => {
    const publicProof = projection({ audience: 'public' })
    const customerProof = projection({ audience: 'customer' })
    const operatorProof = projection({ audience: 'operator' })

    expect(publicProof.receipt?.redactedPaymentRef).toBe(null)
    expect(customerProof.receipt?.redactedPaymentRef).toBe(
      receipt.redactedPaymentRef,
    )
    expect(operatorProof.reconciliationEvent?.operatorRefs).toEqual(
      expect.arrayContaining([
        reconciliationEvent.providerRef,
        reconciliationEvent.externalEventRef,
      ]),
    )
    expect(operatorProof.acceptedWorkPayoutAuthority).toBe(false)
    expect(operatorProof.finalSettlementClaim).toBe(false)
  })

  test('blocks mismatched or unsafe payment material', () => {
    const blockedProof = projection({
      receipt: {
        ...receipt,
        challengeRef: 'challenge.site_checkout.other',
      },
    })

    expect(blockedProof).toMatchObject({
      claimState: 'no_payment_claim',
      proofState: 'blocked',
    })
    expect(blockedProof.receipt).toBe(null)

    expect(() =>
      projection({
        receipt: {
          ...receipt,
          redactedPaymentRef: 'lnbc10n1rawinvoice',
        },
      }),
    ).toThrow(OpenAgentsSitePaymentProofUnsafe)
  })
})
