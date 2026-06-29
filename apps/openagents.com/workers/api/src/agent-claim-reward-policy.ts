import { Schema as S } from 'effect'

import {
  type AgentClaimRewardLedgerRecord,
  type AgentClaimRewardState,
} from './agent-claim-reward-ledger'

export const AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS = [
  'policy.agent_claim_reward.eligibility.x_human_owner.v1',
  'policy.agent_claim_reward.one_x_account_per_campaign.v1',
  'policy.agent_claim_reward.one_owner_per_campaign.v1',
  'policy.agent_claim_reward.destination_reuse_limit.v1',
  'policy.agent_claim_reward.daily_campaign_cap.v1',
  'policy.agent_claim_reward.total_campaign_cap.v1',
  'policy.agent_claim_reward.operator_pause.v1',
  'policy.agent_claim_reward.sanctions_geofence_gate.v1',
  'policy.agent_claim_reward.tax_reporting_threshold_review.v1',
  'policy.agent_claim_reward.money_transmission_review.v1',
  'policy.agent_claim_reward.marketing_classification_review.v1',
  'policy.agent_claim_reward.reversal_and_rejection.v1',
] as const

export const AGENT_CLAIM_REWARD_REQUIRED_CAVEAT_REFS = [
  'caveat.agent_claim_reward.promotional_not_earned_work',
  'caveat.agent_claim_reward.no_guarantee_until_all_gates_pass',
  'caveat.agent_claim_reward.settlement_separate',
  'caveat.agent_claim_reward.forum_access_separate',
] as const

export const AgentClaimRewardPolicyDecisionState = S.Literals([
  'eligible',
  'manual_review',
  'rejected',
  'blocked',
])
export type AgentClaimRewardPolicyDecisionState =
  typeof AgentClaimRewardPolicyDecisionState.Type

export type AgentClaimRewardPolicyInput = Readonly<{
  legalReviewed: boolean
  campaignPaused: boolean
  budgetAvailable: boolean
  dailyCapRemaining: boolean
  campaignCapRemaining: boolean
  hostedMdkReady: boolean
  sanctionsScreenPassed: boolean
  regionAllowed: boolean
  ownerAccountEligible: boolean
  xAccountEligible: boolean
  xAccountMinimumAgePassed: boolean
  tweetVisibleAndStable: boolean
  destinationReuseCount: number
  destinationReuseLimit: number
  suspiciousDeviceCluster: boolean
  repeatedDestinationCluster: boolean
  coordinatedRewardFarmingSignal: boolean
  manualReviewRequired: boolean
  operatorRejectionReason?: string | undefined
}>

export type AgentClaimRewardPolicyDecision = Readonly<{
  state: AgentClaimRewardPolicyDecisionState
  canCreateEligibility: boolean
  canDispatchLivePayout: boolean
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  manualReviewReasonRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  publicReason: string
  rejectionReason: string | null
}>

export type AgentClaimRewardPublicCounters = Readonly<{
  approved: number
  dispatched: number
  expired: number
  rejected: number
  reversed: number
  settled: number
  totalPublicReceipts: number
}>

const blocker = (suffix: string): string =>
  `blocker.agent_claim_reward.${suffix}`

const reviewReason = (suffix: string): string =>
  `review.agent_claim_reward.${suffix}`

const publicRejectionReason = (value: string): string =>
  value
    .trim()
    .replaceAll(/[\u0000-\u001f\u007f]/g, '')
    .slice(0, 500)

export const evaluateAgentClaimRewardPolicy = (
  input: AgentClaimRewardPolicyInput,
): AgentClaimRewardPolicyDecision => {
  const blockerRefs: Array<string> = []
  const manualReviewReasonRefs: Array<string> = []

  if (!input.legalReviewed) {
    blockerRefs.push(blocker('legal_review_required'))
  }
  if (input.campaignPaused) {
    blockerRefs.push(blocker('operator_pause_active'))
  }
  if (!input.budgetAvailable) {
    blockerRefs.push(blocker('budget_unavailable'))
  }
  if (!input.dailyCapRemaining) {
    blockerRefs.push(blocker('daily_cap_exhausted'))
  }
  if (!input.campaignCapRemaining) {
    blockerRefs.push(blocker('campaign_cap_exhausted'))
  }
  if (!input.hostedMdkReady) {
    blockerRefs.push(blocker('hosted_mdk_not_ready'))
  }
  if (!input.sanctionsScreenPassed || !input.regionAllowed) {
    blockerRefs.push(blocker('sanctions_or_region_gate_failed'))
  }
  if (!input.ownerAccountEligible) {
    blockerRefs.push(blocker('owner_account_ineligible'))
  }
  if (!input.xAccountEligible || !input.xAccountMinimumAgePassed) {
    blockerRefs.push(blocker('x_account_ineligible'))
  }
  if (!input.tweetVisibleAndStable) {
    blockerRefs.push(blocker('x_tweet_not_visible_and_stable'))
  }
  if (input.destinationReuseCount > input.destinationReuseLimit) {
    blockerRefs.push(blocker('destination_reuse_limit_exceeded'))
  }

  if (input.suspiciousDeviceCluster) {
    manualReviewReasonRefs.push(reviewReason('suspicious_device_cluster'))
  }
  if (input.repeatedDestinationCluster) {
    manualReviewReasonRefs.push(reviewReason('repeated_destination_cluster'))
  }
  if (input.coordinatedRewardFarmingSignal) {
    manualReviewReasonRefs.push(reviewReason('coordinated_reward_farming'))
  }
  if (input.manualReviewRequired) {
    manualReviewReasonRefs.push(reviewReason('operator_manual_review'))
  }

  if (input.operatorRejectionReason !== undefined) {
    return {
      blockerRefs,
      canCreateEligibility: false,
      canDispatchLivePayout: false,
      caveatRefs: AGENT_CLAIM_REWARD_REQUIRED_CAVEAT_REFS,
      manualReviewReasonRefs,
      policyRefs: AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS,
      publicReason: 'Claim reward rejected by operator policy.',
      rejectionReason: publicRejectionReason(input.operatorRejectionReason),
      state: 'rejected',
    }
  }

  if (blockerRefs.length > 0) {
    return {
      blockerRefs,
      canCreateEligibility: false,
      canDispatchLivePayout: false,
      caveatRefs: AGENT_CLAIM_REWARD_REQUIRED_CAVEAT_REFS,
      manualReviewReasonRefs,
      policyRefs: AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS,
      publicReason: 'Claim reward blocked until required policy gates pass.',
      rejectionReason: null,
      state: 'blocked',
    }
  }

  if (manualReviewReasonRefs.length > 0) {
    return {
      blockerRefs,
      canCreateEligibility: true,
      canDispatchLivePayout: false,
      caveatRefs: AGENT_CLAIM_REWARD_REQUIRED_CAVEAT_REFS,
      manualReviewReasonRefs,
      policyRefs: AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS,
      publicReason: 'Claim reward requires manual anti-abuse review.',
      rejectionReason: null,
      state: 'manual_review',
    }
  }

  return {
    blockerRefs,
    canCreateEligibility: true,
    canDispatchLivePayout: true,
    caveatRefs: AGENT_CLAIM_REWARD_REQUIRED_CAVEAT_REFS,
    manualReviewReasonRefs,
    policyRefs: AGENT_CLAIM_REWARD_REQUIRED_POLICY_REFS,
    publicReason:
      'Claim reward eligible after legal, budget, anti-abuse, and payout gates.',
    rejectionReason: null,
    state: 'eligible',
  }
}

export const projectAgentClaimRewardPublicCounters = (
  records: ReadonlyArray<Pick<AgentClaimRewardLedgerRecord, 'state'>>,
): AgentClaimRewardPublicCounters => {
  const counts: Record<AgentClaimRewardState, number> = {
    approved: 0,
    dispatched: 0,
    expired: 0,
    payout_intent_created: 0,
    pending: 0,
    rejected: 0,
    reversed: 0,
    settled: 0,
    verified: 0,
  }

  for (const record of records) {
    counts[record.state] += 1
  }

  return {
    approved: counts.approved + counts.payout_intent_created,
    dispatched: counts.dispatched,
    expired: counts.expired,
    rejected: counts.rejected,
    reversed: counts.reversed,
    settled: counts.settled,
    totalPublicReceipts: records.length,
  }
}
