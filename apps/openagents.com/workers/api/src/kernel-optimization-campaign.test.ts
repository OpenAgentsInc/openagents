import {
  TASSADAR_TS_REPLAY_CLASS_ID,
  type TassadarReplayVerdict,
  verifyKernelOptimizationParity,
} from '@openagentsinc/tassadar-executor'
import { describe, expect, test } from 'vitest'

import { decodeCreateForumWorkRequestBody } from './forum-work-request-route-contract'
import {
  KernelOptimizationDispatchError,
  type KernelOptimizationJobSpec,
} from './kernel-optimization-work-dispatch'
import {
  KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
  type KernelOptimizationSettlementItem,
  buildKernelOptimizationCampaign,
  reconcileKernelOptimizationCampaignSettlement,
  summarizeKernelOptimizationCampaignSettlement,
} from './kernel-optimization-campaign'

// One campaign across the four smallest Qwen 3.5 models (the historical
// 217.md set Psionic beat), all on the same declared CUDA hardware.
const models = [
  'qwen-3.5-0.5b',
  'qwen-3.5-1.8b',
  'qwen-3.5-4b',
  'qwen-3.5-7b',
] as const

const job = (targetModel: string): KernelOptimizationJobSpec => ({
  baselineRecordRef: `record.public.${targetModel}.cuda.a10g.rmsnorm.328tps`,
  baselineTokensPerSecond: 328,
  budgetSats: 50_000,
  deadlineRef: 'deadline.public.2026-07-01',
  device: 'cuda',
  hardwareRef: 'nvidia-a10g',
  kernelRef: 'rmsnorm',
  targetModel,
  validatorDeviceRef: 'device.public.validator.metal.m3',
})

const verifiedParity: TassadarReplayVerdict = {
  claimedTraceDigest: 'a'.repeat(64),
  classId: TASSADAR_TS_REPLAY_CLASS_ID,
  comparedSteps: 8,
  graphDigest: 'b'.repeat(64),
  outcome: 'verified',
  rejection: null,
  replayedSteps: 8,
  replayedTraceDigest: 'a'.repeat(64),
  validatorDeviceRef: 'device.public.validator.metal.m3',
}
const rejectedParity: TassadarReplayVerdict = {
  ...verifiedParity,
  outcome: 'rejected',
  rejection: { actual: 'c'.repeat(64), reason: 'trace_digest_mismatch' },
  replayedTraceDigest: 'c'.repeat(64),
}

const verdictFor = (
  targetModel: string,
  optimizedTokensPerSecond: number,
  parityVerdict: TassadarReplayVerdict,
) =>
  verifyKernelOptimizationParity({
    baseline: {
      device: 'cuda',
      hardwareRef: 'nvidia-a10g',
      kernelRef: 'baseline-runtime',
      targetModel,
      tokensPerSecond: 328,
    },
    optimized: {
      device: 'cuda',
      hardwareRef: 'nvidia-a10g',
      kernelRef: 'rmsnorm-optimized',
      targetModel,
      tokensPerSecond: optimizedTokensPerSecond,
    },
    parityVerdict,
  })

describe('kernel-optimization campaign fan-out', () => {
  test('fans one campaign into many dispatch-valid, slug-unique requests', () => {
    const campaign = buildKernelOptimizationCampaign({
      campaignRef: 'campaign.qwen35-smallest-4',
      jobs: models.map(job),
    })

    expect(campaign.classId).toBe(KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID)
    expect(campaign.requests).toHaveLength(4)
    expect(campaign.totalBudgetSats).toBe(200_000)

    const slugs = campaign.requests.map((r) => r.requestedSlug)
    expect(new Set(slugs).size).toBe(slugs.length)

    for (const request of campaign.requests) {
      // Each request independently re-decodes => dispatch-valid.
      expect(decodeCreateForumWorkRequestBody(request)).toEqual(request)
    }
  })

  test('rejects an empty campaign', () => {
    expect(() =>
      buildKernelOptimizationCampaign({
        campaignRef: 'campaign.empty',
        jobs: [],
      }),
    ).toThrow(KernelOptimizationDispatchError)
  })

  test('rejects duplicate targets in one campaign', () => {
    expect(() =>
      buildKernelOptimizationCampaign({
        campaignRef: 'campaign.dupe',
        jobs: [job('qwen-3.5-0.5b'), job('qwen-3.5-0.5b')],
      }),
    ).toThrow(KernelOptimizationDispatchError)
  })

  test('rejects a blank campaignRef', () => {
    expect(() =>
      buildKernelOptimizationCampaign({
        campaignRef: '   ',
        jobs: [job('qwen-3.5-0.5b')],
      }),
    ).toThrow(KernelOptimizationDispatchError)
  })
})

describe('kernel-optimization campaign settlement', () => {
  test('pays accepted jobs, refunds rejected, tallies reasons + speedup', () => {
    const items: KernelOptimizationSettlementItem[] = [
      // accepted: faster AND parity-verified
      { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity) },
      { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-1.8b', 410, verifiedParity) },
      // rejected: faster but wrong (parity_rejected)
      { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-4b', 600, rejectedParity) },
      // rejected: parity ok but no throughput improvement
      { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-7b', 300, verifiedParity) },
    ]

    const settlement = summarizeKernelOptimizationCampaignSettlement(
      'campaign.qwen35-smallest-4',
      items,
    )

    expect(settlement.totalJobs).toBe(4)
    expect(settlement.acceptedCount).toBe(2)
    expect(settlement.rejectedCount).toBe(2)
    expect(settlement.payoutOwedSats).toBe(100_000)
    expect(settlement.refundedSats).toBe(100_000)
    expect(settlement.rejectionReasonCounts['parity_rejected']).toBe(1)
    expect(settlement.rejectionReasonCounts['no_throughput_improvement']).toBe(1)
    expect(settlement.speedup).not.toBeNull()
    expect(settlement.speedup?.max).toBeCloseTo(523 / 328, 6)
    expect(settlement.speedup?.min).toBeCloseTo(410 / 328, 6)
  })

  test('returns null speedup when no job is accepted', () => {
    const settlement = summarizeKernelOptimizationCampaignSettlement('campaign.x', [
      { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-0.5b', 600, rejectedParity) },
    ])
    expect(settlement.acceptedCount).toBe(0)
    expect(settlement.speedup).toBeNull()
    expect(settlement.payoutOwedSats).toBe(0)
    expect(settlement.refundedSats).toBe(50_000)
  })

  test('rejects a non-positive-integer settlement budget', () => {
    expect(() =>
      summarizeKernelOptimizationCampaignSettlement('campaign.x', [
        { budgetSats: 1.5, verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity) },
      ]),
    ).toThrow(KernelOptimizationDispatchError)
  })
})

describe('kernel-optimization campaign reconciliation', () => {
  const campaignRef = 'campaign.qwen35-smallest-4'
  const buildFourJobCampaign = () =>
    buildKernelOptimizationCampaign({ campaignRef, jobs: models.map(job) })

  // The full happy-path settlement of the four-job campaign: 2 paid, 2 refunded.
  const fullItems: KernelOptimizationSettlementItem[] = [
    { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity) },
    { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-1.8b', 410, verifiedParity) },
    { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-4b', 600, rejectedParity) },
    { budgetSats: 50_000, verdict: verdictFor('qwen-3.5-7b', 300, verifiedParity) },
  ]

  test('reconciles a complete, escrow-conserving settlement', () => {
    const campaign = buildFourJobCampaign()
    const settlement = summarizeKernelOptimizationCampaignSettlement(
      campaignRef,
      fullItems,
    )

    const report = reconcileKernelOptimizationCampaignSettlement(campaign, settlement)

    expect(report.classId).toBe(KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID)
    expect(report.ok).toBe(true)
    expect(report.discrepancies).toEqual([])
    expect(report.dispatchedJobs).toBe(4)
    expect(report.settledJobs).toBe(4)
    expect(report.jobCountReconciled).toBe(true)
    expect(report.escrowSats).toBe(200_000)
    // 100_000 paid + 100_000 refunded === 200_000 locked.
    expect(report.accountedSats).toBe(200_000)
    expect(report.escrowConserved).toBe(true)
  })

  test('flags a dropped job (job-count drift)', () => {
    const campaign = buildFourJobCampaign()
    // Settle only three of the four dispatched jobs.
    const settlement = summarizeKernelOptimizationCampaignSettlement(
      campaignRef,
      fullItems.slice(0, 3),
    )

    const report = reconcileKernelOptimizationCampaignSettlement(campaign, settlement)

    expect(report.ok).toBe(false)
    expect(report.jobCountReconciled).toBe(false)
    expect(report.dispatchedJobs).toBe(4)
    expect(report.settledJobs).toBe(3)
    // Dropping a refunded job also leaks escrow.
    expect(report.escrowConserved).toBe(false)
    expect(report.discrepancies.some((d) => d.includes('job-count drift'))).toBe(true)
    expect(report.discrepancies.some((d) => d.includes('escrow drift'))).toBe(true)
  })

  test('flags escrow drift even when the job count matches', () => {
    const campaign = buildFourJobCampaign()
    // Same four jobs, but one was settled for the wrong escrow amount.
    const tampered: KernelOptimizationSettlementItem[] = [
      ...fullItems.slice(0, 3),
      { budgetSats: 49_999, verdict: fullItems[3]!.verdict },
    ]
    const settlement = summarizeKernelOptimizationCampaignSettlement(
      campaignRef,
      tampered,
    )

    const report = reconcileKernelOptimizationCampaignSettlement(campaign, settlement)

    expect(report.jobCountReconciled).toBe(true)
    expect(report.escrowConserved).toBe(false)
    expect(report.ok).toBe(false)
    expect(report.accountedSats).toBe(199_999)
    expect(report.discrepancies.some((d) => d.includes('escrow drift'))).toBe(true)
  })

  test('flags reconciling mismatched campaigns', () => {
    const campaign = buildFourJobCampaign()
    const settlement = summarizeKernelOptimizationCampaignSettlement(
      'campaign.some-other-run',
      fullItems,
    )

    const report = reconcileKernelOptimizationCampaignSettlement(campaign, settlement)

    expect(report.ok).toBe(false)
    expect(report.campaignRef).toBe(campaignRef)
    expect(report.discrepancies.some((d) => d.includes('campaignRef mismatch'))).toBe(
      true,
    )
  })
})
