/**
 * Kernel-optimization at-scale campaign fan-out + settlement aggregation.
 *
 * Advances `compute.agentic_kernel_optimization_at_scale.v1`'s
 * at-scale-run blocker by composing the two pieces that already exist —
 * the market-dispatch encoder (`buildKernelOptimizationWorkRequest`) and the
 * throughput-parity verdict (`verifyKernelOptimizationParity` /
 * `KernelOptimizationVerdict`) — into the across-the-mesh shape the promise
 * names: ONE campaign that fans out across MANY (model, device, kernel)
 * targets into many dispatch-valid work requests, and a settlement ledger that
 * reduces the many parity verdicts back into one campaign result.
 *
 * It moves no money, posts no request, and runs no kernel. It produces (a) the
 * set of dispatch payloads the forum work-request route already accepts, with a
 * guaranteed-unique requested slug per job so an at-scale dispatch cannot
 * silently collide, and (b) the settlement ledger the verified-work rail would
 * execute: pay accepted jobs, refund rejected ones, all mechanically derived
 * from the parity verdicts (faster AND still correct), never operator judgment.
 *
 * What it does NOT do (still red): post the requests, run real kernels, capture
 * live tok/s, replay real output traces, or settle real escrow.
 */
import type { KernelOptimizationVerdict } from '@openagentsinc/tassadar-executor'

import type { CreateForumWorkRequestBody } from './forum-work-request-route-contract'
import {
  KernelOptimizationDispatchError,
  type KernelOptimizationJobSpec,
  buildKernelOptimizationWorkRequest,
} from './kernel-optimization-work-dispatch'

export const KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID =
  'kernel_optimization_campaign.v1'

/** A single campaign: one batch of kernel-optimization jobs across targets. */
export type KernelOptimizationCampaignSpec = Readonly<{
  /** Public ref identifying this campaign (e.g. "campaign.qwen35-smallest-4"). */
  campaignRef: string
  /** The jobs to dispatch; non-empty, with distinct targets. */
  jobs: ReadonlyArray<KernelOptimizationJobSpec>
}>

/** The dispatch-ready fan-out of one campaign. */
export type KernelOptimizationCampaign = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  /** One dispatch-valid forum work-request body per job, slug-unique. */
  requests: ReadonlyArray<CreateForumWorkRequestBody>
  /** Total escrow the campaign would lock across all jobs, in whole sats. */
  totalBudgetSats: number
}>

/** One settled job: its escrowed budget paired with its parity verdict. */
export type KernelOptimizationSettlementItem = Readonly<{
  /** Escrowed budget for this job, in whole sats; positive integer. */
  budgetSats: number
  verdict: KernelOptimizationVerdict
}>

/** The settlement ledger reducing many verdicts back into one campaign result. */
export type KernelOptimizationCampaignSettlement = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  totalJobs: number
  acceptedCount: number
  rejectedCount: number
  /** Sum of budgets over accepted jobs — what the rail would pay out. */
  payoutOwedSats: number
  /** Sum of budgets over rejected jobs — what the rail would refund. */
  refundedSats: number
  /** Count of rejected jobs by parity-verdict rejection reason. */
  rejectionReasonCounts: Readonly<Record<string, number>>
  /** Speedup ratio stats over accepted jobs, or null when none accepted. */
  speedup: Readonly<{ min: number; max: number; mean: number }> | null
}>

const isPositiveIntegerSats = (value: number): boolean =>
  Number.isInteger(value) && value > 0 && value <= 21_000_000_000_000

const targetKey = (job: KernelOptimizationJobSpec): string =>
  [job.kernelRef, job.targetModel, job.device, job.hardwareRef]
    .map((part) => part.trim().toLowerCase())
    .join('|')

/**
 * Fan one campaign out into many dispatch-valid work requests.
 *
 * Guards the at-scale invariants the single-job encoder cannot: the campaign is
 * non-empty, no two jobs share the same (kernel, model, device, hardware)
 * target, and — because the encoder truncates slugs to 80 chars — no two jobs
 * collapse to the same requested slug. Any violation throws rather than
 * dispatching a silently-degenerate batch.
 */
export const buildKernelOptimizationCampaign = (
  spec: KernelOptimizationCampaignSpec,
): KernelOptimizationCampaign => {
  const campaignRef = spec.campaignRef.trim()
  if (campaignRef.length === 0) {
    throw new KernelOptimizationDispatchError('campaignRef must be non-empty.')
  }
  if (spec.jobs.length === 0) {
    throw new KernelOptimizationDispatchError(
      'campaign must contain at least one job.',
    )
  }

  const seenTargets = new Set<string>()
  for (const job of spec.jobs) {
    const key = targetKey(job)
    if (seenTargets.has(key)) {
      throw new KernelOptimizationDispatchError(
        `duplicate campaign target: ${key}`,
      )
    }
    seenTargets.add(key)
  }

  const requests = spec.jobs.map((job) => buildKernelOptimizationWorkRequest(job))

  const seenSlugs = new Set<string>()
  let totalBudgetSats = 0
  for (const request of requests) {
    const slug = request.requestedSlug ?? ''
    if (seenSlugs.has(slug)) {
      throw new KernelOptimizationDispatchError(
        `campaign jobs collapse to the same requested slug: ${slug}`,
      )
    }
    seenSlugs.add(slug)
    totalBudgetSats += request.budgetSats
  }

  return {
    campaignRef,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    requests,
    totalBudgetSats,
  }
}

/**
 * Reduce the parity verdicts of a settled campaign into one settlement ledger.
 *
 * Acceptance is taken straight from each verdict (`outcome === "accepted"`),
 * which already encodes "faster AND still correct". Accepted jobs accrue
 * payout; rejected jobs accrue refund and are tallied by rejection reason, so
 * an at-scale failure mode (e.g. many `parity_rejected`) is legible without
 * re-reading every verdict.
 */
export const summarizeKernelOptimizationCampaignSettlement = (
  campaignRef: string,
  items: ReadonlyArray<KernelOptimizationSettlementItem>,
): KernelOptimizationCampaignSettlement => {
  const ref = campaignRef.trim()
  if (ref.length === 0) {
    throw new KernelOptimizationDispatchError('campaignRef must be non-empty.')
  }

  let acceptedCount = 0
  let rejectedCount = 0
  let payoutOwedSats = 0
  let refundedSats = 0
  const rejectionReasonCounts: Record<string, number> = {}
  const acceptedSpeedups: number[] = []

  for (const item of items) {
    if (!isPositiveIntegerSats(item.budgetSats)) {
      throw new KernelOptimizationDispatchError(
        `settlement budgetSats must be a positive integer sat amount (got ${item.budgetSats}).`,
      )
    }
    if (item.verdict.outcome === 'accepted') {
      acceptedCount += 1
      payoutOwedSats += item.budgetSats
      if (item.verdict.speedupRatio !== null) {
        acceptedSpeedups.push(item.verdict.speedupRatio)
      }
    } else {
      rejectedCount += 1
      refundedSats += item.budgetSats
      const reason = item.verdict.rejection?.reason ?? 'unknown'
      rejectionReasonCounts[reason] = (rejectionReasonCounts[reason] ?? 0) + 1
    }
  }

  const speedup =
    acceptedSpeedups.length === 0
      ? null
      : {
          max: Math.max(...acceptedSpeedups),
          mean:
            acceptedSpeedups.reduce((sum, value) => sum + value, 0) /
            acceptedSpeedups.length,
          min: Math.min(...acceptedSpeedups),
        }

  return {
    acceptedCount,
    campaignRef: ref,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    payoutOwedSats,
    refundedSats,
    rejectionReasonCounts,
    rejectedCount,
    speedup,
    totalJobs: items.length,
  }
}
