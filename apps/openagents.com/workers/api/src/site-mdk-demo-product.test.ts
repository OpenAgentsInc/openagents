import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeSiteCommerceRoutes } from './site-commerce-routes'
import {
  OpenAgentsSitePaymentCatalogUnsafe,
  openAgentsPaidEndpointProductFromSitePaymentCatalogItem,
  openAgentsSitePaymentCatalogHasPrivateMaterial,
} from './site-payment-catalog'
import {
  openAgentsSitePaymentDiscoveryHasPrivateMaterial,
  projectOpenAgentsSitePaymentDiscovery,
} from './site-payment-discovery'
import {
  OMEGA_MDK_DEMO_CATALOG_REF,
  OMEGA_MDK_DEMO_PRODUCT_ID,
  OMEGA_MDK_DEMO_SITE_ID,
  OMEGA_MDK_DEMO_SITE_VERSION_ID,
  omegaMdkDemoProductMapping,
  omegaMdkDemoSitePaymentCatalog,
  omegaMdkDemoSitePaymentCatalogFromMapping,
} from './site-mdk-demo-product'

const fixedNow = new Date('2026-06-07T00:05:00.000Z')

const routes = makeSiteCommerceRoutes({
  challengeExpiresAt: () => '2026-06-07T00:15:00.000Z',
  checkoutCatalog: omegaMdkDemoSitePaymentCatalog,
  nowEpochMillis: () => fixedNow.getTime(),
  nowIso: () => fixedNow.toISOString(),
})

const makeRequest = (
  path: string,
  input: Readonly<{
    body?: unknown
    idempotencyKey?: string
    method?: string
  }> = {},
) =>
  new Request(`https://openagents.com${path}`, {
    ...(input.body === undefined
      ? {}
      : { body: JSON.stringify(input.body) }),
    headers: {
      ...(input.idempotencyKey === undefined
        ? {}
        : { 'idempotency-key': input.idempotencyKey }),
      ...(input.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
    },
    method: input.method ?? 'POST',
  })

const routeRequest = async (request: Request): Promise<Response> => {
  const routed = routes.routeSiteCommerceRequest(request)

  if (routed === undefined) {
    throw new Error('Expected Site commerce route to match.')
  }

  return Effect.runPromise(routed)
}

describe('Omega MDK demo product mapping', () => {
  test('creates a public-safe Site commerce catalog record for the demo amount checkout', () => {
    expect(omegaMdkDemoSitePaymentCatalog.items).toHaveLength(1)
    expect(omegaMdkDemoSitePaymentCatalog.items[0]).toMatchObject({
      catalogRef: OMEGA_MDK_DEMO_CATALOG_REF,
      checkoutPath: '/checkout/omega-demo',
      displayRef: 'display.omega_demo_checkout',
      itemKind: 'product',
      metadataRefs: [
        'metadata.site_payment.omega_mdk_demo.v1',
        'metadata.mdk_account_app.omega',
        'mdk_amount_checkout.omega_demo_checkout.usd_100',
      ],
      price: {
        amountMinorUnits: 100,
        asset: 'usd',
        denomination: 'usd_cent',
      },
      productId: OMEGA_MDK_DEMO_PRODUCT_ID,
      sandbox: true,
      settlementMode: 'checkout_only',
      siteId: OMEGA_MDK_DEMO_SITE_ID,
      siteVersionId: OMEGA_MDK_DEMO_SITE_VERSION_ID,
      status: 'active',
    })
    expect(
      openAgentsSitePaymentCatalogHasPrivateMaterial(
        omegaMdkDemoSitePaymentCatalog,
      ),
    ).toBe(false)
    expect(
      openAgentsPaidEndpointProductFromSitePaymentCatalogItem(
        omegaMdkDemoSitePaymentCatalog.items[0]!,
      ),
    ).toMatchObject({
      binding: {
        actionRef: OMEGA_MDK_DEMO_PRODUCT_ID,
        kind: 'site_checkout',
        pathTemplate: '/checkout/omega-demo',
      },
      price: {
        amountMinorUnits: 100,
        asset: 'usd',
        denomination: 'usd_cent',
      },
      providerBindingRefs: ['provider_binding.openagents.hosted_mdk'],
      surface: 'site_checkout',
    })
  })

  test('projects demo product discovery without MDK credentials or payment material', () => {
    const discovery = projectOpenAgentsSitePaymentDiscovery({
      audience: 'agent',
      catalog: omegaMdkDemoSitePaymentCatalog,
      siteId: OMEGA_MDK_DEMO_SITE_ID,
    })

    expect(discovery).toMatchObject({
      endpoints: {
        checkoutIntent:
          '/api/sites/site_omega_mdk_demo/commerce/checkout-intents',
      },
      provider: 'openagents_hosted_mdk',
      siteId: OMEGA_MDK_DEMO_SITE_ID,
    })
    expect(discovery.items).toEqual([
      expect.objectContaining({
        catalogRef: OMEGA_MDK_DEMO_CATALOG_REF,
        checkoutIntentEndpoint:
          '/api/sites/site_omega_mdk_demo/commerce/checkout-intents',
        checkoutPath: '/checkout/omega-demo',
        customerDataRequirementRefs: ['email'],
        itemKind: 'product',
        metadataRefs: [
          'metadata.site_payment.omega_mdk_demo.v1',
          'metadata.mdk_account_app.omega',
          'mdk_amount_checkout.omega_demo_checkout.usd_100',
        ],
        productId: OMEGA_MDK_DEMO_PRODUCT_ID,
        spendCapHintRefs: ['spend_cap.usd.product'],
      }),
    ])
    expect(openAgentsSitePaymentDiscoveryHasPrivateMaterial(discovery))
      .toBe(false)
  })

  test('allows the demo product to be selected by checkout intent contracts', async () => {
    const response = await routeRequest(
      makeRequest(
        '/api/sites/site_omega_mdk_demo/commerce/checkout-intents',
        {
          body: {
            cancelReturnPath: '/checkout/omega-demo/cancelled',
            catalogRef: OMEGA_MDK_DEMO_CATALOG_REF,
            customerDataRefs: ['email'],
            expectedPrice: {
              amountMinorUnits: 100,
              asset: 'usd',
              denomination: 'usd_cent',
            },
            itemKind: 'product',
            productId: OMEGA_MDK_DEMO_PRODUCT_ID,
            siteVersionId: OMEGA_MDK_DEMO_SITE_VERSION_ID,
            successReturnPath: '/checkout/omega-demo/complete',
          },
          idempotencyKey: 'omega-demo-checkout-1',
        },
      ),
    )
    const payload = await response.json() as {
      siteCommerce: {
        checkoutIntent: {
          catalogItem: {
            catalogRef: string
            productId: string
          }
          hostedCheckout: {
            invoiceRef: string | null
            paymentHashRef: string | null
          }
          provider: string
          state: string
        }
        implementationState: string
      }
    }

    expect(response.status).toBe(201)
    expect(payload).toMatchObject({
      siteCommerce: {
        checkoutIntent: {
          catalogItem: {
            catalogRef: OMEGA_MDK_DEMO_CATALOG_REF,
            productId: OMEGA_MDK_DEMO_PRODUCT_ID,
          },
          hostedCheckout: {
            invoiceRef: null,
            paymentHashRef: null,
          },
          provider: 'openagents_hosted_mdk',
          state: 'created',
        },
        implementationState: 'fake_provider_contract',
      },
    })
    expect(JSON.stringify(payload)).not.toMatch(
      /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|mdk_access_token|payment_preimage|raw_invoice|wallet_secret)\b/i,
    )
  })

  test('rejects unsafe demo mapping refs, paths, payment material, and customer values', () => {
    expect(() =>
      omegaMdkDemoSitePaymentCatalogFromMapping({
        ...omegaMdkDemoProductMapping,
        providerProductRef: 'mdk_access_token.raw_secret',
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
    expect(() =>
      omegaMdkDemoSitePaymentCatalogFromMapping({
        ...omegaMdkDemoProductMapping,
        checkoutPath: '/checkout/omega-demo?raw_invoice=abc',
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
    expect(() =>
      omegaMdkDemoSitePaymentCatalogFromMapping({
        ...omegaMdkDemoProductMapping,
        displayRef: 'buyer@example.com',
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
    expect(() =>
      omegaMdkDemoSitePaymentCatalogFromMapping({
        ...omegaMdkDemoProductMapping,
        metadataRefs: ['wallet_secret.raw'],
      }),
    ).toThrow(OpenAgentsSitePaymentCatalogUnsafe)
  })
})
