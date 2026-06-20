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
  type KernelOptimizationKeyedSettlementItem,
  type KernelOptimizationSettlementItem,
  buildKernelOptimizationCampaign,
  evaluateKernelOptimizationCampaignRelease,
  reconcileKernelOptimizationCampaignBaselines,
  reconcileKernelOptimizationCampaignOps,
  reconcileKernelOptimizationCampaignPerJob,
  reconcileKernelOptimizationCampaignSettlement,
  reconcileKernelOptimizationCampaignTargets,
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

// The baseline kernel replayed on the same validator device, yielding the same
// output trace ('a') as the verified optimized replay => cross-graph output
// equivalence. Its graph is the baseline record's graph (set below).
const baselineParity: TassadarReplayVerdict = { ...verifiedParity }

const verdictFor = (
  targetModel: string,
  optimizedTokensPerSecond: number,
  parityVerdict: TassadarReplayVerdict,
  optimizedOpRef = 'rmsnorm',
) =>
  verifyKernelOptimizationParity({
    baseline: {
      device: 'cuda',
      graphDigest: parityVerdict.graphDigest,
      hardwareRef: 'nvidia-a10g',
      kernelRef: 'baseline-runtime',
      opRef: optimizedOpRef,
      targetModel,
      tokensPerSecond: 328,
    },
    baselineParityVerdict: { ...baselineParity, graphDigest: parityVerdict.graphDigest },
    optimized: {
      device: 'cuda',
      graphDigest: parityVerdict.graphDigest,
      hardwareRef: 'nvidia-a10g',
      kernelRef: `${optimizedOpRef}-optimized`,
      opRef: optimizedOpRef,
      targetModel,
      tokensPerSecond: optimizedTokensPerSecond,
    },
    optimizedOpRef,
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

describe('kernel-optimization campaign per-job reconciliation', () => {
  const campaignRef = 'campaign.qwen35-smallest-4'
  const buildFourJobCampaign = () =>
    buildKernelOptimizationCampaign({ campaignRef, jobs: models.map(job) })

  // Key each settlement to its dispatched job by the campaign's unique slug.
  const keyedItem = (
    campaign: ReturnType<typeof buildFourJobCampaign>,
    index: number,
    item: KernelOptimizationSettlementItem,
  ): KernelOptimizationKeyedSettlementItem => ({
    ...item,
    requestedSlug: campaign.requests[index]!.requestedSlug ?? '',
  })

  const fullKeyedItems = (campaign: ReturnType<typeof buildFourJobCampaign>) => [
    keyedItem(campaign, 0, {
      budgetSats: 50_000,
      verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity),
    }),
    keyedItem(campaign, 1, {
      budgetSats: 50_000,
      verdict: verdictFor('qwen-3.5-1.8b', 410, verifiedParity),
    }),
    keyedItem(campaign, 2, {
      budgetSats: 50_000,
      verdict: verdictFor('qwen-3.5-4b', 600, rejectedParity),
    }),
    keyedItem(campaign, 3, {
      budgetSats: 50_000,
      verdict: verdictFor('qwen-3.5-7b', 300, verifiedParity),
    }),
  ]

  test('matches every settlement to its dispatched job', () => {
    const campaign = buildFourJobCampaign()
    const report = reconcileKernelOptimizationCampaignPerJob(
      campaign,
      fullKeyedItems(campaign),
    )

    expect(report.classId).toBe(KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID)
    expect(report.ok).toBe(true)
    expect(report.discrepancies).toEqual([])
    expect(report.matchedSlugs).toHaveLength(4)
    expect(report.unsettledSlugs).toEqual([])
    expect(report.unexpectedSlugs).toEqual([])
    expect(report.duplicateSlugs).toEqual([])
    expect(report.budgetMismatches).toEqual([])
  })

  test('names the specific dispatched job that was never settled', () => {
    const campaign = buildFourJobCampaign()
    const items = fullKeyedItems(campaign).slice(0, 3)
    const report = reconcileKernelOptimizationCampaignPerJob(campaign, items)

    expect(report.ok).toBe(false)
    expect(report.matchedSlugs).toHaveLength(3)
    expect(report.unsettledSlugs).toEqual([
      campaign.requests[3]!.requestedSlug ?? '',
    ])
    expect(
      report.discrepancies.some((d) => d.includes('never settled')),
    ).toBe(true)
  })

  test('catches the offsetting double-settle the totals check misses', () => {
    const campaign = buildFourJobCampaign()
    const items = fullKeyedItems(campaign)
    // Settle job 0 twice and drop job 3: job COUNT (4) and total escrow
    // (200_000) both still match, so the totals reconciler would pass.
    const tampered = [items[0]!, items[1]!, items[2]!, items[0]!]

    const totals = reconcileKernelOptimizationCampaignSettlement(
      campaign,
      summarizeKernelOptimizationCampaignSettlement(
        campaignRef,
        tampered,
      ),
    )
    expect(totals.jobCountReconciled).toBe(true)
    expect(totals.escrowConserved).toBe(true)
    expect(totals.ok).toBe(true)

    const perJob = reconcileKernelOptimizationCampaignPerJob(campaign, tampered)
    expect(perJob.ok).toBe(false)
    expect(perJob.duplicateSlugs).toEqual([
      campaign.requests[0]!.requestedSlug ?? '',
    ])
    expect(perJob.unsettledSlugs).toEqual([
      campaign.requests[3]!.requestedSlug ?? '',
    ])
  })

  test('flags a settlement for a job the campaign never dispatched', () => {
    const campaign = buildFourJobCampaign()
    const items = [
      ...fullKeyedItems(campaign).slice(0, 3),
      {
        budgetSats: 50_000,
        requestedSlug: 'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
        verdict: verdictFor('qwen-3.5-7b', 300, verifiedParity),
      },
    ]
    const report = reconcileKernelOptimizationCampaignPerJob(campaign, items)

    expect(report.ok).toBe(false)
    expect(report.unexpectedSlugs).toEqual([
      'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
    ])
    expect(report.unsettledSlugs).toEqual([
      campaign.requests[3]!.requestedSlug ?? '',
    ])
    expect(
      report.discrepancies.some((d) => d.includes('never dispatched')),
    ).toBe(true)
  })

  test('flags per-job escrow drift even when the slug matches', () => {
    const campaign = buildFourJobCampaign()
    const items = fullKeyedItems(campaign)
    const tampered = [
      items[0]!,
      items[1]!,
      items[2]!,
      { ...items[3]!, budgetSats: 49_999 },
    ]
    const report = reconcileKernelOptimizationCampaignPerJob(campaign, tampered)

    expect(report.ok).toBe(false)
    expect(report.matchedSlugs).toHaveLength(3)
    expect(report.budgetMismatches).toHaveLength(1)
    expect(report.budgetMismatches[0]?.dispatchedSats).toBe(50_000)
    expect(report.budgetMismatches[0]?.settledSats).toBe(49_999)
    expect(
      report.discrepancies.some((d) => d.includes('per-job escrow drift')),
    ).toBe(true)
  })
})

describe('kernel-optimization campaign target reconciliation', () => {
  const campaignRef = 'campaign.qwen35-smallest-4'
  const spec = { campaignRef, jobs: models.map(job) }
  const buildFourJobCampaign = () => buildKernelOptimizationCampaign(spec)

  // A clean settlement: each slug carries a verdict for its own dispatched
  // target (same model/device/hardware the job optimizes).
  const cleanKeyedItems = (
    campaign: ReturnType<typeof buildFourJobCampaign>,
  ): KernelOptimizationKeyedSettlementItem[] =>
    models.map((model, index) => ({
      budgetSats: 50_000,
      requestedSlug: campaign.requests[index]!.requestedSlug ?? '',
      verdict: verdictFor(model, 523, verifiedParity),
    }))

  test('passes when every settled verdict optimizes its dispatched target', () => {
    const campaign = buildFourJobCampaign()
    const report = reconcileKernelOptimizationCampaignTargets(
      spec,
      cleanKeyedItems(campaign),
    )

    expect(report.classId).toBe(KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID)
    expect(report.campaignRef).toBe(campaignRef)
    expect(report.ok).toBe(true)
    expect(report.matchedSlugs).toHaveLength(4)
    expect(report.targetMismatches).toEqual([])
    expect(report.unmatchedSlugs).toEqual([])
    expect(report.discrepancies).toEqual([])
  })

  test('catches a target swap that the slug + escrow checks both miss', () => {
    const campaign = buildFourJobCampaign()
    const items = cleanKeyedItems(campaign)
    // File a verdict for the qwen-3.5-7b target under the qwen-3.5-0.5b job's
    // slug, keeping that job's exact budget. The per-job (slug + escrow)
    // reconciler still passes; only the target reconciler catches it.
    const swapped: KernelOptimizationKeyedSettlementItem[] = [
      {
        budgetSats: 50_000,
        requestedSlug: items[0]!.requestedSlug,
        verdict: verdictFor('qwen-3.5-7b', 523, verifiedParity),
      },
      items[1]!,
      items[2]!,
      items[3]!,
    ]

    const perJob = reconcileKernelOptimizationCampaignPerJob(campaign, swapped)
    expect(perJob.ok).toBe(true)

    const report = reconcileKernelOptimizationCampaignTargets(spec, swapped)
    expect(report.ok).toBe(false)
    expect(report.matchedSlugs).toHaveLength(3)
    expect(report.targetMismatches).toHaveLength(1)
    expect(report.targetMismatches[0]?.requestedSlug).toBe(
      campaign.requests[0]!.requestedSlug ?? '',
    )
    expect(report.targetMismatches[0]?.dispatchedTarget.targetModel).toBe(
      'qwen-3.5-0.5b',
    )
    expect(report.targetMismatches[0]?.settledVerdictTarget.targetModel).toBe(
      'qwen-3.5-7b',
    )
    expect(report.discrepancies.some((d) => d.includes('target swap'))).toBe(
      true,
    )
  })

  test('flags a settlement for a slug this campaign never dispatched', () => {
    const campaign = buildFourJobCampaign()
    const items: KernelOptimizationKeyedSettlementItem[] = [
      ...cleanKeyedItems(campaign).slice(0, 3),
      {
        budgetSats: 50_000,
        requestedSlug: 'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
        verdict: verdictFor('qwen-3.5-7b', 523, verifiedParity),
      },
    ]
    const report = reconcileKernelOptimizationCampaignTargets(spec, items)

    expect(report.ok).toBe(false)
    expect(report.unmatchedSlugs).toEqual([
      'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
    ])
    expect(
      report.discrepancies.some((d) => d.includes('never dispatched')),
    ).toBe(true)
  })

  test('rejects a spec the campaign builder would reject (empty jobs)', () => {
    expect(() =>
      reconcileKernelOptimizationCampaignTargets(
        { campaignRef, jobs: [] },
        [],
      ),
    ).toThrow(KernelOptimizationDispatchError)
  })
})

describe('kernel-optimization campaign op reconciliation', () => {
  // Two ops dispatched against the SAME (model, device, hardware) target — the
  // case the model/device/hardware target reconciler cannot tell apart.
  const opJob = (
    targetModel: string,
    kernelRef: string,
  ): KernelOptimizationJobSpec => ({
    baselineRecordRef: `record.public.${targetModel}.cuda.a10g.${kernelRef}.328tps`,
    baselineTokensPerSecond: 328,
    budgetSats: 50_000,
    deadlineRef: 'deadline.public.2026-07-01',
    device: 'cuda',
    hardwareRef: 'nvidia-a10g',
    kernelRef,
    targetModel,
    validatorDeviceRef: 'device.public.validator.metal.m3',
  })

  const campaignRef = 'campaign.qwen35-0.5b-two-ops'
  const spec = {
    campaignRef,
    jobs: [
      opJob('qwen-3.5-0.5b', 'rmsnorm'),
      opJob('qwen-3.5-0.5b', 'attention.flash'),
    ],
  }
  const buildTwoOpCampaign = () => buildKernelOptimizationCampaign(spec)

  const keyedFor = (
    campaign: ReturnType<typeof buildTwoOpCampaign>,
    index: number,
    op: string,
  ): KernelOptimizationKeyedSettlementItem => ({
    budgetSats: 50_000,
    requestedSlug: campaign.requests[index]!.requestedSlug ?? '',
    verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity, op),
  })

  test('passes when every settled verdict optimizes its dispatched op', () => {
    const campaign = buildTwoOpCampaign()
    const items = [
      keyedFor(campaign, 0, 'rmsnorm'),
      keyedFor(campaign, 1, 'attention.flash'),
    ]
    const report = reconcileKernelOptimizationCampaignOps(spec, items)

    expect(report.classId).toBe(KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID)
    expect(report.campaignRef).toBe(campaignRef)
    expect(report.ok).toBe(true)
    expect(report.matchedSlugs).toHaveLength(2)
    expect(report.opMismatches).toEqual([])
    expect(report.unmatchedSlugs).toEqual([])
    expect(report.discrepancies).toEqual([])
  })

  test('catches an op swap the model/device/hardware target check misses', () => {
    const campaign = buildTwoOpCampaign()
    // File the attention.flash verdict under the rmsnorm job's slug, keeping
    // that job's exact target and budget. Both jobs share the same target, so
    // the target reconciler and the per-job (slug + escrow) reconciler pass.
    const swapped: KernelOptimizationKeyedSettlementItem[] = [
      {
        budgetSats: 50_000,
        requestedSlug: campaign.requests[0]!.requestedSlug ?? '',
        verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity, 'attention.flash'),
      },
      keyedFor(campaign, 1, 'attention.flash'),
    ]

    // The coarse target reconciler is blind to it (same model/device/hardware).
    const target = reconcileKernelOptimizationCampaignTargets(spec, swapped)
    expect(target.ok).toBe(true)
    // The per-job slug + escrow reconciler is blind to it too.
    const perJob = reconcileKernelOptimizationCampaignPerJob(campaign, swapped)
    expect(perJob.ok).toBe(true)

    // Only the op reconciler catches it.
    const report = reconcileKernelOptimizationCampaignOps(spec, swapped)
    expect(report.ok).toBe(false)
    expect(report.matchedSlugs).toHaveLength(1)
    expect(report.opMismatches).toHaveLength(1)
    expect(report.opMismatches[0]?.requestedSlug).toBe(
      campaign.requests[0]!.requestedSlug ?? '',
    )
    expect(report.opMismatches[0]?.dispatchedOpRef).toBe('rmsnorm')
    expect(report.opMismatches[0]?.settledVerdictOpRef).toBe('attention.flash')
    expect(report.discrepancies.some((d) => d.includes('op swap'))).toBe(true)
  })

  test('treats a blank verdict op as a mismatch', () => {
    const campaign = buildTwoOpCampaign()
    const items: KernelOptimizationKeyedSettlementItem[] = [
      {
        budgetSats: 50_000,
        requestedSlug: campaign.requests[0]!.requestedSlug ?? '',
        verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity, '   '),
      },
      keyedFor(campaign, 1, 'attention.flash'),
    ]
    const report = reconcileKernelOptimizationCampaignOps(spec, items)

    expect(report.ok).toBe(false)
    expect(report.opMismatches).toHaveLength(1)
    expect(report.opMismatches[0]?.settledVerdictOpRef).toBe('')
    expect(report.discrepancies.some((d) => d.includes('(blank)'))).toBe(true)
  })

  test('flags a settlement for a slug this campaign never dispatched', () => {
    const campaign = buildTwoOpCampaign()
    const items: KernelOptimizationKeyedSettlementItem[] = [
      keyedFor(campaign, 0, 'rmsnorm'),
      {
        budgetSats: 50_000,
        requestedSlug: 'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
        verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity),
      },
    ]
    const report = reconcileKernelOptimizationCampaignOps(spec, items)

    expect(report.ok).toBe(false)
    expect(report.unmatchedSlugs).toEqual([
      'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
    ])
    expect(
      report.discrepancies.some((d) => d.includes('never dispatched')),
    ).toBe(true)
  })

  test('rejects a spec the campaign builder would reject (empty jobs)', () => {
    expect(() =>
      reconcileKernelOptimizationCampaignOps({ campaignRef, jobs: [] }, []),
    ).toThrow(KernelOptimizationDispatchError)
  })
})

describe('kernel-optimization campaign baseline reconciliation', () => {
  // Two ops on the same target, each dispatched to beat a named 328 tok/s
  // baseline (see `job`/`opJob` above: baselineTokensPerSecond: 328).
  const opJob = (
    targetModel: string,
    kernelRef: string,
  ): KernelOptimizationJobSpec => ({
    baselineRecordRef: `record.public.${targetModel}.cuda.a10g.${kernelRef}.328tps`,
    baselineTokensPerSecond: 328,
    budgetSats: 50_000,
    deadlineRef: 'deadline.public.2026-07-01',
    device: 'cuda',
    hardwareRef: 'nvidia-a10g',
    kernelRef,
    targetModel,
    validatorDeviceRef: 'device.public.validator.metal.m3',
  })

  const campaignRef = 'campaign.qwen35-0.5b-baseline'
  const spec = {
    campaignRef,
    jobs: [
      opJob('qwen-3.5-0.5b', 'rmsnorm'),
      opJob('qwen-3.5-0.5b', 'attention.flash'),
    ],
  }
  const campaign = buildKernelOptimizationCampaign(spec)

  // An ACCEPTED verdict (verified parity + improvement) whose speedup was scored
  // against a weaker baseline than the dispatched named 328 tok/s floor.
  const verdictAgainstBaseline = (
    op: string,
    baselineTokensPerSecond: number,
    optimizedTokensPerSecond: number,
  ) =>
    verifyKernelOptimizationParity({
      baseline: {
        device: 'cuda',
        graphDigest: verifiedParity.graphDigest,
        hardwareRef: 'nvidia-a10g',
        kernelRef: 'baseline-runtime',
        opRef: op,
        targetModel: 'qwen-3.5-0.5b',
        tokensPerSecond: baselineTokensPerSecond,
      },
      baselineParityVerdict: {
        ...baselineParity,
        graphDigest: verifiedParity.graphDigest,
      },
      optimized: {
        device: 'cuda',
        graphDigest: verifiedParity.graphDigest,
        hardwareRef: 'nvidia-a10g',
        kernelRef: `${op}-optimized`,
        opRef: op,
        targetModel: 'qwen-3.5-0.5b',
        tokensPerSecond: optimizedTokensPerSecond,
      },
      optimizedOpRef: op,
      parityVerdict: verifiedParity,
    })

  test('passes when every verdict scored against its dispatched named baseline', () => {
    const items: KernelOptimizationKeyedSettlementItem[] = [
      {
        budgetSats: 50_000,
        requestedSlug: campaign.requests[0]!.requestedSlug ?? '',
        verdict: verdictAgainstBaseline('rmsnorm', 328, 523),
      },
      {
        budgetSats: 50_000,
        requestedSlug: campaign.requests[1]!.requestedSlug ?? '',
        verdict: verdictAgainstBaseline('attention.flash', 328, 401),
      },
    ]
    const report = reconcileKernelOptimizationCampaignBaselines(spec, items)

    expect(report.classId).toBe(KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID)
    expect(report.campaignRef).toBe(campaignRef)
    expect(report.ok).toBe(true)
    expect(report.matchedSlugs).toHaveLength(2)
    expect(report.baselineMismatches).toEqual([])
    expect(report.discrepancies).toEqual([])
  })

  test('catches a baseline swap the target, op, and per-job checks all miss', () => {
    // The rmsnorm job's verdict was scored against a cherry-picked 200 tok/s
    // baseline (beating it at 250) instead of the dispatched named 328 floor.
    // Same slug, escrow, target, and op, so every other reconciler passes.
    const swapped: KernelOptimizationKeyedSettlementItem[] = [
      {
        budgetSats: 50_000,
        requestedSlug: campaign.requests[0]!.requestedSlug ?? '',
        verdict: verdictAgainstBaseline('rmsnorm', 200, 250),
      },
      {
        budgetSats: 50_000,
        requestedSlug: campaign.requests[1]!.requestedSlug ?? '',
        verdict: verdictAgainstBaseline('attention.flash', 328, 401),
      },
    ]

    // Every other reconciler is blind to it.
    expect(reconcileKernelOptimizationCampaignTargets(spec, swapped).ok).toBe(true)
    expect(reconcileKernelOptimizationCampaignOps(spec, swapped).ok).toBe(true)
    expect(
      reconcileKernelOptimizationCampaignPerJob(campaign, swapped).ok,
    ).toBe(true)

    // Only the baseline reconciler catches it.
    const report = reconcileKernelOptimizationCampaignBaselines(spec, swapped)
    expect(report.ok).toBe(false)
    expect(report.matchedSlugs).toHaveLength(1)
    expect(report.baselineMismatches).toHaveLength(1)
    expect(report.baselineMismatches[0]?.requestedSlug).toBe(
      campaign.requests[0]!.requestedSlug ?? '',
    )
    expect(
      report.baselineMismatches[0]?.dispatchedBaselineTokensPerSecond,
    ).toBe(328)
    expect(
      report.baselineMismatches[0]?.settledVerdictBaselineTokensPerSecond,
    ).toBe(200)
    expect(report.discrepancies.some((d) => d.includes('baseline swap'))).toBe(
      true,
    )
  })

  test('flags a settlement for a slug this campaign never dispatched', () => {
    const items: KernelOptimizationKeyedSettlementItem[] = [
      {
        budgetSats: 50_000,
        requestedSlug: campaign.requests[0]!.requestedSlug ?? '',
        verdict: verdictAgainstBaseline('rmsnorm', 328, 523),
      },
      {
        budgetSats: 50_000,
        requestedSlug: 'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
        verdict: verdictAgainstBaseline('rmsnorm', 328, 523),
      },
    ]
    const report = reconcileKernelOptimizationCampaignBaselines(spec, items)

    expect(report.ok).toBe(false)
    expect(report.unmatchedSlugs).toEqual([
      'kernel-opt-rmsnorm-not-in-this-campaign-cuda',
    ])
    expect(
      report.discrepancies.some((d) => d.includes('never dispatched')),
    ).toBe(true)
  })

  test('rejects a spec the campaign builder would reject (empty jobs)', () => {
    expect(() =>
      reconcileKernelOptimizationCampaignBaselines({ campaignRef, jobs: [] }, []),
    ).toThrow(KernelOptimizationDispatchError)
  })
})

describe('kernel-optimization campaign release gate', () => {
  // A campaign hosting two ops on the SAME target, so every gate is exercised:
  // the totals/per-job gates are blind to op swaps, and the coarse target gate
  // is blind to them too — only the op gate tells the two jobs apart.
  const opJob = (
    targetModel: string,
    kernelRef: string,
  ): KernelOptimizationJobSpec => ({
    baselineRecordRef: `record.public.${targetModel}.cuda.a10g.${kernelRef}.328tps`,
    baselineTokensPerSecond: 328,
    budgetSats: 50_000,
    deadlineRef: 'deadline.public.2026-07-01',
    device: 'cuda',
    hardwareRef: 'nvidia-a10g',
    kernelRef,
    targetModel,
    validatorDeviceRef: 'device.public.validator.metal.m3',
  })

  const campaignRef = 'campaign.qwen35-0.5b-release-gate'
  const spec = {
    campaignRef,
    jobs: [
      opJob('qwen-3.5-0.5b', 'rmsnorm'),
      opJob('qwen-3.5-0.5b', 'attention.flash'),
    ],
  }
  const campaign = buildKernelOptimizationCampaign(spec)

  const keyedFor = (
    index: number,
    op: string,
    overrides: Partial<KernelOptimizationKeyedSettlementItem> = {},
  ): KernelOptimizationKeyedSettlementItem => ({
    budgetSats: 50_000,
    requestedSlug: campaign.requests[index]!.requestedSlug ?? '',
    verdict: verdictFor('qwen-3.5-0.5b', 523, verifiedParity, op),
    ...overrides,
  })

  test('is ok and surfaces the settlement when every gate holds', () => {
    const items = [keyedFor(0, 'rmsnorm'), keyedFor(1, 'attention.flash')]
    const gate = evaluateKernelOptimizationCampaignRelease(spec, items)

    expect(gate.classId).toBe(KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID)
    expect(gate.campaignRef).toBe(campaignRef)
    expect(gate.ok).toBe(true)
    expect(gate.discrepancies).toHaveLength(0)
    expect(gate.totals.ok).toBe(true)
    expect(gate.perJob.ok).toBe(true)
    expect(gate.targets.ok).toBe(true)
    expect(gate.ops.ok).toBe(true)
    expect(gate.baselines.ok).toBe(true)
    // The settlement the gate evaluated is exposed: both accepted, full payout.
    expect(gate.settlement.acceptedCount).toBe(2)
    expect(gate.settlement.payoutOwedSats).toBe(100_000)
    expect(gate.settlement.refundedSats).toBe(0)
  })

  test('fails when only the op gate catches a same-target op swap', () => {
    // Both items filed under their dispatched slug with the dispatched escrow and
    // the dispatched (model/device/hardware) target, but the verdict under the
    // rmsnorm slug actually optimized attention.flash. Totals + per-job + target
    // all pass; only the op gate catches it — proving the composite gate is not
    // redundant with any single reconciler.
    const items = [
      keyedFor(0, 'attention.flash'),
      keyedFor(1, 'attention.flash'),
    ]
    const gate = evaluateKernelOptimizationCampaignRelease(spec, items)

    expect(gate.ok).toBe(false)
    expect(gate.totals.ok).toBe(true)
    expect(gate.perJob.ok).toBe(true)
    expect(gate.targets.ok).toBe(true)
    expect(gate.ops.ok).toBe(false)
    expect(gate.discrepancies.some((d) => d.startsWith('op: '))).toBe(true)
    expect(gate.discrepancies.some((d) => d.includes('op swap'))).toBe(true)
  })

  test('fails when only the baseline gate catches a cherry-picked baseline', () => {
    // Both items filed under their dispatched slug, escrow, target, and op, but
    // the rmsnorm verdict scored its speedup against a weaker 200 tok/s baseline
    // instead of the dispatched named 328 floor. Totals + per-job + target + op
    // all pass; only the baseline gate catches it — proving the composite gate is
    // not redundant with any single reconciler.
    const cherryPicked = verifyKernelOptimizationParity({
      baseline: {
        device: 'cuda',
        graphDigest: verifiedParity.graphDigest,
        hardwareRef: 'nvidia-a10g',
        kernelRef: 'baseline-runtime',
        opRef: 'rmsnorm',
        targetModel: 'qwen-3.5-0.5b',
        tokensPerSecond: 200,
      },
      baselineParityVerdict: {
        ...baselineParity,
        graphDigest: verifiedParity.graphDigest,
      },
      optimized: {
        device: 'cuda',
        graphDigest: verifiedParity.graphDigest,
        hardwareRef: 'nvidia-a10g',
        kernelRef: 'rmsnorm-optimized',
        opRef: 'rmsnorm',
        targetModel: 'qwen-3.5-0.5b',
        tokensPerSecond: 250,
      },
      optimizedOpRef: 'rmsnorm',
      parityVerdict: verifiedParity,
    })
    const items = [
      keyedFor(0, 'rmsnorm', { verdict: cherryPicked }),
      keyedFor(1, 'attention.flash'),
    ]
    const gate = evaluateKernelOptimizationCampaignRelease(spec, items)

    expect(gate.ok).toBe(false)
    expect(gate.totals.ok).toBe(true)
    expect(gate.perJob.ok).toBe(true)
    expect(gate.targets.ok).toBe(true)
    expect(gate.ops.ok).toBe(true)
    expect(gate.baselines.ok).toBe(false)
    expect(gate.discrepancies.some((d) => d.startsWith('baseline: '))).toBe(true)
    expect(gate.discrepancies.some((d) => d.includes('baseline swap'))).toBe(true)
  })

  test('fails and prefixes per-job drift when a job is settled twice', () => {
    // rmsnorm settled twice, attention.flash never => per-job catches the
    // duplicate + the unsettled job, but total job count and escrow still match
    // (2 items, 100_000 sat), so the totals gate is blind to it.
    const items = [
      keyedFor(0, 'rmsnorm'),
      keyedFor(0, 'rmsnorm'),
    ]
    const gate = evaluateKernelOptimizationCampaignRelease(spec, items)

    expect(gate.ok).toBe(false)
    expect(gate.totals.ok).toBe(true)
    expect(gate.perJob.ok).toBe(false)
    expect(gate.discrepancies.some((d) => d.startsWith('per-job: '))).toBe(true)
  })

  test('rejects a structurally invalid spec the builder would reject', () => {
    expect(() =>
      evaluateKernelOptimizationCampaignRelease({ campaignRef, jobs: [] }, []),
    ).toThrow(KernelOptimizationDispatchError)
  })
})
