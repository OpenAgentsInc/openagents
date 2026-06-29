import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsGeneratedSitePaymentSmokeFixture,
  OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe,
  assertOpenAgentsGeneratedSitePaymentSmokeFixtureSafe,
  exampleOpenAgentsGeneratedSitePaymentCatalog,
  exampleOpenAgentsGeneratedSitePaymentHelperPlans,
  exampleOpenAgentsGeneratedSitePaymentManifest,
  exampleOpenAgentsGeneratedSitePaymentSmokeFixture,
  openAgentsGeneratedSitePaymentSmokeFixtureHasPrivateMaterial,
  projectOpenAgentsGeneratedSitePaymentLaunchGate,
} from './generated-site-payment-smoke-fixture'
import { OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES } from './redaction-regression-fixtures'
import { OpenAgentsGeneratedSitePaymentHelperRequestPlan } from './site-mdk-generated-helpers'
import { OpenAgentsSiteMdkSmokeProjection } from './site-mdk-smoke'
import {
  OpenAgentsSitePaymentCatalogProjection,
  sitePaymentCatalogFromManifest,
} from './site-payment-catalog'
import {
  OpenAgentsSitePaymentDiscoveryProjection,
  projectOpenAgentsSitePaymentDiscovery,
} from './site-payment-discovery'
import {
  OpenAgentsSitePaymentManifestProjection,
  projectOpenAgentsSitePaymentManifest,
} from './site-payment-manifest'

describe('generated Site payment smoke fixture', () => {
  test('builds one deterministic generated Site fixture with checkout and paid action paths', () => {
    const fixture = exampleOpenAgentsGeneratedSitePaymentSmokeFixture()

    expect(
      S.decodeUnknownSync(OpenAgentsGeneratedSitePaymentSmokeFixture)(fixture),
    ).toEqual(fixture)
    expect(
      S.decodeUnknownSync(OpenAgentsSitePaymentManifestProjection)(
        fixture.manifestProjection,
      ),
    ).toEqual(fixture.manifestProjection)
    expect(
      S.decodeUnknownSync(OpenAgentsSitePaymentCatalogProjection)(
        fixture.catalogProjection,
      ),
    ).toEqual(fixture.catalogProjection)
    expect(
      S.decodeUnknownSync(OpenAgentsSitePaymentDiscoveryProjection)(
        fixture.discoveryProjection,
      ),
    ).toEqual(fixture.discoveryProjection)
    expect(
      S.decodeUnknownSync(OpenAgentsSiteMdkSmokeProjection)(
        fixture.smokeProjection,
      ),
    ).toEqual(fixture.smokeProjection)

    expect(fixture.manifestProjection.products).toHaveLength(1)
    expect(fixture.manifestProjection.paidActions).toHaveLength(1)
    expect(fixture.catalogProjection.items).toHaveLength(2)
    expect(fixture.discoveryProjection.items).toHaveLength(2)
    expect(fixture.discoveryProjection.agentReadable).toBe(true)
    expect(fixture.smokeProjection).toMatchObject({
      acceptedWorkPayoutClaimAllowed: false,
      notProductionPaymentEvidence: true,
      providerPayoutClaimAllowed: false,
      settlementClaimAllowed: false,
      smokeState: 'passed',
      walletSpendAllowed: false,
    })
    expect(fixture.paymentLaunchGate).toMatchObject({
      checkoutEvidenceOnly: true,
      liveBitcoinCheckoutClaimAllowed: false,
      payoutSettlementClaimAllowed: false,
      state: 'checkout_evidence_only',
    })
    expect(fixture.paymentLaunchGate.receiptBundleRefs).toEqual(
      expect.arrayContaining([
        'receipt.public.generated_site_payment_smoke.record_only',
        'entitlement.public.generated_site_payment_smoke.active',
        'reconciliation.public.generated_site_payment_smoke.matched',
        'payment_proof.public.generated_site_payment_smoke',
      ]),
    )
    expect(fixture.paymentLaunchGate.publicCopyRefs).toEqual([
      'copy.generated_site_payment.checkout_evidence_only',
    ])
    expect(fixture).toMatchObject({
      noDeploymentAuthority: true,
      noLiveCheckoutCreated: true,
      noRealInvoiceCreated: true,
      noWalletSpendAuthority: true,
    })
    expect(JSON.stringify(fixture)).not.toMatch(
      /(MDK_ACCESS_TOKEN|MDK_MNEMONIC|lnbc|lntb|lno1|payment_hash|payment_preimage|preimage=|checkout_id=|provider_grant|wallet_secret|raw_customer_email)/i,
    )
  })

  test('is consumable by existing manifest, catalog, discovery, and helper contracts', () => {
    const manifest = exampleOpenAgentsGeneratedSitePaymentManifest()
    const catalog = exampleOpenAgentsGeneratedSitePaymentCatalog()
    const derivedCatalog = sitePaymentCatalogFromManifest({
      createdAt: '2026-06-07T12:00:00.000Z',
      deploymentId: null,
      manifest,
      manifestRef: 'manifest.generated_site_payment_smoke.v1',
      orderRef: 'order.generated_site_payment_smoke',
      siteId: 'site_payment_smoke',
      siteVersionId: 'version_site_payment_smoke_v1',
      sourceManifestDigest: 'digest.generated_site_payment_smoke.v1',
      status: 'active',
      updatedAt: '2026-06-07T12:00:00.000Z',
      workroomRef: 'workroom.generated_site_payment_smoke',
    })
    const discovery = projectOpenAgentsSitePaymentDiscovery({
      audience: 'agent',
      catalog,
      siteId: 'site_payment_smoke',
    })

    expect(
      projectOpenAgentsSitePaymentManifest(manifest, 'agent').products,
    ).toHaveLength(1)
    expect(derivedCatalog).toEqual(catalog)
    expect(discovery.items.map(item => item.itemKind).sort()).toEqual([
      'paid_action',
      'product',
    ])

    for (const plan of exampleOpenAgentsGeneratedSitePaymentHelperPlans()) {
      expect(
        S.decodeUnknownSync(OpenAgentsGeneratedSitePaymentHelperRequestPlan)(
          plan,
        ),
      ).toEqual(plan)
      expect(plan.url).not.toContain('?')
      expect(plan.url).not.toContain('#')
    }
  })

  test('separates live checkout evidence from payout settlement receipts', () => {
    const liveCheckout = projectOpenAgentsGeneratedSitePaymentLaunchGate({
      activeEntitlementRefs: ['entitlement.public.generated_site.live.active'],
      checkoutIntentRefs: ['site_checkout_intent_generated_site.live'],
      implementationState: 'live_provider',
      paymentProofRefs: ['payment_proof.public.generated_site.live'],
      receiptRefs: ['receipt.public.generated_site.live'],
      reconciliationEventRefs: [
        'reconciliation.public.generated_site.live.matched',
      ],
      settlementReceiptRefs: [],
    })
    const settled = projectOpenAgentsGeneratedSitePaymentLaunchGate({
      activeEntitlementRefs: ['entitlement.public.generated_site.live.active'],
      checkoutIntentRefs: ['site_checkout_intent_generated_site.live'],
      implementationState: 'live_provider',
      paymentProofRefs: ['payment_proof.public.generated_site.live'],
      receiptRefs: ['receipt.public.generated_site.live'],
      reconciliationEventRefs: [
        'reconciliation.public.generated_site.live.matched',
      ],
      settlementReceiptRefs: [
        'receipt.nexus_pylon.settlement.generated_site.live',
      ],
    })

    expect(liveCheckout).toMatchObject({
      checkoutEvidenceOnly: true,
      liveBitcoinCheckoutClaimAllowed: true,
      payoutSettlementClaimAllowed: false,
      state: 'live_bitcoin_checkout_verified',
    })
    expect(liveCheckout.publicCopyRefs).toEqual([
      'copy.generated_site_payment.live_checkout_evidence_only',
    ])
    expect(settled).toMatchObject({
      checkoutEvidenceOnly: false,
      liveBitcoinCheckoutClaimAllowed: true,
      payoutSettlementClaimAllowed: true,
      state: 'payout_settlement_verified',
    })
    expect(settled.publicCopyRefs).toEqual([
      'copy.generated_site_payment.payout_settlement_receipts_visible',
    ])
  })

  test('rejects unsafe payment, wallet, provider, customer, and secret fixture values', () => {
    const fixture = exampleOpenAgentsGeneratedSitePaymentSmokeFixture()

    for (const unsafe of OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES) {
      expect(
        openAgentsGeneratedSitePaymentSmokeFixtureHasPrivateMaterial(
          unsafe.value,
        ),
        unsafe.label,
      ).toBe(true)
      expect(() =>
        assertOpenAgentsGeneratedSitePaymentSmokeFixtureSafe({
          ...fixture,
          sourceRefs: [unsafe.value],
        }),
      ).toThrow(OpenAgentsGeneratedSitePaymentSmokeFixtureUnsafe)
    }
  })
})
