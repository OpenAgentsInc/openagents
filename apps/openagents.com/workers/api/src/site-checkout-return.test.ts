import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentReceiptRecord,
} from './buyer-payment-ledger'
import {
  OpenAgentsSiteCheckoutReturnProjection,
  OpenAgentsSiteCheckoutReturnUnsafe,
  openAgentsSiteCheckoutReturnHasPrivateMaterial,
  projectOpenAgentsSiteCheckoutReturn,
} from './site-checkout-return'
import {
  projectOpenAgentsSiteCheckoutUiPrimitives,
  siteCheckoutUiPrimitivesFromCatalog,
} from './site-checkout-ui-primitives'
import { sitePaymentCatalogFromManifest } from './site-payment-catalog'

const fixedNow = new Date('2026-06-06T13:00:00.000Z')

const catalog = sitePaymentCatalogFromManifest({
  createdAt: '2026-06-06T12:00:00.000Z',
  deploymentId: 'deployment.site_otec.v5',
  manifest: {
    payments: {
      agentReadable: true,
      enabled: true,
      metadataRefs: ['metadata.site_payment.otec.v5'],
      paidActions: [],
      products: [
        {
          agentReadable: true,
          checkoutPath: '/checkout/consultation-deposit',
          customerDataRequirements: [],
          displayRef: 'display.consultation_deposit',
          entitlementScope: 'product',
          id: 'consultation_deposit',
          metadataRefs: ['metadata.product.consultation_deposit'],
          price: {
            amountMinorUnits: 2500,
            asset: 'usd',
            denomination: 'usd_cent',
          },
          publicProjectionState: 'listed',
          sandbox: true,
          settlementMode: 'checkout_only',
        },
      ],
      provider: 'openagents_hosted_mdk',
      sandboxDefault: true,
    },
  },
  manifestRef: 'manifest.site_otec.payments.v5',
  orderRef: 'order.site_otec',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v5',
  sourceManifestDigest: 'sha256:site_otec_manifest_v5',
  status: 'active',
  updatedAt: '2026-06-06T12:01:00.000Z',
  workroomRef: 'workroom.site_otec',
} as const)

const catalogItem = catalog.items[0]!

const uiPrimitives = projectOpenAgentsSiteCheckoutUiPrimitives(
  siteCheckoutUiPrimitivesFromCatalog({
    cancelPath: '/pricing',
    catalog,
    runtimeTarget: 'static',
    sourceSurface: 'generated_html',
    successPath: '/checkout/thanks',
  }),
  'agent',
)

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
  price: catalogItem.price,
  productId: catalogItem.catalogRef,
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:site_checkout:site_otec:checkout_1',
  spendCap: catalogItem.price,
  status: 'issued',
  surface: 'site_checkout',
}

const hostedCheckout = {
  acceptedWorkSettlementAuthority: false,
  amount: challenge.price,
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
  status: 'created',
} as const

const receipt: BuyerPaymentReceiptRecord = {
  actorRef: challenge.actorRef,
  amount: challenge.price,
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

const baseInput = {
  audience: 'agent',
  buyerPaymentChallenge: challenge,
  entitlement: null,
  hostedCheckout,
  nowEpochMillis: fixedNow.getTime(),
  observedReturnPath: '/checkout/thanks',
  receipt: null,
  returnAction: 'success',
  route: {
    cancelPath: '/pricing',
    checkoutIntentRef: 'site_checkout_intent.site_otec.checkout_1',
    checkoutRef: hostedCheckout.checkoutRef,
    siteId: 'site_otec',
    siteVersionId: 'version_site_otec_v5',
    successPath: '/checkout/thanks',
  },
  uiPrimitives,
} as const

describe('OpenAgents Site checkout return projection', () => {
  test('projects clean success returns without granting final entitlement', () => {
    const projection = projectOpenAgentsSiteCheckoutReturn(baseInput)

    expect(S.decodeUnknownSync(OpenAgentsSiteCheckoutReturnProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      cleanReturnPath: '/checkout/thanks',
      entitlementStatus: 'pending_reconciliation',
      finalEntitlementCreated: false,
      returnAction: 'success',
      returnState: 'success',
      serverRefs: {
        buyerPaymentChallengeRef: challenge.challengeRef,
        checkoutIntentRef: 'site_checkout_intent.site_otec.checkout_1',
        checkoutRef: hostedCheckout.checkoutRef,
      },
    })
    expect(projection.uiPrimitiveRefs).toEqual(
      expect.arrayContaining([
        'site_checkout_ui:site_otec:version_site_otec_v5:success_state',
        'site_checkout_ui:site_otec:version_site_otec_v5:entitlement_state',
      ]),
    )
    expect(openAgentsSiteCheckoutReturnHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('distinguishes paid and entitled states from receipt and entitlement records', () => {
    const paid = projectOpenAgentsSiteCheckoutReturn({
      ...baseInput,
      receipt,
    })
    const entitled = projectOpenAgentsSiteCheckoutReturn({
      ...baseInput,
      entitlement,
      receipt,
    })

    expect(paid.returnState).toBe('paid')
    expect(paid.entitlementStatus).toBe('pending_reconciliation')
    expect(paid.receipt?.receiptRef).toBe(receipt.receiptRef)
    expect(entitled.returnState).toBe('entitled')
    expect(entitled.entitlementStatus).toBe('active')
    expect(entitled.entitlement?.entitlementRef).toBe(
      entitlement.entitlementRef,
    )
  })

  test('projects cancel, expired, pending, and unpaid states', () => {
    const cancel = projectOpenAgentsSiteCheckoutReturn({
      ...baseInput,
      observedReturnPath: '/pricing',
      returnAction: 'cancel',
    })
    const expired = projectOpenAgentsSiteCheckoutReturn({
      ...baseInput,
      buyerPaymentChallenge: {
        ...challenge,
        expiresAt: '2026-06-06T12:30:00.000Z',
      },
      returnAction: 'status',
    })
    const pending = projectOpenAgentsSiteCheckoutReturn({
      ...baseInput,
      hostedCheckout: {
        ...hostedCheckout,
        status: 'pending_payment',
      },
      observedReturnPath: '/checkout/status',
      returnAction: 'status',
    })
    const unpaid = projectOpenAgentsSiteCheckoutReturn({
      ...baseInput,
      observedReturnPath: '/checkout/status',
      returnAction: 'status',
    })

    expect(cancel).toMatchObject({
      cleanReturnPath: '/pricing',
      returnState: 'cancel',
    })
    expect(expired.returnState).toBe('expired')
    expect(pending.returnState).toBe('pending')
    expect(unpaid.returnState).toBe('unpaid')
  })

  test('blocks mismatched refs and rejects checkout query state or payment material', () => {
    const blocked = projectOpenAgentsSiteCheckoutReturn({
      ...baseInput,
      hostedCheckout: {
        ...hostedCheckout,
        challengeRef: 'challenge.site_checkout.other',
      },
    })

    expect(blocked.returnState).toBe('blocked')
    expect(blocked.buyerPaymentChallenge).toBe(null)
    expect(() =>
      projectOpenAgentsSiteCheckoutReturn({
        ...baseInput,
        observedReturnPath: '/checkout/thanks?checkout_id=abc',
      }),
    ).toThrow(OpenAgentsSiteCheckoutReturnUnsafe)
    expect(() =>
      projectOpenAgentsSiteCheckoutReturn({
        ...baseInput,
        hostedCheckout: {
          ...hostedCheckout,
          invoiceRef: 'lnbc2500n1rawinvoice',
        },
      }),
    ).toThrow(OpenAgentsSiteCheckoutReturnUnsafe)
  })
})
