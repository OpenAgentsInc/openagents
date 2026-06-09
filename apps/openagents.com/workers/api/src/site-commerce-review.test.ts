import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsSiteCommerceReviewProjection,
  OpenAgentsSiteCommerceReviewUnsafe,
  assertOpenAgentsSiteCommerceReviewDecisionSafe,
  projectOpenAgentsSiteCommerceReview,
  projectOpenAgentsSiteCommerceReviewDecision,
} from './site-commerce-review'
import { siteCheckoutUiPrimitivesFromCatalog } from './site-checkout-ui-primitives'
import { sitePaymentCatalogFromManifest } from './site-payment-catalog'

const catalog = sitePaymentCatalogFromManifest({
  createdAt: '2026-06-07T10:00:00.000Z',
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
  },
  manifestRef: 'manifest.site_otec.payments.v4',
  orderRef: 'order.site_otec',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v4',
  sourceManifestDigest: 'sha256:site_otec_manifest_v4',
  status: 'active',
  updatedAt: '2026-06-07T10:01:00.000Z',
  workroomRef: 'workroom.site_otec',
} as const)

const uiPrimitives = siteCheckoutUiPrimitivesFromCatalog({
  cancelPath: '/checkout/cancelled',
  catalog,
  runtimeTarget: 'workers_for_platforms',
  sourceSurface: 'generated_react',
  successPath: '/checkout/complete',
})

const acceptedDecision = {
  actorRef: 'operator.site_commerce_review',
  archivedAt: null,
  catalogRef: catalog.items[0]!.catalogRef,
  createdAt: '2026-06-07T10:05:00.000Z',
  customerInputRequirementRefs: [],
  decisionRef: 'site_commerce_review:site_otec:v4:deposit',
  id: 'site_commerce_review_decision_site_otec_deposit',
  idempotencyKeyHash: 'hash.site_commerce_review.site_otec.deposit',
  publicProjectionJson: '{}',
  reasonRefs: ['reason.site_commerce_review.catalog_ok'],
  reviewStatus: 'accepted',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v4',
  updatedAt: '2026-06-07T10:05:00.000Z',
} as const

describe('OpenAgents Site commerce review', () => {
  test('projects checkout products and paid actions for builder review', () => {
    const review = projectOpenAgentsSiteCommerceReview({
      audience: 'agent',
      cancelPath: '/checkout/cancelled',
      catalog,
      decisions: [acceptedDecision],
      successPath: '/checkout/complete',
      uiPrimitives,
    })

    expect(S.decodeUnknownSync(OpenAgentsSiteCommerceReviewProjection)(review))
      .toEqual(review)
    expect(review).toMatchObject({
      decisionAuthority: {
        createsDeploymentAuthority: false,
        createsPaymentAuthority: false,
        createsPayoutAuthority: false,
        createsSettlementClaim: false,
      },
      redaction: {
        exposesCheckoutQueryState: false,
        exposesRawTimestamps: false,
      },
      reviewStatusCounts: {
        accepted: 1,
        needsReview: 1,
      },
      sourceSafety: {
        generatedSourceEmbedsMdkCredentials: false,
        usesHostedOmegaPaymentBoundary: true,
      },
    })
    expect(review.items[0]).toMatchObject({
      checkoutPath: '/checkout/consultation-deposit',
      customerDataRequirementRefs: ['email'],
      priceRef: 'price.usd.usd_cent.2500',
      reviewStatus: 'accepted',
      sourceSafeCheckoutPrimitiveRefs: expect.arrayContaining([
        'site_checkout_ui:site_otec:version_site_otec_v4:product_card:consultation_deposit',
      ]),
    })
    expect(review.items[1]).toMatchObject({
      actionId: 'download_report',
      itemKind: 'paid_action',
      reviewStatus: 'needs_review',
    })
    expect(JSON.stringify(review)).not.toMatch(
      /(2026-\d{2}-\d{2}T|checkout_id=|lnbc|mdk_access_token|mnemonic|wallet_secret)/i,
    )
  })

  test('projects decisions without creating payment, payout, or deploy authority', () => {
    expect(assertOpenAgentsSiteCommerceReviewDecisionSafe(acceptedDecision))
      .toBeUndefined()

    expect(projectOpenAgentsSiteCommerceReviewDecision(acceptedDecision))
      .toMatchObject({
        paymentAuthorityCreated: false,
        payoutAuthorityCreated: false,
        reviewStatus: 'accepted',
      })
  })

  test('rejects unsafe decision refs and private payment material', () => {
    expect(() =>
      assertOpenAgentsSiteCommerceReviewDecisionSafe({
        ...acceptedDecision,
        reasonRefs: ['lnbc2500n1rawinvoice'],
      }),
    ).toThrow(OpenAgentsSiteCommerceReviewUnsafe)

    expect(() =>
      projectOpenAgentsSiteCommerceReview({
        audience: 'agent',
        cancelPath: '/checkout/cancelled?checkout_id=abc',
        catalog,
        decisions: [],
        successPath: '/checkout/complete',
        uiPrimitives,
      }),
    ).toThrow(OpenAgentsSiteCommerceReviewUnsafe)
  })
})
