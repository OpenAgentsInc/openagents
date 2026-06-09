import { describe, expect, test } from 'vitest'

import { createPendingAgentClaimRewardRecord } from './agent-claim-reward-ledger'
import {
  AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS,
  evaluateAgentClaimRewardPolicy,
  projectAgentClaimRewardPublicCounters,
} from './agent-claim-reward-policy'

const eligiblePolicyInput = {
  budgetAvailable: true,
  campaignCapRemaining: true,
  campaignPaused: false,
  coordinatedRewardFarmingSignal: false,
  dailyCapRemaining: true,
  destinationReuseCount: 1,
  destinationReuseLimit: 3,
  hostedMdkReady: true,
  legalReviewed: true,
  manualReviewRequired: false,
  ownerAccountEligible: true,
  regionAllowed: true,
  repeatedDestinationCluster: false,
  sanctionsScreenPassed: true,
  suspiciousDeviceCluster: false,
  tweetVisibleAndStable: true,
  xAccountEligible: true,
  xAccountMinimumAgePassed: true,
}

describe('agent claim reward policy', () => {
  test('allows live dispatch only after all policy gates pass', () => {
    const decision = evaluateAgentClaimRewardPolicy(eligiblePolicyInput)

    expect(decision).toMatchObject({
      canCreateEligibility: true,
      canDispatchLivePayout: true,
      state: 'eligible',
    })
    expect(decision.policyRefs).toEqual(AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS)
  })

  test('blocks live dispatch when legal, budget, region, or payout gates are missing', () => {
    const decision = evaluateAgentClaimRewardPolicy({
      ...eligiblePolicyInput,
      budgetAvailable: false,
      hostedMdkReady: false,
      legalReviewed: false,
      regionAllowed: false,
    })

    expect(decision.state).toBe('blocked')
    expect(decision.canCreateEligibility).toBe(false)
    expect(decision.canDispatchLivePayout).toBe(false)
    expect(decision.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.agent_claim_reward.legal_review_required',
        'blocker.agent_claim_reward.budget_unavailable',
        'blocker.agent_claim_reward.hosted_mdk_not_ready',
        'blocker.agent_claim_reward.sanctions_or_region_gate_failed',
      ]),
    )
  })

  test('routes reward farming signals into manual review without public fraud details', () => {
    const decision = evaluateAgentClaimRewardPolicy({
      ...eligiblePolicyInput,
      coordinatedRewardFarmingSignal: true,
      repeatedDestinationCluster: true,
      suspiciousDeviceCluster: true,
    })
    const serialized = JSON.stringify(decision)

    expect(decision.state).toBe('manual_review')
    expect(decision.canCreateEligibility).toBe(true)
    expect(decision.canDispatchLivePayout).toBe(false)
    expect(decision.manualReviewReasonRefs).toEqual(
      expect.arrayContaining([
        'review.agent_claim_reward.suspicious_device_cluster',
        'review.agent_claim_reward.repeated_destination_cluster',
        'review.agent_claim_reward.coordinated_reward_farming',
      ]),
    )
    expect(serialized).not.toContain('fingerprint')
    expect(serialized).not.toContain('ipAddress')
    expect(serialized).not.toContain('deviceId')
  })

  test('supports operator pause and public-safe rejection reasons', () => {
    const paused = evaluateAgentClaimRewardPolicy({
      ...eligiblePolicyInput,
      campaignPaused: true,
    })
    const rejected = evaluateAgentClaimRewardPolicy({
      ...eligiblePolicyInput,
      operatorRejectionReason:
        'Duplicate account cluster; raw fingerprint private-123 should not leak.',
    })

    expect(paused.state).toBe('blocked')
    expect(paused.blockerRefs).toContain(
      'blocker.agent_claim_reward.operator_pause_active',
    )
    expect(rejected.state).toBe('rejected')
    expect(rejected.canDispatchLivePayout).toBe(false)
    expect(rejected.rejectionReason).toContain('Duplicate account cluster')
    expect(rejected.rejectionReason).not.toContain('\u0000')
  })

  test('projects public counters without private review signals', () => {
    const base = createPendingAgentClaimRewardRecord({
      agentClaimRef: 'agent_claim_verified_one',
      id: 'claim_reward_receipt_one',
      idempotencyKey: 'claim-reward-one',
      now: '2026-06-09T00:00:00.000Z',
      ownerRef: 'owner:github-owner-one',
      tweetRef: 'x_tweet:100',
      xAccountRef: 'x:ownerone',
    })
    const counters = projectAgentClaimRewardPublicCounters([
      { ...base, state: 'approved' },
      { ...base, state: 'rejected' },
      { ...base, state: 'reversed' },
      { ...base, state: 'settled' },
    ])

    expect(counters).toEqual({
      approved: 1,
      dispatched: 0,
      expired: 0,
      rejected: 1,
      reversed: 1,
      settled: 1,
      totalPublicReceipts: 4,
    })
  })
})
