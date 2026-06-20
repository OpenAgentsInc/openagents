import { describe, expect, test } from 'vitest'

import type { XClaimRewardRecord } from './agent-owner-claim-routes'
import { assertXClaimRewardSmokeCandidate } from './x-claim-reward-smoke-candidate'

const eligibleReward = (
  overrides: Partial<XClaimRewardRecord> = {},
): XClaimRewardRecord => ({
  agentUserId: 'user_agent_1',
  amountSats: 1000,
  challengeId: 'x_challenge_1',
  claimId: 'agent_claim_1',
  createdAt: '2026-06-10T10:00:00.000Z',
  evidenceRefs: ['receipt.public.x_claim.1'],
  id: 'x_claim_reward_1',
  ownerUserId: 'user_owner_1',
  receiptRef: 'x_claim_reward_receipt_x_claim_reward_1',
  state: 'eligible',
  stateReasonRef: null,
  treasuryPaymentId: null,
  updatedAt: '2026-06-10T10:00:00.000Z',
  xAccountRef: 'x_account.public.owner_1',
  ...overrides,
})

describe('X claim reward smoke candidate gate', () => {
  test('accepts a clean eligible reward as a smoke candidate', () => {
    const gate = assertXClaimRewardSmokeCandidate(eligibleReward())

    expect(gate.ready).toBe(true)
    expect(gate.blockingReasonRefs).toEqual([])
    expect(gate.checks.every(check => check.ok)).toBe(true)
    expect(gate.candidateSummary).toEqual({
      amountSats: 1000,
      receiptRef: 'x_claim_reward_receipt_x_claim_reward_1',
      rewardId: 'x_claim_reward_1',
      state: 'eligible',
    })
  })

  test('blocks a reward that has already left the eligible state', () => {
    const gate = assertXClaimRewardSmokeCandidate(
      eligibleReward({ state: 'dispatch_requested' }),
    )

    expect(gate.ready).toBe(false)
    expect(gate.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_candidate_not_eligible',
    )
  })

  test('blocks a reward whose amount is not the bounded campaign reward', () => {
    const gate = assertXClaimRewardSmokeCandidate(
      eligibleReward({ amountSats: 5000 }),
    )

    expect(gate.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_candidate_amount_mismatch',
    )
  })

  test('blocks a malformed receipt ref', () => {
    const gate = assertXClaimRewardSmokeCandidate(
      eligibleReward({ receiptRef: 'not_a_receipt' }),
    )

    expect(gate.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_candidate_receipt_ref_malformed',
    )
  })

  test('blocks a reward that already has a treasury payment attached', () => {
    const gate = assertXClaimRewardSmokeCandidate(
      eligibleReward({ treasuryPaymentId: 'payment_secret_1' }),
    )

    expect(gate.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_candidate_payment_already_attached',
    )
  })

  test('blocks leaked payment material in any public field', () => {
    const invoiceLeak = assertXClaimRewardSmokeCandidate(
      eligibleReward({
        evidenceRefs: ['receipt.public.x_claim.1', 'lnbc1000n1psomeinvoice'],
      }),
    )
    const offerLeak = assertXClaimRewardSmokeCandidate(
      eligibleReward({
        evidenceRefs: [
          'receipt.public.x_claim.1',
          'lno1qqqqqqqqqqqqqqqqqqqqqqqqqqqqq',
        ],
      }),
    )
    const addressLeak = assertXClaimRewardSmokeCandidate(
      eligibleReward({ stateReasonRef: 'pay owner@getalby.com' }),
    )
    const preimageLeak = assertXClaimRewardSmokeCandidate(
      eligibleReward({
        evidenceRefs: ['receipt.public.x_claim.1', 'a'.repeat(64)],
      }),
    )

    for (const gate of [invoiceLeak, offerLeak, addressLeak, preimageLeak]) {
      expect(gate.ready).toBe(false)
      expect(gate.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_smoke_candidate_payment_material_leaked',
      )
    }
  })

  test('candidate summary carries no payment id or destination', () => {
    const serialized = JSON.stringify(
      assertXClaimRewardSmokeCandidate(
        eligibleReward({ agentUserId: 'user_agent_secret' }),
      ).candidateSummary,
    )

    expect(serialized).not.toContain('user_agent_secret')
    expect(serialized).not.toContain('lnbc')
  })
})
