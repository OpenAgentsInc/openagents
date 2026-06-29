import { describe, expect, test } from 'vitest'

import {
  SiteCommerceRevenueLinkageError,
  type SiteCommerceRevenueLinkageInput,
  deriveSiteCommerceRevenueProjection,
} from './site-commerce-revenue-share'

const baseLinkage = {
  amount: 100,
  asset: 'sats',
  customerRef: 'customer_ben',
  entitlementRef: 'site_entitlement_123',
  eventKind: 'l402_redeemed',
  paidActionId: 'generate-report',
  paymentEvidenceRef: 'mdk_payment_proof_123',
  providerPayoutClaimed: false,
  publicReceiptRef: 'site_payment_receipt_123',
  receiptRefs: {},
  referralSourceRef: 'referral_source_chris',
  requestedContributorAsset: 'sats',
  siteId: 'site_otec',
  siteVersionId: 'site_version_2',
  softwareOrderId: 'software_order_ben_otec',
} satisfies SiteCommerceRevenueLinkageInput

describe('Site commerce revenue-share linkage', () => {
  test('links paid Site activity to referral attribution without provider payout eligibility', () => {
    const projection = deriveSiteCommerceRevenueProjection(baseLinkage)

    expect(projection).toEqual({
      acceptedWork: {
        ref: null,
        status: 'absent',
      },
      entitlement: {
        ref: 'site_entitlement_123',
        status: 'present',
      },
      paymentEvidence: {
        asset: 'sats',
        ref: 'mdk_payment_proof_123',
        status: 'present',
      },
      providerPayoutEligibility: {
        reason:
          'No provider payout claim is backed by the current linkage event.',
        status: 'not_eligible',
      },
      publicReceiptRef: 'site_payment_receipt_123',
      referralAttribution: {
        ref: 'referral_source_chris',
        rewardTrigger: 'paid_activity',
      },
      settlement: {
        asset: 'sats',
        withdrawalPosture: 'bitcoin_withdrawable_after_settlement',
      },
    })
  })

  test('does not let raw signup attribution create payout eligibility', () => {
    const {
      entitlementRef: _entitlementRef,
      paymentEvidenceRef: _paymentEvidenceRef,
      ...signupLinkage
    } = baseLinkage

    expect(() =>
      deriveSiteCommerceRevenueProjection({
        ...signupLinkage,
        amount: 1,
        eventKind: 'signup_attributed',
        providerPayoutClaimed: true,
      }),
    ).toThrow(SiteCommerceRevenueLinkageError)
  })

  test('keeps credit spend from becoming immediate Bitcoin withdrawal liability', () => {
    expect(() =>
      deriveSiteCommerceRevenueProjection({
        ...baseLinkage,
        asset: 'credits',
        eventKind: 'credit_spent',
        requestedContributorAsset: 'sats',
      }),
    ).toThrow(SiteCommerceRevenueLinkageError)
  })

  test('projects credit-sourced rewards as internal credit only', () => {
    const projection = deriveSiteCommerceRevenueProjection({
      ...baseLinkage,
      asset: 'credits',
      eventKind: 'credit_spent',
      requestedContributorAsset: 'credits',
    })

    expect(projection.settlement).toEqual({
      asset: 'credits',
      withdrawalPosture: 'internal_credit_only',
    })
  })

  test('requires Nexus, Treasury, and LDK refs for Pylon accepted-work payout claims', () => {
    expect(() =>
      deriveSiteCommerceRevenueProjection({
        ...baseLinkage,
        acceptedWorkRef: 'accepted_work_123',
        eventKind: 'accepted_work_closed',
        providerPayoutClaimed: true,
        receiptRefs: {
          nexusReceiptRef: 'nexus_receipt_123',
          treasuryReceiptRef: 'treasury_receipt_123',
        },
      }),
    ).toThrow(SiteCommerceRevenueLinkageError)
  })

  test('allows Pylon accepted-work payout eligibility only with required receipt refs', () => {
    const projection = deriveSiteCommerceRevenueProjection({
      ...baseLinkage,
      acceptedWorkRef: 'accepted_work_123',
      eventKind: 'accepted_work_closed',
      providerPayoutClaimed: true,
      receiptRefs: {
        ldkSettlementReceiptRef: 'ldk_settlement_receipt_123',
        nexusReceiptRef: 'nexus_receipt_123',
        treasuryReceiptRef: 'treasury_receipt_123',
      },
    })

    expect(projection.providerPayoutEligibility).toEqual({
      reason: 'Accepted-work and Nexus/Treasury/LDK receipt refs are present.',
      status: 'eligible_pending_settlement_refs',
    })
  })

  test('rejects unsafe raw payment material in public receipt refs', () => {
    expect(() =>
      deriveSiteCommerceRevenueProjection({
        ...baseLinkage,
        paymentEvidenceRef: 'lnbc1000n1rawinvoice',
      }),
    ).toThrow(SiteCommerceRevenueLinkageError)
  })
})
