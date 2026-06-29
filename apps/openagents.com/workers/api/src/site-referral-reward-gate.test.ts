import { describe, expect, test } from 'vitest'

import {
  SiteReferralRewardGateUnsafe,
  projectSiteReferralRewardGate,
  siteReferralRewardGateHasPrivateMaterial,
} from './site-referral-reward-gate'

describe('Site referral reward gate', () => {
  test('keeps raw attribution separate from reward eligibility', () => {
    const gate = projectSiteReferralRewardGate({
      attributionRefs: ['attribution.public.site_referral.capture_1'],
      paidActivityRefs: [],
      policyBlockerRefs: [],
      settlementReceiptRefs: [],
    })

    expect(gate).toMatchObject({
      attributionCaptured: true,
      bitcoinWithdrawalCopyAllowed: false,
      payoutPending: false,
      rewardEligible: false,
      settled: false,
      state: 'attribution_only',
    })
    expect(gate.publicCopyRefs).toEqual([
      'copy.public.site_referral.bitcoin_stream_claim_blocked',
    ])
  })

  test('requires paid activity before reward eligibility and payout pending state', () => {
    const gate = projectSiteReferralRewardGate({
      attributionRefs: ['attribution.public.site_referral.capture_1'],
      paidActivityRefs: ['workflow.public.site_referral.site_checkout_paid'],
      policyBlockerRefs: [],
      settlementReceiptRefs: [],
    })

    expect(gate).toMatchObject({
      bitcoinWithdrawalCopyAllowed: false,
      paidActivityRefs: ['workflow.public.site_referral.site_checkout_paid'],
      payoutPending: true,
      rewardEligible: true,
      settled: false,
      state: 'payout_pending',
    })
  })

  test('blocks reward and payout on abuse, dispute, cap, refund, and clawback refs', () => {
    for (const blockerRef of [
      'blocker.public.site_referral.self_referral',
      'blocker.public.site_referral.duplicate_account',
      'blocker.public.site_referral.dispute_hold',
      'blocker.public.site_referral.cap_exceeded',
      'blocker.public.site_referral.chargeback_refund',
      'blocker.public.site_referral.clawback',
    ]) {
      const gate = projectSiteReferralRewardGate({
        attributionRefs: ['attribution.public.site_referral.capture_1'],
        paidActivityRefs: ['workflow.public.site_referral.site_checkout_paid'],
        policyBlockerRefs: [blockerRef],
        settlementReceiptRefs: ['settlement.public.site_referral.reward_1'],
      })

      expect(gate).toMatchObject({
        bitcoinWithdrawalCopyAllowed: false,
        blockerRefs: [blockerRef],
        payoutPending: false,
        rewardEligible: false,
        settled: false,
        state: 'blocked_by_policy',
      })
    }
  })

  test('allows Bitcoin copy only when settlement receipts exist without policy blockers', () => {
    const gate = projectSiteReferralRewardGate({
      attributionRefs: ['attribution.public.site_referral.capture_1'],
      paidActivityRefs: ['workflow.public.site_referral.site_checkout_paid'],
      policyBlockerRefs: [],
      settlementReceiptRefs: ['settlement.public.site_referral.reward_1'],
    })

    expect(gate).toMatchObject({
      bitcoinWithdrawalCopyAllowed: true,
      payoutPending: false,
      rewardEligible: true,
      settled: true,
      settlementReceiptRefs: ['settlement.public.site_referral.reward_1'],
      state: 'settled',
    })
    expect(gate.publicCopyRefs).toEqual([
      'copy.public.site_referral.bitcoin_settlement_receipts_visible',
    ])
  })

  test('documents credit rewards as non-Bitcoin liability until settlement', () => {
    const gate = projectSiteReferralRewardGate({
      attributionRefs: ['attribution.public.site_referral.capture_1'],
      paidActivityRefs: [
        'workflow.public.site_referral.credit_reward_recorded',
      ],
      policyBlockerRefs: [],
      settlementReceiptRefs: [],
    })

    expect(gate.caveatRefs).toContain(
      'caveat.public.site_referral.credits_do_not_create_bitcoin_liability',
    )
    expect(gate.bitcoinWithdrawalCopyAllowed).toBe(false)
  })

  test('rejects raw signup, customer, payment, wallet, payout, provider, and timestamp material', () => {
    for (const input of [
      {
        attributionRefs: ['customer_email.private@example.com'],
        paidActivityRefs: [],
        policyBlockerRefs: [],
        settlementReceiptRefs: [],
      },
      {
        attributionRefs: ['attribution.public.site_referral.capture_1'],
        paidActivityRefs: ['lnbc1rawinvoice'],
        policyBlockerRefs: [],
        settlementReceiptRefs: [],
      },
      {
        attributionRefs: ['attribution.public.site_referral.capture_1'],
        paidActivityRefs: ['workflow.public.site_referral.site_checkout_paid'],
        policyBlockerRefs: [],
        settlementReceiptRefs: ['payout_address.bc1qprivate'],
      },
      {
        attributionRefs: ['attribution.public.site_referral.capture_1'],
        paidActivityRefs: ['workflow.public.site_referral.site_checkout_paid'],
        policyBlockerRefs: ['2026-06-08T12:00:00Z'],
        settlementReceiptRefs: [],
      },
    ]) {
      expect(() => projectSiteReferralRewardGate(input)).toThrow(
        SiteReferralRewardGateUnsafe,
      )
    }
  })

  test('keeps public projection free of private material', () => {
    const gate = projectSiteReferralRewardGate({
      attributionRefs: ['attribution.public.site_referral.capture_1'],
      paidActivityRefs: ['workflow.public.site_referral.site_checkout_paid'],
      policyBlockerRefs: [],
      settlementReceiptRefs: ['settlement.public.site_referral.reward_1'],
    })

    expect(siteReferralRewardGateHasPrivateMaterial(gate)).toBe(false)
    expect(JSON.stringify(gate)).not.toContain('lnbc')
    expect(JSON.stringify(gate)).not.toContain('preimage')
    expect(JSON.stringify(gate)).not.toContain('wallet')
    expect(JSON.stringify(gate)).not.toContain('customer_email')
  })
})
