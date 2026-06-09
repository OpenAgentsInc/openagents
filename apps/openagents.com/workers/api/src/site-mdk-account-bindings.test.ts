import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsSiteMdkAccountBindingProjection,
  OpenAgentsSiteMdkAccountBindingUnsafe,
  assertOpenAgentsSiteMdkAccountBindingSafe,
  currentMdkAccountBindingForCatalogItem,
  projectOpenAgentsSiteMdkAccountBinding,
} from './site-mdk-account-bindings'
import { sitePaymentCatalogFromManifest } from './site-payment-catalog'

const catalog = sitePaymentCatalogFromManifest({
  createdAt: '2026-06-07T12:00:00.000Z',
  deploymentId: 'deployment.site_otec.v5',
  manifest: {
    payments: {
      agentReadable: true,
      enabled: true,
      metadataRefs: ['metadata.site_payment.site_otec.v5'],
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
  updatedAt: '2026-06-07T12:01:00.000Z',
  workroomRef: 'workroom.site_otec',
} as const)

const approvedBinding = {
  allowedActionRefs: [],
  allowedCatalogRefs: [catalog.items[0]!.catalogRef],
  allowedProductRefs: ['consultation_deposit'],
  archivedAt: null,
  bindingRef: 'site_mdk_account:site_otec:customer_wallet',
  caveatRefs: ['caveat.site_mdk_account.binding_reviewed'],
  createdAt: '2026-06-07T12:05:00.000Z',
  customerRef: 'customer.site_otec',
  environment: 'sandbox',
  id: 'site_mdk_account_binding_site_otec_customer_wallet',
  idempotencyKeyHash: 'hash.site_mdk_account.site_otec.customer_wallet',
  orderRef: 'order.site_otec',
  publicProjectionJson: '{}',
  requestedProviderMode: 'customer_owned_mdk',
  reviewStatus: 'approved',
  reviewerRefs: ['operator.site_mdk_account'],
  secretBindingRefs: ['hosted_secret.site_mdk_account.site_otec.mdk'],
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v5',
  updatedAt: '2026-06-07T12:05:00.000Z',
} as const

describe('OpenAgents Site MDK account bindings', () => {
  test('projects customer-owned account state without exposing secret refs publicly', () => {
    const customerProjection = projectOpenAgentsSiteMdkAccountBinding({
      audience: 'customer',
      binding: approvedBinding,
      siteId: 'site_otec',
    })
    const operatorProjection = projectOpenAgentsSiteMdkAccountBinding({
      audience: 'operator',
      binding: approvedBinding,
      siteId: 'site_otec',
    })

    expect(S.decodeUnknownSync(OpenAgentsSiteMdkAccountBindingProjection)(
      customerProjection,
    )).toEqual(customerProjection)
    expect(customerProjection).toMatchObject({
      bindingState: 'configured',
      checkoutAuthorityCreated: false,
      providerMode: 'customer_owned_mdk',
      secretBindingRefs: [],
      secretBindingState: 'redacted',
      walletMaterialExposed: false,
    })
    expect(operatorProjection.secretBindingRefs).toEqual([
      'hosted_secret.site_mdk_account.site_otec.mdk',
    ])
    expect(JSON.stringify(customerProjection)).not.toMatch(
      /(2026-\d{2}-\d{2}T|MDK_ACCESS_TOKEN|mnemonic|lnbc|wallet_secret|customer\\.site_otec)/i,
    )
  })

  test('reports unavailable, pending, blocked, and revoked customer-safe states', () => {
    expect(projectOpenAgentsSiteMdkAccountBinding({
      audience: 'customer',
      binding: null,
      siteId: 'site_otec',
    }).bindingState).toBe('unavailable')
    expect(projectOpenAgentsSiteMdkAccountBinding({
      audience: 'customer',
      binding: { ...approvedBinding, reviewStatus: 'pending_review' },
      siteId: 'site_otec',
    }).bindingState).toBe('pending_review')
    expect(projectOpenAgentsSiteMdkAccountBinding({
      audience: 'customer',
      binding: { ...approvedBinding, reviewStatus: 'blocked' },
      siteId: 'site_otec',
    }).bindingState).toBe('blocked')
    expect(projectOpenAgentsSiteMdkAccountBinding({
      audience: 'customer',
      binding: { ...approvedBinding, reviewStatus: 'revoked' },
      siteId: 'site_otec',
    }).bindingState).toBe('revoked')
  })

  test('matches only approved bindings to catalog items', () => {
    expect(currentMdkAccountBindingForCatalogItem(
      [approvedBinding],
      catalog.items[0]!,
    )?.bindingRef).toBe('site_mdk_account:site_otec:customer_wallet')
    expect(currentMdkAccountBindingForCatalogItem(
      [{ ...approvedBinding, reviewStatus: 'pending_review' }],
      catalog.items[0]!,
    )).toBeNull()
  })

  test('rejects secret-shaped account binding input', () => {
    expect(() =>
      assertOpenAgentsSiteMdkAccountBindingSafe({
        ...approvedBinding,
        secretBindingRefs: ['MDK_ACCESS_TOKEN=re_secret'],
      }),
    ).toThrow(OpenAgentsSiteMdkAccountBindingUnsafe)
  })
})
