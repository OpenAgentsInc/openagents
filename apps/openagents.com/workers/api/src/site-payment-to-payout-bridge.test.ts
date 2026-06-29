import { describe, expect, test } from 'vitest'

import {
  type BuyerPaymentReceiptRecord,
  type BuyerPaymentReconciliationEventRecord,
} from './buyer-payment-ledger'
import { OpenAgentsSiteCheckoutReturnProjection } from './site-checkout-return'
import { OpenAgentsSiteMdkCheckoutIntentRecord } from './site-mdk-checkout-intents'
import {
  buildOpenAgentsSitePaymentToPayoutBridge,
  openAgentsSitePaymentToPayoutBridgeHasPrivateMaterial,
  type OpenAgentsSitePaymentToPayoutBridgeInput,
  type OpenAgentsSitePaymentToPayoutBridgeRequest,
} from './site-payment-to-payout-bridge'
import {
  projectPylonV02OmegaReleaseGate,
  readyPylonV02OmegaReleaseGateRecord,
} from './pylon-v02-omega-release-gate'

const now = '2026-06-07T09:20:00.000Z'

const amount = {
  amountMinorUnits: 1_000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
} as const

const spendCap = {
  amountMinorUnits: 2_000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
} as const

const checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord = {
  amount,
  archivedAt: null,
  cancelReturnPath: '/checkout/cancel',
  catalogRef: 'product.demo',
  challengeRef: 'challenge.site_checkout.demo.bridge',
  checkoutIntentRef: 'site_checkout_intent_demo_bridge',
  checkoutLaunchPath: '/checkout/demo',
  checkoutRef: 'mdk_checkout.public.demo.bridge',
  checkoutUrlRef: 'mdk_checkout_url.public.demo.bridge',
  createdAt: now,
  environment: 'sandbox',
  hostedCheckoutProjectionJson: '{}',
  id: 'site_checkout_intent_demo_bridge',
  idempotencyKeyHash: 'hash.site_checkout.demo.bridge',
  metadataRefs: ['metadata.site_checkout.demo'],
  productId: 'product.demo',
  providerRef: 'provider.openagents.hosted_mdk.fake',
  publicProjectionJson: '{}',
  sandbox: true,
  siteId: 'site_demo',
  siteVersionId: 'version_demo_v1',
  status: 'payment_received',
  successReturnPath: '/checkout/success',
  updatedAt: now,
}

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: 'site.site_demo.checkout',
  amount,
  archivedAt: null,
  challengeRef: checkoutIntent.challengeRef,
  createdAt: now,
  entitlementRef: 'entitlement.site_payment.demo.bridge',
  id: 'receipt_site_payment_demo_bridge',
  metadataRefs: ['metadata.site_payment.receipt.demo'],
  ownerUserId: null,
  productId: checkoutIntent.productId,
  publicProjectionJson: '{}',
  receiptRef: 'receipt.site_payment.demo.bridge',
  redactedPaymentRef: 'payment.redacted.demo.bridge',
  status: 'issued',
  surface: 'site_checkout',
}

const reconciliationEvent: BuyerPaymentReconciliationEventRecord = {
  archivedAt: null,
  challengeRef: checkoutIntent.challengeRef,
  createdAt: now,
  eventRef: 'event.site_mdk.demo.bridge',
  externalEventRef: 'evt.public.site_mdk.demo.bridge',
  id: 'event.site_mdk.demo.bridge',
  idempotencyKeyHash: 'sha256:site_mdk:demo:bridge',
  metadataRefs: ['metadata.site_mdk.reconciliation.demo'],
  productId: checkoutIntent.productId,
  providerRef: checkoutIntent.providerRef,
  publicProjectionJson: '{}',
  receiptRef: receipt.receiptRef,
  resultRef: 'result.site_mdk_reconciliation.matched',
  status: 'matched',
}

const request: OpenAgentsSitePaymentToPayoutBridgeRequest = {
  acceptedWorkRefs: ['accepted_work.public.demo.bridge'],
  adapterKind: 'simulation',
  amount,
  artanisDispatchRef: 'artanis.dispatch.public.demo.bridge',
  assignmentRef: 'assignment.public.demo.bridge',
  checkoutIntentRef: checkoutIntent.checkoutIntentRef,
  metadataRefs: ['metadata.site_payment_to_payout.demo'],
  ownerUserId: null,
  payoutTargetApprovalRef: 'approval.public.pylon.demo.bridge',
  payoutTargetRef: 'payout_target.public.pylon.demo.bridge',
  policySnapshotRef: 'policy_snapshot.public.demo.bridge',
  pylonJobRef: 'pylon_job.public.demo.bridge',
  spendCap,
  walletReadiness: 'ready',
}

const readyGate = projectPylonV02OmegaReleaseGate(
  readyPylonV02OmegaReleaseGateRecord(),
  'operator',
  now,
)

const input = (
  overrides: Partial<OpenAgentsSitePaymentToPayoutBridgeInput> = {},
): OpenAgentsSitePaymentToPayoutBridgeInput => ({
  audience: 'operator',
  existingPayoutIntentForBuyerPaymentRef: null,
  idempotencyKey: 'bridge-idempotency-1',
  nowIso: now,
  receipt,
  reconciliationEvent,
  releaseGate: readyGate,
  request,
  returnProjection: null,
  siteCheckoutIntent: checkoutIntent,
  ...overrides,
})

describe('OpenAgents Site payment-to-payout bridge', () => {
  test('builds an authority-ready payout intent only from verified buyer payment evidence', () => {
    const result = buildOpenAgentsSitePaymentToPayoutBridge(input())

    expect(result._tag).toBe('Ready')

    if (result._tag !== 'Ready') {
      throw new Error('expected ready bridge')
    }

    expect(result.intent).toMatchObject({
      acceptedWorkRefs: ['accepted_work.public.demo.bridge'],
      buyerPaymentRef: receipt.receiptRef,
      idempotencyKeyHash: 'hash.site_payment_to_payout.site_demo.bridge-idempotency-1',
      payoutTargetApprovalRef: 'approval.public.pylon.demo.bridge',
      sourceKind: 'pylon_marketplace_assignment',
    })
    expect(result.projection.checkoutReturnAuthority).toBe(false)
    expect(result.projection.state).toBe('payout_intent_ready')
    expect(result.projection.settlementClaimAllowed).toBe(false)
    expect(
      openAgentsSitePaymentToPayoutBridgeHasPrivateMaterial(result.projection),
    ).toBe(false)
  })

  test('blocks duplicate buyer receipts independently from idempotency', () => {
    const result = buildOpenAgentsSitePaymentToPayoutBridge(
      input({
        existingPayoutIntentForBuyerPaymentRef: {
          acceptedWorkRefs: ['accepted_work.public.demo.bridge'],
          actorRef: 'agent.artanis',
          adapterKind: 'simulation',
          amount,
          archivedAt: null,
          artanisDispatchRef: null,
          assignmentRef: null,
          buyerPaymentRef: receipt.receiptRef,
          createdAt: now,
          id: 'nexus_treasury_payout_intent_existing',
          idempotencyKeyHash: 'hash.site_payment_to_payout.site_demo.old',
          metadataRefs: [],
          ownerUserId: null,
          payoutIntentRef: 'payout_intent.site_payment_to_payout.existing',
          payoutTargetApprovalRef: 'approval.public.pylon.demo.bridge',
          payoutTargetRef: 'payout_target.public.pylon.demo.bridge',
          policySnapshotRef: 'policy_snapshot.public.demo.bridge',
          publicProjectionJson: '{}',
          pylonJobRef: null,
          sourceKind: 'pylon_marketplace_assignment',
          spendCap,
          status: 'approved',
          updatedAt: now,
        },
      }),
    )

    expect(result._tag).toBe('Blocked')
    expect(result.projection.blockerRefs).toContain(
      'duplicate_buyer_payment_ref',
    )
  })

  test('blocks unverified return-only or missing reconciliation evidence', () => {
    const returnProjection: typeof OpenAgentsSiteCheckoutReturnProjection.Type = {
      audience: 'agent',
      buyerPaymentChallenge: null,
      cleanReturnPath: '/checkout/success',
      entitlement: null,
      entitlementStatus: 'pending_reconciliation',
      finalEntitlementCreated: false,
      hostedCheckout: null,
      reasonRefs: ['reason.checkout_return_not_authority'],
      receipt: null,
      returnAction: 'success',
      returnState: 'success',
      serverRefs: {
        buyerPaymentChallengeRef: checkoutIntent.challengeRef,
        checkoutIntentRef: checkoutIntent.checkoutIntentRef,
        checkoutRef: checkoutIntent.checkoutRef,
        entitlementRef: null,
        receiptRef: null,
      },
      uiPrimitiveRefs: [],
    }
    const result = buildOpenAgentsSitePaymentToPayoutBridge(
      input({
        receipt: null,
        reconciliationEvent: null,
        returnProjection,
      }),
    )

    expect(result._tag).toBe('Blocked')
    expect(result.projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'checkout_return_not_authority',
        'missing_verified_buyer_payment',
      ]),
    )
  })

  test('blocks missing accepted work, payout approval, stale wallet readiness, and spend-cap rejection', () => {
    const cases = [
      {
        overrides: { request: { ...request, acceptedWorkRefs: [] } },
        reason: 'missing_accepted_work_ref',
      },
      {
        overrides: { request: { ...request, payoutTargetApprovalRef: null } },
        reason: 'missing_payout_target_approval',
      },
      {
        overrides: { request: { ...request, walletReadiness: 'stale' as const } },
        reason: 'stale_or_absent_wallet_readiness',
      },
      {
        overrides: {
          request: {
            ...request,
            amount: {
              amountMinorUnits: 3_000,
              asset: 'bitcoin',
              denomination: 'bitcoin_millisatoshi',
            } as const,
          },
        },
        reason: 'spend_cap_exceeded',
      },
    ] as const

    for (const item of cases) {
      const result = buildOpenAgentsSitePaymentToPayoutBridge(
        input(item.overrides),
      )

      expect(result._tag).toBe('Blocked')
      expect(result.projection.blockerRefs).toContain(item.reason)
    }
  })

  test('blocks product-to-payout claims when real movement release-gate evidence is missing', () => {
    const blockedGate = {
      ...readyGate,
      evidenceRefs: readyGate.evidenceRefs.filter(
        ref => !ref.includes('issue_431'),
      ),
    }
    const result = buildOpenAgentsSitePaymentToPayoutBridge(
      input({ releaseGate: blockedGate }),
    )

    expect(result._tag).toBe('Blocked')
    expect(result.projection.blockerRefs).toContain('missing_real_movement_gate')
  })

  test('redacts public projection boundaries', () => {
    const result = buildOpenAgentsSitePaymentToPayoutBridge(
      input({ audience: 'public' }),
    )

    expect(result.projection.operatorRefs).toEqual([])
    expect(result.projection.acceptedWorkRefs).toEqual([])
    expect(JSON.stringify(result.projection)).not.toMatch(
      /(lnbc|lntb|payment_preimage|mnemonic|wallet_secret|wallet_state|secret|@)/i,
    )
  })
})
