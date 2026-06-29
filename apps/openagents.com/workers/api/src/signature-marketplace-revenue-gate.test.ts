import { describe, expect, test } from 'vitest'

import {
  SignatureMarketplaceRevenueGateUnsafe,
  projectSignatureMarketplaceRevenueGate,
  signatureMarketplaceRevenueGateHasPrivateMaterial,
} from './signature-marketplace-revenue-gate'

const payableUsageInput = {
  activationRefs: ['activation.public.signature_market.site_builder_v1'],
  attributionRefs: ['attribution.public.signature_market.site_builder_001'],
  contributorPayableCents: 700,
  disputePolicyRefs: ['policy.public.signature_market.dispute_window_v1'],
  exactUsageSubjectRefs: [
    'usage_subject.public.signature_market.package_site_builder.version_v1',
  ],
  forkPolicyRefs: ['policy.public.signature_market.fork_handling_v1'],
  grossRevenueCents: 1000,
  licensePolicyRefs: ['policy.public.signature_market.license_review_v1'],
  packagePublicationRefs: [
    'publication.public.signature_market.site_builder_v1',
  ],
  packageRefs: ['package.public.signature_market.site_builder'],
  packageValidationRefs: ['validation.public.signature_market.site_builder_v1'],
  payoutEligibilityRefs: [
    'eligibility.public.signature_market.site_builder_contributor_001',
  ],
  pricingPolicyRefs: ['pricing.public.signature_market.usage_cents_v1'],
  programSignatureRefs: ['program_signature.public.site_builder_v1'],
  refundPolicyRefs: ['policy.public.signature_market.refund_handling_v1'],
  revenueProjectionRefs: [
    'revenue.public.signature_market.usage_projection_001',
  ],
  revSharePolicyRefs: ['split.public.signature_market.seventy_thirty_v1'],
  usageEventRefs: [
    'usage.public.signature_market.site_builder_001',
    'usage.public.signature_market.site_builder_001',
  ],
  usageIdempotencyRefs: [
    'idempotency.public.signature_market.site_builder_001',
  ],
}

describe('Signature marketplace revenue gate', () => {
  test('keeps package validation separate from install, promotion, and listing authority', () => {
    const gate = projectSignatureMarketplaceRevenueGate({
      packageRefs: ['package.public.signature_market.site_builder'],
      packageValidationRefs: [
        'validation.public.signature_market.site_builder_v1',
      ],
      programSignatureRefs: ['program_signature.public.site_builder_v1'],
    })

    expect(gate).toMatchObject({
      candidateRuntimeActivationAllowed: false,
      installAllowed: false,
      marketplaceListingMutationAllowed: false,
      payoutClaimAllowed: false,
      revenueProjectionAllowed: false,
      state: 'validated',
    })
    expect(gate.caveatRefs).toContain(
      'caveat.public.signature_market.validation_does_not_install',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.package_publication_missing',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.package_activation_missing',
    )
  })

  test('requires activation, exact usage refs, and idempotency before usage can drive revenue', () => {
    const gate = projectSignatureMarketplaceRevenueGate({
      activationRefs: payableUsageInput.activationRefs,
      packagePublicationRefs: payableUsageInput.packagePublicationRefs,
      packageRefs: ['package.public.signature_market.site_builder'],
      packageValidationRefs: [
        'validation.public.signature_market.site_builder_v1',
      ],
      programSignatureRefs: ['program_signature.public.site_builder_v1'],
      usageEventRefs: [
        'usage.public.signature_market.site_builder_001',
        'usage.public.signature_market.site_builder_001',
      ],
    })

    expect(gate).toMatchObject({
      candidateRuntimeActivationAllowed: true,
      installAllowed: true,
      marketplaceListingMutationAllowed: true,
      meteredUsageEventCount: 1,
      revenueProjectionAllowed: false,
      state: 'validated',
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.exact_usage_subject_missing',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.usage_idempotency_missing',
    )
  })

  test('projects usage revenue without payout claim until settlement receipts exist', () => {
    const gate = projectSignatureMarketplaceRevenueGate(payableUsageInput)

    expect(gate).toMatchObject({
      contributorPayableCents: 700,
      grossRevenueCents: 1000,
      meteredUsageEventCount: 1,
      payoutClaimAllowed: false,
      payoutEligibilityClaimAllowed: true,
      revenueProjectionAllowed: true,
      settledContributorCents: 0,
      settlementClaimAllowed: false,
      state: 'payable',
    })
    expect(gate.publicCopyRefs).toEqual([
      'copy.public.signature_market.revenue_projection_pending_settlement',
    ])
    expect(gate.blockerRefs).toEqual([
      'blocker.public.signature_market.settlement_receipt_missing',
    ])
  })

  test('allows payout and settlement claims only with exact settlement refs', () => {
    const gate = projectSignatureMarketplaceRevenueGate({
      ...payableUsageInput,
      settledContributorCents: 700,
      settlementReceiptRefs: [
        'settlement.public.signature_market.site_builder_001',
      ],
    })

    expect(gate).toMatchObject({
      payoutClaimAllowed: true,
      revenueProjectionAllowed: true,
      settledContributorCents: 700,
      settlementClaimAllowed: true,
      signatureRevenueCopyAllowed: true,
      state: 'settled',
    })
    expect(gate.publicCopyRefs).toEqual([
      'copy.public.signature_market.settlement_receipts_visible',
    ])
  })

  test('requires fork, license, dispute, refund, and rev-share policy states', () => {
    const gate = projectSignatureMarketplaceRevenueGate({
      attributionRefs: payableUsageInput.attributionRefs,
      activationRefs: payableUsageInput.activationRefs,
      exactUsageSubjectRefs: payableUsageInput.exactUsageSubjectRefs,
      grossRevenueCents: 1000,
      packagePublicationRefs: payableUsageInput.packagePublicationRefs,
      packageRefs: payableUsageInput.packageRefs,
      packageValidationRefs: payableUsageInput.packageValidationRefs,
      pricingPolicyRefs: payableUsageInput.pricingPolicyRefs,
      programSignatureRefs: payableUsageInput.programSignatureRefs,
      revenueProjectionRefs: payableUsageInput.revenueProjectionRefs,
      usageEventRefs: payableUsageInput.usageEventRefs,
      usageIdempotencyRefs: payableUsageInput.usageIdempotencyRefs,
    })

    expect(gate).toMatchObject({
      payoutEligibilityClaimAllowed: false,
      revenueProjectionAllowed: true,
      state: 'priced',
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.fork_policy_missing',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.license_policy_missing',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.dispute_policy_missing',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.refund_policy_missing',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.rev_share_policy_missing',
    )
  })

  test('rejects unsafe package, usage, provider, payment, wallet, private repo, and timestamp material', () => {
    const unsafeInputs = [
      { packageValidationRefs: ['raw_package.private_payload'] },
      { packagePublicationRefs: ['raw_package.private_payload'] },
      { activationRefs: ['raw_package.private_payload'] },
      { usageEventRefs: ['usage_event_raw.provider_payload'] },
      { attributionRefs: ['github.com/acme/private-signatures'] },
      { pricingPolicyRefs: ['provider_payload.openai.raw'] },
      { revenueProjectionRefs: ['customer_email.alice@example.com'] },
      { payoutEligibilityRefs: ['wallet.private.signature_market'] },
      { settlementReceiptRefs: ['2026-06-08T12:00:00Z'] },
    ]

    unsafeInputs.forEach(input => {
      expect(() => projectSignatureMarketplaceRevenueGate(input)).toThrow(
        SignatureMarketplaceRevenueGateUnsafe,
      )
    })
  })

  test('keeps settled public projection free of private material', () => {
    const gate = projectSignatureMarketplaceRevenueGate({
      ...payableUsageInput,
      settledContributorCents: 700,
      settlementReceiptRefs: [
        'settlement.public.signature_market.site_builder_001',
      ],
    })
    const json = JSON.stringify(gate)

    expect(signatureMarketplaceRevenueGateHasPrivateMaterial(gate)).toBe(false)
    expect(json).not.toMatch(
      /raw_package|usage_event_raw|provider_payload|customer_email|wallet|preimage|lnbc|@|github\.com\/[^:/]+\/private/i,
    )
  })

  test('rejects impossible revenue share and settlement amounts', () => {
    expect(() =>
      projectSignatureMarketplaceRevenueGate({
        contributorPayableCents: 1200,
        grossRevenueCents: 1000,
      }),
    ).toThrow(SignatureMarketplaceRevenueGateUnsafe)
    expect(() =>
      projectSignatureMarketplaceRevenueGate({
        contributorPayableCents: 700,
        grossRevenueCents: 1000,
        settledContributorCents: 800,
      }),
    ).toThrow(SignatureMarketplaceRevenueGateUnsafe)
  })
})
