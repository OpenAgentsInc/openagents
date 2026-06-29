import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsSitePaymentDiscoveryProjection,
  OpenAgentsSitePaymentDiscoveryUnsafe,
  openAgentsSitePaymentDiscoveryHasPrivateMaterial,
  projectOpenAgentsSitePaymentDiscovery,
} from './site-payment-discovery'
import { sitePaymentCatalogFromManifest } from './site-payment-catalog'

const manifest = {
  payments: {
    agentReadable: true,
    enabled: true,
    metadataRefs: ['metadata.site_payment.otec.v4'],
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
    products: [
      {
        agentReadable: true,
        checkoutPath: '/checkout/consultation-deposit',
        customerDataRequirements: [
          {
            key: 'email',
            kind: 'email',
            labelRef: 'label.customer.email',
            required: true,
          },
        ],
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
} as const

const catalog = sitePaymentCatalogFromManifest({
  createdAt: '2026-06-06T12:00:00.000Z',
  deploymentId: 'deployment.otec.v4',
  manifest,
  manifestRef: 'manifest.otec.payments.v4',
  orderRef: 'order.otec',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v4',
  sourceManifestDigest: 'sha256:site_otec_manifest_v4',
  status: 'active',
  updatedAt: '2026-06-06T12:01:00.000Z',
  workroomRef: 'workroom.otec',
} as const)

describe('OpenAgents Site payment discovery', () => {
  test('projects agent-readable products, paid actions, endpoints, and states', () => {
    const discovery = projectOpenAgentsSitePaymentDiscovery({
      audience: 'agent',
      catalog,
      siteId: 'site_otec',
    })

    expect(S.decodeUnknownSync(OpenAgentsSitePaymentDiscoveryProjection)(
      discovery,
    )).toEqual(discovery)
    expect(discovery).toMatchObject({
      endpoints: {
        checkoutIntent: '/api/sites/site_otec/commerce/checkout-intents',
        checkoutReturn:
          '/api/sites/site_otec/commerce/checkout-returns/{checkoutIntentRef}/{returnAction}',
        commerceReview: '/api/sites/site_otec/commerce/review',
        commerceReviewDecision:
          '/api/sites/site_otec/commerce/review-decisions',
        l402Challenge: '/api/sites/site_otec/commerce/l402/challenges',
        l402Redemption: '/api/sites/site_otec/commerce/l402/redemptions',
        payoutBridge: '/api/sites/site_otec/commerce/payout-bridges',
        paymentProof:
          '/api/sites/site_otec/commerce/payment-proofs/{checkoutIntentRef}',
        providerEventReconcile:
          '/api/sites/site_otec/commerce/mdk/webhooks',
      },
      provider: 'openagents_hosted_mdk',
      siteId: 'site_otec',
      surfaceStates: {
        checkoutIntent: 'gated',
        checkoutReturn: 'available',
        commerceReview: 'available',
        commerceReviewDecision: 'gated',
        entitlementProjection: 'available',
        l402Challenge: 'available_contract',
        l402Redemption: 'available_contract',
        payoutBridge: 'gated',
        paymentProof: 'available',
        providerEventReconciliation: 'gated',
        settlement: 'gated',
        wfpMiddleware: 'available_contract',
      },
    })
    expect(discovery.items).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          catalogRef:
            'site_payment:site_otec:version_site_otec_v4:product:consultation_deposit',
          checkoutIntentEndpoint:
            '/api/sites/site_otec/commerce/checkout-intents',
          customerDataRequirementRefs: ['email'],
          itemKind: 'product',
          l402ChallengeEndpoint: null,
          productId: 'consultation_deposit',
          spendCapHintRefs: ['spend_cap.usd.product'],
        }),
        expect.objectContaining({
          actionId: 'download_report',
          itemKind: 'paid_action',
          l402ChallengeEndpoint:
            '/api/sites/site_otec/commerce/l402/challenges',
          l402HeaderRef: 'WWW-Authenticate: L402',
          method: 'GET',
          path: '/api/actions/download-report',
          spendCapHintRefs: ['spend_cap.bitcoin.action'],
        }),
      ]),
    )
    expect(openAgentsSitePaymentDiscoveryHasPrivateMaterial(discovery))
      .toBe(false)
  })

  test('rejects raw payment material and customer private data', () => {
    expect(() =>
      projectOpenAgentsSitePaymentDiscovery({
        audience: 'agent',
        catalog: {
          items: [
            {
              ...catalog.items[0]!,
              checkoutPath: '/checkout/consultation?checkout_id=raw',
            },
          ],
        },
        siteId: 'site_otec',
      }),
    ).toThrow(OpenAgentsSitePaymentDiscoveryUnsafe)
    expect(() =>
      projectOpenAgentsSitePaymentDiscovery({
        audience: 'agent',
        catalog: {
          items: [
            {
              ...catalog.items[1]!,
              metadataRefs: ['ben@example.com'],
            },
          ],
        },
        siteId: 'site_otec',
      }),
    ).toThrow(OpenAgentsSitePaymentDiscoveryUnsafe)
  })
})
