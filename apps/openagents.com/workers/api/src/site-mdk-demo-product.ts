import {
  type OpenAgentsSitePaymentCatalog,
  decodeOpenAgentsSitePaymentCatalog,
} from './site-payment-catalog'

export const OMEGA_MDK_DEMO_SITE_ID = 'site_omega_mdk_demo'
export const OMEGA_MDK_DEMO_SITE_VERSION_ID = 'version_omega_mdk_demo_v1'
export const OMEGA_MDK_DEMO_PRODUCT_ID = 'omega_demo_checkout'
export const OMEGA_MDK_DEMO_CATALOG_REF =
  'site_payment:site_omega_mdk_demo:version_omega_mdk_demo_v1:product:omega_demo_checkout'

export type OmegaMdkDemoProductMapping = Readonly<{
  checkoutPath: string
  createdAt: string
  displayRef: string
  metadataRefs: ReadonlyArray<string>
  price: Readonly<{
    amountMinorUnits: number
    asset: 'usd'
    denomination: 'usd_cent'
  }>
  providerProductRef: string
  sandbox: boolean
  updatedAt: string
}>

export const omegaMdkDemoProductMapping = {
  checkoutPath: '/checkout/omega-demo',
  createdAt: '2026-06-07T00:00:00.000Z',
  displayRef: 'display.omega_demo_checkout',
  metadataRefs: [
    'metadata.site_payment.omega_mdk_demo.v1',
    'metadata.mdk_account_app.omega',
  ],
  price: {
    amountMinorUnits: 100,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  providerProductRef: 'mdk_amount_checkout.omega_demo_checkout.usd_100',
  sandbox: true,
  updatedAt: '2026-06-07T00:00:00.000Z',
} as const satisfies OmegaMdkDemoProductMapping

export const omegaMdkDemoSitePaymentCatalogFromMapping = (
  mapping: OmegaMdkDemoProductMapping,
): OpenAgentsSitePaymentCatalog =>
  decodeOpenAgentsSitePaymentCatalog({
    items: [
      {
        agentReadable: true,
        archivedAt: null,
        catalogRef: OMEGA_MDK_DEMO_CATALOG_REF,
        checkoutPath: mapping.checkoutPath,
        createdAt: mapping.createdAt,
        customerDataRequirements: [
          {
            key: 'email',
            kind: 'email',
            labelRef: 'label.customer.email',
            required: true,
          },
        ],
        deploymentId: null,
        displayRef: mapping.displayRef,
        entitlementScope: 'product',
        itemKind: 'product',
        manifestRef: 'manifest.omega_mdk_demo.amount_checkout.v1',
        metadataRefs: [
          ...mapping.metadataRefs,
          mapping.providerProductRef,
        ],
        orderRef: null,
        price: mapping.price,
        productId: OMEGA_MDK_DEMO_PRODUCT_ID,
        publicProjectionState: 'listed',
        sandbox: mapping.sandbox,
        settlementMode: 'checkout_only',
        siteId: OMEGA_MDK_DEMO_SITE_ID,
        siteVersionId: OMEGA_MDK_DEMO_SITE_VERSION_ID,
        sourceManifestDigest: 'sha256:omega_mdk_demo_amount_checkout_v1',
        status: 'active',
        updatedAt: mapping.updatedAt,
        workroomRef: null,
      },
    ],
  })

export const omegaMdkDemoSitePaymentCatalog =
  omegaMdkDemoSitePaymentCatalogFromMapping(omegaMdkDemoProductMapping)
