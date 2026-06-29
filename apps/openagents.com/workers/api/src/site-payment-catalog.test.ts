import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { decodeOpenAgentsPaidEndpointProductCatalog } from './paid-endpoint-product-catalog'
import {
  OpenAgentsSitePaymentCatalogHostedCheckoutPlan,
  OpenAgentsSitePaymentCatalogProjection,
  OpenAgentsSitePaymentCatalogUnsafe,
  decodeOpenAgentsSitePaymentCatalog,
  openAgentsPaidEndpointProductFromSitePaymentCatalogItem,
  openAgentsSitePaymentCatalogHasPrivateMaterial,
  paidEndpointCatalogFromSitePaymentCatalog,
  projectOpenAgentsSitePaymentCatalog,
  sitePaymentCatalogFromManifest,
} from './site-payment-catalog'

const sitePaymentManifest = {
  payments: {
    agentReadable: true,
    enabled: true,
    metadataRefs: ['metadata.site_payment.otec.v1'],
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

const catalogInput = {
  createdAt: '2026-06-06T08:00:00.000Z',
  deploymentId: 'deployment.otec.v2',
  manifest: sitePaymentManifest,
  manifestRef: 'manifest.otec.payments.v1',
  orderRef: 'order.otec',
  siteId: 'site.otec',
  siteVersionId: 'version.otec.v2',
  sourceManifestDigest: 'sha256:abcdef123456',
  status: 'active',
  updatedAt: '2026-06-06T08:01:00.000Z',
  workroomRef: 'workroom.otec',
} as const

describe('OpenAgents Site payment catalog', () => {
  test('builds versioned product and paid-action records from a manifest', () => {
    const catalog = sitePaymentCatalogFromManifest(catalogInput)

    expect(catalog.items.map(item => item.catalogRef)).toEqual([
      'site_payment:site_otec:version_otec_v2:product:consultation_deposit',
      'site_payment:site_otec:version_otec_v2:paid_action:download_report',
    ])
    expect(catalog.items[0]).toMatchObject({
      checkoutPath: '/checkout/consultation-deposit',
      deploymentId: 'deployment.otec.v2',
      itemKind: 'product',
      orderRef: 'order.otec',
      price: {
        asset: 'usd',
        denomination: 'usd_cent',
      },
      siteId: 'site.otec',
      siteVersionId: 'version.otec.v2',
      workroomRef: 'workroom.otec',
    })
    expect(catalog.items[1]).toMatchObject({
      actionRef: 'action.report.download',
      itemKind: 'paid_action',
      method: 'GET',
      path: '/api/actions/download-report',
    })
  })

  test('projects public, customer, agent, and operator views without private refs', () => {
    const catalog = sitePaymentCatalogFromManifest(catalogInput)
    const publicProjection = projectOpenAgentsSitePaymentCatalog(
      catalog,
      'public',
    )
    const customerProjection = projectOpenAgentsSitePaymentCatalog(
      catalog,
      'customer',
    )
    const agentProjection = projectOpenAgentsSitePaymentCatalog(
      catalog,
      'agent',
    )
    const operatorProjection = projectOpenAgentsSitePaymentCatalog(
      catalog,
      'operator',
    )

    expect(publicProjection.items).toHaveLength(2)
    expect(customerProjection.items).toHaveLength(2)
    expect(agentProjection.items).toHaveLength(2)
    expect(operatorProjection.items).toHaveLength(2)
    expect(publicProjection.items[0]?.metadataRefs).toEqual([])
    expect(publicProjection.items[0]?.operatorRefs).toEqual([])
    expect(agentProjection.items[0]?.metadataRefs).toEqual([
      'metadata.site_payment.otec.v1',
      'metadata.product.consultation_deposit',
    ])
    expect(operatorProjection.items[0]?.operatorRefs).toEqual([
      'deployment.otec.v2',
      'manifest.otec.payments.v1',
      'order.otec',
      'sha256:abcdef123456',
      'workroom.otec',
    ])
    expect(S.decodeUnknownSync(OpenAgentsSitePaymentCatalogProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(JSON.stringify(publicProjection)).not.toContain('order.otec')
    expect(openAgentsSitePaymentCatalogHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('hides public-hidden catalog records from public projection only', () => {
    const catalog = sitePaymentCatalogFromManifest({
      ...catalogInput,
      manifest: {
        payments: {
          ...sitePaymentManifest.payments,
          products: [
            {
              ...sitePaymentManifest.payments.products[0]!,
              publicProjectionState: 'hidden',
            },
          ],
        },
      },
    })
    const publicProjection = projectOpenAgentsSitePaymentCatalog(
      catalog,
      'public',
    )
    const customerProjection = projectOpenAgentsSitePaymentCatalog(
      catalog,
      'customer',
    )

    expect(publicProjection.items.map(item => item.itemKind)).toEqual([
      'paid_action',
    ])
    expect(customerProjection.items.map(item => item.itemKind)).toEqual([
      'product',
      'paid_action',
    ])
  })

  test('integrates with paid endpoint product records and hosted checkout plans', () => {
    const catalog = sitePaymentCatalogFromManifest(catalogInput)
    const productRecord = openAgentsPaidEndpointProductFromSitePaymentCatalogItem(
      catalog.items[0]!,
    )
    const actionRecord = openAgentsPaidEndpointProductFromSitePaymentCatalogItem(
      catalog.items[1]!,
    )
    const endpointCatalog = paidEndpointCatalogFromSitePaymentCatalog(catalog)
    const hostedPlan = {
      catalogRecord: catalog.items[0]!,
      hostedRequest: {
        amount: productRecord.price,
        cancelRef: 'return.cancel',
        challengeExpiresAt: '2026-06-06T08:15:00.000Z',
        challengeRef: 'challenge.site_payment.otec.deposit',
        customerDataRefs: ['customer_data.email.required'],
        environment: 'sandbox',
        idempotencyKeyHash: 'idem.site_payment.otec.deposit',
        l402CredentialRef: null,
        metadataRefs: ['metadata.site_payment.otec.v1'],
        mode: 'product',
        productId: productRecord.productId,
        returnRef: 'return.success',
        sandbox: true,
        siteRef: 'site.otec',
      },
    }

    expect(productRecord).toMatchObject({
      binding: {
        kind: 'site_checkout',
        pathTemplate: '/checkout/consultation-deposit',
      },
      productId:
        'site_payment:site_otec:version_otec_v2:product:consultation_deposit',
      surface: 'site_checkout',
    })
    expect(actionRecord).toMatchObject({
      binding: {
        actionRef: 'action.report.download',
        kind: 'site_paid_action',
        method: 'GET',
        pathTemplate: '/api/actions/download-report',
      },
      surface: 'site_checkout',
    })
    expect(endpointCatalog.products).toHaveLength(2)
    expect(decodeOpenAgentsPaidEndpointProductCatalog(endpointCatalog))
      .toEqual(endpointCatalog)
    expect(S.decodeUnknownSync(OpenAgentsSitePaymentCatalogHostedCheckoutPlan)(
      hostedPlan,
    )).toEqual(hostedPlan)
  })

  test('rejects duplicate refs, unsafe metadata, raw payment material, and unsafe paths', () => {
    const catalog = sitePaymentCatalogFromManifest(catalogInput)

    expect(() =>
      decodeOpenAgentsSitePaymentCatalog({
        items: [
          catalog.items[0],
          {
            ...catalog.items[1]!,
            catalogRef: catalog.items[0]!.catalogRef,
          },
        ],
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentCatalog({
        items: [
          {
            ...catalog.items[0]!,
            metadataRefs: ['ben@example.com'],
          },
        ],
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentCatalog({
        items: [
          {
            ...catalog.items[0]!,
            checkoutPath: '/checkout/deposit?raw_invoice=abc',
          },
        ],
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentCatalog({
        items: [
          {
            ...catalog.items[1]!,
            path: 'https://example.com/api/actions/download-report',
          },
        ],
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
  })
})
