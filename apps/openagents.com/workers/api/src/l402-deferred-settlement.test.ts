import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentReceiptRecord,
} from './buyer-payment-ledger'
import type { OpenAgentsL402VerificationResult } from './l402-credential-service'
import {
  OpenAgentsL402DeferredSettlementProjection,
  OpenAgentsL402DeferredSettlementUnsafe,
  evaluateOpenAgentsL402DeferredSettlement,
  openAgentsL402DeferredSettlementHasPrivateMaterial,
} from './l402-deferred-settlement'

const now = '2026-06-07T12:00:00.000Z'

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'agent:user_123',
  archivedAt: null,
  challengeRef: 'challenge.site_checkout.otec.report',
  createdAt: '2026-06-07T11:55:00.000Z',
  expiresAt: '2026-06-07T12:10:00.000Z',
  id: 'buyer_payment_challenge_otec_report',
  idempotencyKeyHash: 'hash.challenge.otec_report',
  metadataRefs: ['metadata.site_payment.deferred'],
  method: 'GET',
  ownerUserId: 'user_owner_123',
  path: '/api/actions/download-report',
  price: {
    amountMinorUnits: 25_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  productId: 'product.site_otec.report',
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:request_body_digest',
  spendCap: {
    amountMinorUnits: 25_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  status: 'issued',
  surface: 'site_checkout',
}

const verification: OpenAgentsL402VerificationResult = {
  credentialRef: 'credential.l402.site_otec.report',
  payload: {
    amount: challenge.price,
    challengeRef: challenge.challengeRef,
    credentialRef: 'credential.l402.site_otec.report',
    endpointRef: 'endpoint.site_otec.report',
    entitlementScopeRefs: ['entitlement.site_otec.report'],
    expiresAt: challenge.expiresAt,
    idempotencyKeyHash: 'hash.redemption.otec_report',
    issuedAt: challenge.createdAt,
    method: challenge.method,
    path: challenge.path,
    paymentHashRef: 'payment_hash_ref.redacted.site_otec.report',
    productId: challenge.productId,
    replayNonceRef: 'nonce.l402.site_otec.report',
    requestBodyDigest: challenge.requestBodyDigest,
    version: 'oa-l402-v1',
  },
  reasonRef: 'reason.l402.valid',
  status: 'valid',
}

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: challenge.actorRef,
  amount: challenge.price,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  createdAt: now,
  entitlementRef: 'entitlement.site_otec.report',
  id: 'buyer_payment_receipt_otec_report',
  metadataRefs: ['metadata.receipt.redacted'],
  ownerUserId: challenge.ownerUserId,
  productId: challenge.productId,
  publicProjectionJson: '{}',
  receiptRef: 'receipt.site_otec.report',
  redactedPaymentRef: 'payment_ref.redacted.site_otec.report',
  status: 'issued',
  surface: challenge.surface,
}

const entitlement: BuyerPaymentEntitlementRecord = {
  actorRef: challenge.actorRef,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  consumedAt: null,
  createdAt: now,
  entitlementRef: receipt.entitlementRef,
  expiresAt: '2026-06-08T12:00:00.000Z',
  id: 'buyer_payment_entitlement_otec_report',
  ownerUserId: challenge.ownerUserId,
  productId: challenge.productId,
  receiptRef: receipt.receiptRef,
  scopeRefs: ['entitlement.site_otec.report'],
  status: 'active',
  surface: challenge.surface,
}

const baseInput = {
  actorRef: challenge.actorRef,
  audience: 'agent',
  buyerPaymentChallenge: challenge,
  endpointRef: 'endpoint.site_otec.report',
  existingEntitlement: null,
  existingReceipt: null,
  expectedEntitlementScopeRefs: ['entitlement.site_otec.report'],
  idempotencyKeyHash: 'hash.deferred_settlement.otec_report',
  manualApprovalRef: null,
  metadataRefs: ['metadata.deferred_settlement.test'],
  mode: 'deferred_until_success',
  nowIso: now,
  productId: challenge.productId,
  surface: 'site_checkout',
  verification,
  workResult: {
    artifactReceiptRef: null,
    failureRef: null,
    responseCloseoutRef: null,
    retryable: false,
    status: 'succeeded',
  },
} as const

describe('OpenAgents L402 deferred settlement', () => {
  test('settles only after the configured success boundary is reached', () => {
    const projection = evaluateOpenAgentsL402DeferredSettlement(baseInput)

    expect(S.decodeUnknownSync(OpenAgentsL402DeferredSettlementProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.status).toBe('settled')
    expect(projection.statusCode).toBe(200)
    expect(projection.credentialConsumed).toBe(true)
    expect(projection.credentialReusable).toBe(false)
    expect(projection.safeBody).toMatchObject({
      receiptRef: 'receipt.l402_deferred.product.site_otec.report.challenge.site_checkout.otec.report',
      status: 'settled',
    })
    expect(openAgentsL402DeferredSettlementHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps credentials reusable when protected work fails before charge', () => {
    const projection = evaluateOpenAgentsL402DeferredSettlement({
      ...baseInput,
      workResult: {
        artifactReceiptRef: null,
        failureRef: 'failure.site_otec.build.retryable',
        responseCloseoutRef: null,
        retryable: true,
        status: 'failed',
      },
    })

    expect(projection.status).toBe('retryable_failure')
    expect(projection.statusCode).toBe(500)
    expect(projection.credentialConsumed).toBe(false)
    expect(projection.credentialReusable).toBe(true)
    expect(projection.failureRef).toBe('failure.site_otec.build.retryable')
    expect(projection.reasonRefs).toContain(
      'reason.l402_deferred_settlement.work_failed_before_charge',
    )
  })

  test('waits for artifact receipt before deferred artifact settlement', () => {
    const pending = evaluateOpenAgentsL402DeferredSettlement({
      ...baseInput,
      mode: 'deferred_until_artifact_receipt',
      workResult: {
        artifactReceiptRef: null,
        failureRef: null,
        responseCloseoutRef: null,
        retryable: false,
        status: 'succeeded',
      },
    })

    const settled = evaluateOpenAgentsL402DeferredSettlement({
      ...baseInput,
      mode: 'deferred_until_artifact_receipt',
      workResult: {
        artifactReceiptRef: 'artifact.site_otec.report.download',
        failureRef: null,
        responseCloseoutRef: null,
        retryable: false,
        status: 'artifact_receipt_created',
      },
    })

    expect(pending.status).toBe('settlement_pending')
    expect(pending.statusCode).toBe(202)
    expect(pending.credentialReusable).toBe(true)
    expect(settled.status).toBe('settled')
    expect(settled.credentialConsumed).toBe(true)
  })

  test('allows matching active entitlement without requiring another payment', () => {
    const projection = evaluateOpenAgentsL402DeferredSettlement({
      ...baseInput,
      existingEntitlement: entitlement,
      verification: null,
      workResult: {
        artifactReceiptRef: null,
        failureRef: null,
        responseCloseoutRef: null,
        retryable: false,
        status: 'not_started',
      },
    })

    expect(projection.status).toBe('allow')
    expect(projection.statusCode).toBe(200)
    expect(projection.credentialConsumed).toBe(false)
    expect(projection.entitlement?.entitlementRef).toBe(entitlement.entitlementRef)
  })

  test('collapses existing receipt and entitlement into settled projection', () => {
    const projection = evaluateOpenAgentsL402DeferredSettlement({
      ...baseInput,
      existingEntitlement: entitlement,
      existingReceipt: receipt,
      verification: null,
    })

    expect(projection.status).toBe('settled')
    expect(projection.receipt?.receiptRef).toBe(receipt.receiptRef)
    expect(projection.entitlement?.entitlementRef).toBe(entitlement.entitlementRef)
  })

  test('blocks expired challenges and requires payment for invalid credentials', () => {
    const blocked = evaluateOpenAgentsL402DeferredSettlement({
      ...baseInput,
      buyerPaymentChallenge: {
        ...challenge,
        expiresAt: '2026-06-07T11:59:00.000Z',
      },
    })
    const paymentRequired = evaluateOpenAgentsL402DeferredSettlement({
      ...baseInput,
      verification: {
        credentialRef: null,
        payload: null,
        reasonRef: 'reason.l402.proof_missing',
        status: 'proof_missing',
      },
    })

    expect(blocked.status).toBe('blocked')
    expect(blocked.statusCode).toBe(403)
    expect(paymentRequired.status).toBe('payment_required')
    expect(paymentRequired.statusCode).toBe(402)
    expect(paymentRequired.buyerPaymentChallenge?.challengeRef).toBe(
      challenge.challengeRef,
    )
  })

  test('rejects raw payment material in input', () => {
    expect(() =>
      evaluateOpenAgentsL402DeferredSettlement({
        ...baseInput,
        verification: {
          ...verification,
          payload: verification.payload === null
            ? null
            : {
              ...verification.payload,
              paymentHashRef: 'lnbc2500n1rawinvoice',
            },
        },
      }),
    ).toThrow(OpenAgentsL402DeferredSettlementUnsafe)

    expect(() =>
      evaluateOpenAgentsL402DeferredSettlement({
        ...baseInput,
        manualApprovalRef: 'operator@example.com',
        mode: 'manual_operator_review',
      }),
    ).toThrow(OpenAgentsL402DeferredSettlementUnsafe)
  })
})
