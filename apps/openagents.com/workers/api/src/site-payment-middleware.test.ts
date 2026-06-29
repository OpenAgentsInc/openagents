import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
} from './buyer-payment-ledger'
import {
  OpenAgentsSitePaymentMiddlewareProjection,
  OpenAgentsSitePaymentMiddlewareUnsafe,
  evaluateOpenAgentsSitePaymentMiddleware,
  openAgentsSitePaymentMiddlewareHasPrivateMaterial,
} from './site-payment-middleware'
import { sitePaymentCatalogFromManifest } from './site-payment-catalog'

const catalog = sitePaymentCatalogFromManifest({
  createdAt: '2026-06-06T11:00:00.000Z',
  deploymentId: 'deployment.site_otec.v4',
  manifest: {
    payments: {
      agentReadable: true,
      enabled: true,
      metadataRefs: ['metadata.site_payment.site_otec.v4'],
      paidActions: [
        {
          actionRef: 'action.report.download',
          agentReadable: true,
          checkoutPath: '/checkout/download-report',
          customerDataRequirements: [],
          displayRef: 'display.download_report',
          entitlementScope: 'action',
          id: 'download_report',
          metadataRefs: ['metadata.action.download_report'],
          method: 'GET',
          path: '/api/actions/download-report',
          price: {
            amountMinorUnits: 25_000,
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
          },
          publicProjectionState: 'listed',
          sandbox: true,
          settlementMode: 'deferred',
        },
      ],
      products: [],
      provider: 'openagents_hosted_mdk',
      sandboxDefault: true,
    },
  },
  manifestRef: 'manifest.site_otec.payments.v4',
  orderRef: 'order.site_otec',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v4',
  sourceManifestDigest: 'sha256:site_otec_manifest_v4',
  status: 'active',
  updatedAt: '2026-06-06T11:01:00.000Z',
  workroomRef: 'workroom.site_otec',
} as const)

const catalogItem = catalog.items[0]!

const protectedRoute = {
  actionId: 'download_report',
  catalogRef: catalogItem.catalogRef,
  entitlementScope: 'action',
  metadataRefs: ['metadata.action.download_report'],
  method: 'GET',
  path: '/api/actions/download-report',
  price: {
    amountMinorUnits: 25_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  publicProjectionState: 'listed',
  sandbox: true,
  settlementMode: 'deferred',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v4',
} as const

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'site.site_otec.checkout',
  archivedAt: null,
  challengeRef: 'challenge.site_checkout.site_otec.download_report',
  createdAt: '2026-06-06T11:02:00.000Z',
  expiresAt: '2026-06-06T11:12:00.000Z',
  id: 'buyer_payment_challenge_site_otec_download_report',
  idempotencyKeyHash: 'hash.site_checkout.site_otec.download_report',
  metadataRefs: ['metadata.action.download_report'],
  method: 'GET',
  ownerUserId: null,
  path: '/api/actions/download-report',
  price: protectedRoute.price,
  productId: catalogItem.catalogRef,
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:site_checkout:download_report',
  spendCap: protectedRoute.price,
  status: 'issued',
  surface: 'site_checkout',
}

const entitlement: BuyerPaymentEntitlementRecord = {
  actorRef: challenge.actorRef,
  archivedAt: null,
  challengeRef: challenge.challengeRef,
  consumedAt: null,
  createdAt: '2026-06-06T11:03:00.000Z',
  entitlementRef: 'entitlement.site_payment.site_otec.download_report',
  expiresAt: '2026-06-07T11:03:00.000Z',
  id: 'buyer_payment_entitlement_site_otec_download_report',
  ownerUserId: null,
  productId: catalogItem.catalogRef,
  receiptRef: 'receipt.site_payment.site_otec.download_report',
  scopeRefs: [
    'entitlement.site_payment.site_otec.version_site_otec_v4.action.download_report',
  ],
  status: 'active',
  surface: 'site_checkout',
}

const missingPaymentHeader = {
  bearerAuthPresent: false,
  challengeRef: null,
  credential: null,
  credentialSource: null,
  proofRef: null,
  reasonRef: 'reason.payment_header.missing',
  status: 'missing',
} as const

const presentPaymentHeader = {
  bearerAuthPresent: false,
  challengeRef: null,
  credential: 'oa-l402-v1.redacted_credential',
  credentialSource: 'authorization_l402',
  proofRef: 'proof_ref.redacted.site_payment',
  reasonRef: 'reason.payment_header.l402_authorization',
  status: 'l402_authorization',
} as const

const baseInput = {
  audience: 'agent',
  buyerPaymentChallenge: challenge,
  catalogItem,
  entitlement: null,
  hostedCheckout: {
    acceptedWorkSettlementAuthority: false,
    amount: challenge.price,
    audience: 'public',
    challengeRef: challenge.challengeRef,
    checkoutRef: 'checkout.site_otec.download_report',
    checkoutUrlRef: 'checkout_url.site_otec.download_report',
    environment: 'sandbox',
    invoiceRef: null,
    paymentHashRef: null,
    productId: catalogItem.catalogRef,
    provider: 'mdk_hosted',
    providerPayoutAuthority: false,
    providerRef: 'provider.openagents.hosted_mdk.fake',
    sandbox: true,
    settlementAuthority: 'buyer_payment_evidence_only',
    siteRef: 'site_otec',
    status: 'created',
  },
  paymentHeader: missingPaymentHeader,
  protectedRoute,
} as const

describe('OpenAgents Site payment middleware', () => {
  test('returns payment-required projections with clean L402 headers', () => {
    const projection = evaluateOpenAgentsSitePaymentMiddleware(baseInput)

    expect(S.decodeUnknownSync(OpenAgentsSitePaymentMiddlewareProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      decisionStatus: 'payment_required',
      hostedCheckoutUrlRef: 'checkout_url.site_otec.download_report',
      l402Response: {
        errorKind: 'payment_required',
        statusCode: 402,
      },
      statusCode: 402,
    })
    expect(projection.wwwAuthenticate).toContain('L402')
    expect(projection.wwwAuthenticate).toContain(
      'challenge_ref="challenge.site_checkout.site_otec.download_report"',
    )
    expect(openAgentsSitePaymentMiddlewareHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('allows active matching entitlements', () => {
    const projection = evaluateOpenAgentsSitePaymentMiddleware({
      ...baseInput,
      entitlement,
    })

    expect(projection.decisionStatus).toBe('allow')
    expect(projection.statusCode).toBe(200)
    expect(projection.entitlement?.entitlementRef).toBe(
      'entitlement.site_payment.site_otec.download_report',
    )
    expect(projection.wwwAuthenticate).toBe(null)
  })

  test('requires entitlement when a payment credential is present but not granted', () => {
    const projection = evaluateOpenAgentsSitePaymentMiddleware({
      ...baseInput,
      paymentHeader: presentPaymentHeader,
    })

    expect(projection.decisionStatus).toBe('entitlement_required')
    expect(projection.statusCode).toBe(403)
    expect(projection.l402Response).toBe(null)
    expect(projection.paymentHeader.credentialPresent).toBe(true)
  })

  test('blocks route/catalog mismatches and rejects unsafe material', () => {
    const blocked = evaluateOpenAgentsSitePaymentMiddleware({
      ...baseInput,
      protectedRoute: {
        ...protectedRoute,
        path: '/api/actions/other-report',
      },
    })

    expect(blocked.decisionStatus).toBe('blocked')
    expect(blocked.statusCode).toBe(403)
    expect(() =>
      evaluateOpenAgentsSitePaymentMiddleware({
        ...baseInput,
        protectedRoute: {
          ...protectedRoute,
          path: '/api/actions/download-report?checkout_id=abc',
        },
      }),
    ).toThrow(OpenAgentsSitePaymentMiddlewareUnsafe)
    expect(() =>
      evaluateOpenAgentsSitePaymentMiddleware({
        ...baseInput,
        hostedCheckout: {
          ...baseInput.hostedCheckout,
          invoiceRef: 'lnbc2500n1rawinvoice',
        },
      }),
    ).toThrow(OpenAgentsSitePaymentMiddlewareUnsafe)
  })
})
