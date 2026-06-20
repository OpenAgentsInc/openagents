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
