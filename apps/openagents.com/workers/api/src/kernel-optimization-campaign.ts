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

/**
 * A settled job tied to the specific dispatched job it settles.
 *
 * `requestedSlug` is the per-job identity the campaign already guarantees is
 * unique across the fan-out (see `buildKernelOptimizationCampaign`), so it is
 * the natural key for matching a settlement back to its dispatched request —
 * the parity verdict alone cannot do this (it carries the target model/device/
 * hardware but not the dispatched job's slug or op-level kernel ref).
 */
export type KernelOptimizationKeyedSettlementItem =
  KernelOptimizationSettlementItem &
    Readonly<{
      /** The dispatched request's `requestedSlug` this settlement settles. */
      requestedSlug: string
    }>

/** One per-job escrow disagreement between dispatch and settlement. */
export type KernelOptimizationBudgetMismatch = Readonly<{
  requestedSlug: string
  dispatchedSats: number
  settledSats: number
}>

/**
 * The result of matching each settlement item to its specific dispatched job.
 *
 * The totals-level `reconcileKernelOptimizationCampaignSettlement` confirms the
 * job COUNT and total escrow conserve, but it cannot tell WHICH job drifted, and
 * it is blind to offsetting errors that net to zero — e.g. job A settled twice
 * while job B is never settled (count and total escrow can both still match), or
 * one job overpaid by the exact amount another is underpaid. This per-job report
 * names the specific dispatched slugs that drifted.
 */
export type KernelOptimizationCampaignPerJobReconciliation = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  /** True iff every per-job check below passed — safe to release per job. */
  ok: boolean
  /** Dispatched slugs settled exactly once with the dispatched escrow. */
  matchedSlugs: ReadonlyArray<string>
  /** Dispatched slugs with no settlement item at all. */
  unsettledSlugs: ReadonlyArray<string>
  /** Settled slugs that no dispatched request in this campaign backs. */
  unexpectedSlugs: ReadonlyArray<string>
  /** Slugs that appear in more than one settlement item. */
  duplicateSlugs: ReadonlyArray<string>
  /** Matched slugs whose settled escrow differs from the dispatched escrow. */
  budgetMismatches: ReadonlyArray<KernelOptimizationBudgetMismatch>
  /** Human-readable reasons the reconciliation failed; empty iff `ok`. */
  discrepancies: ReadonlyArray<string>
}>

/**
 * The result of reconciling a dispatched campaign against its settlement ledger.
 *
 * At scale the dangerous failure modes are not bad verdicts (those are caught by
 * the parity verifier) but accounting drift: a dispatched job that never gets
 * settled, an extra settlement that no dispatched job backs, or escrow that does
 * not conserve (sats created or destroyed across the fan-out). This report makes
 * all three mechanically checkable before any payout/refund is released.
 */
export type KernelOptimizationCampaignReconciliation = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  /** True iff every check below passed — safe to release payout/refund. */
  ok: boolean
  /** Number of dispatch-valid requests the campaign fanned out. */
  dispatchedJobs: number
  /** Number of jobs the settlement ledger accounted for. */
  settledJobs: number
  jobCountReconciled: boolean
  /** Total escrow the campaign locked, in whole sats. */
  escrowSats: number
  /** payoutOwed + refunded from the settlement, in whole sats. */
  accountedSats: number
  /** True iff escrow conserves: nothing created or destroyed in settlement. */
  escrowConserved: boolean
  /** Human-readable reasons the reconciliation failed; empty iff `ok`. */
  discrepancies: ReadonlyArray<string>
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

/**
 * Reconcile a dispatched campaign against its settlement ledger.
 *
 * This is the at-scale safety net the two halves above cannot provide on their
 * own: `buildKernelOptimizationCampaign` produces the dispatch set and
 * `summarizeKernelOptimizationCampaignSettlement` reduces the verdicts, but
 * nothing yet asserts they describe the SAME campaign. Across a many-job mesh
 * run the dangerous drift is accounting, not correctness:
 *
 *   1. campaignRef drift — settling one campaign's verdicts against another's
 *      dispatch set.
 *   2. job-count drift — a dispatched job never settled, or an extra settlement
 *      no dispatched job backs.
 *   3. escrow drift — payout + refund != the escrow the campaign actually locked
 *      (sats created or destroyed in settlement).
 *
 * It moves no money; it returns a verdict-style report whose `ok` gate must hold
 * before the verified-work rail releases any payout or refund. It never throws:
 * a mismatch is a finding to surface (listed in `discrepancies`), not a
 * programming error.
 */
export const reconcileKernelOptimizationCampaignSettlement = (
  campaign: KernelOptimizationCampaign,
  settlement: KernelOptimizationCampaignSettlement,
): KernelOptimizationCampaignReconciliation => {
  const discrepancies: string[] = []

  if (campaign.campaignRef !== settlement.campaignRef) {
    discrepancies.push(
      `campaignRef mismatch: dispatched "${campaign.campaignRef}" vs settled "${settlement.campaignRef}"`,
    )
  }

  const dispatchedJobs = campaign.requests.length
  const settledJobs = settlement.totalJobs
  const jobCountReconciled = dispatchedJobs === settledJobs
  if (!jobCountReconciled) {
    discrepancies.push(
      `job-count drift: dispatched ${dispatchedJobs} job(s) but settled ${settledJobs}`,
    )
  }

  const escrowSats = campaign.totalBudgetSats
  const accountedSats = settlement.payoutOwedSats + settlement.refundedSats
  const escrowConserved = escrowSats === accountedSats
  if (!escrowConserved) {
    discrepancies.push(
      `escrow drift: locked ${escrowSats} sat(s) but settlement accounts for ${accountedSats} (payout ${settlement.payoutOwedSats} + refund ${settlement.refundedSats})`,
    )
  }

  return {
    accountedSats,
    campaignRef: campaign.campaignRef,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    discrepancies,
    dispatchedJobs,
    escrowConserved,
    escrowSats,
    jobCountReconciled,
    ok: discrepancies.length === 0,
    settledJobs,
  }
}

/**
 * Reconcile a dispatched campaign against its settlement ledger PER JOB.
 *
 * Where `reconcileKernelOptimizationCampaignSettlement` reconciles totals (job
 * count + escrow conservation), this matches each settlement item to the
 * specific dispatched request it settles, keyed on the unique `requestedSlug`
 * the campaign fan-out guarantees. That closes two gaps the totals check cannot:
 *
 *   1. It names WHICH job drifted — a dispatched job that was never settled
 *      (`unsettledSlugs`), or a settlement for a job this campaign never
 *      dispatched (`unexpectedSlugs`).
 *   2. It catches OFFSETTING errors the totals check is blind to — e.g. one job
 *      settled twice while another is never settled (`duplicateSlugs`; job count
 *      and total escrow can both still match), or one job overpaid by exactly
 *      what another is underpaid (`budgetMismatches`; the totals net to zero).
 *
 * Like the totals-level reconciler it moves no money and never throws: every
 * disagreement is a finding to surface, not a programming error. The `ok` gate
 * must hold before the verified-work rail releases any per-job payout/refund.
 */
export const reconcileKernelOptimizationCampaignPerJob = (
  campaign: KernelOptimizationCampaign,
  items: ReadonlyArray<KernelOptimizationKeyedSettlementItem>,
): KernelOptimizationCampaignPerJobReconciliation => {
  const discrepancies: string[] = []

  // Dispatched jobs, keyed by their unique requested slug -> escrow locked.
  const dispatchedBudgetBySlug = new Map<string, number>()
  for (const request of campaign.requests) {
    const slug = request.requestedSlug ?? ''
    dispatchedBudgetBySlug.set(slug, request.budgetSats)
  }

  // Tally settlement items per slug so duplicates are visible.
  const settledCountBySlug = new Map<string, number>()
  const settledBudgetBySlug = new Map<string, number>()
  for (const item of items) {
    const slug = item.requestedSlug.trim()
    settledCountBySlug.set(slug, (settledCountBySlug.get(slug) ?? 0) + 1)
    // Record the escrow only from the first sighting; duplicates are reported
    // separately and must not silently overwrite the matched amount.
    if (!settledBudgetBySlug.has(slug)) {
      settledBudgetBySlug.set(slug, item.budgetSats)
    }
  }

  const matchedSlugs: string[] = []
  const unsettledSlugs: string[] = []
  const budgetMismatches: KernelOptimizationBudgetMismatch[] = []

  for (const [slug, dispatchedSats] of dispatchedBudgetBySlug) {
    const settledCount = settledCountBySlug.get(slug) ?? 0
    if (settledCount === 0) {
      unsettledSlugs.push(slug)
      continue
    }
    // Duplicate handling is reported below; here only single settlements with a
    // matching escrow count as cleanly matched.
    const settledSats = settledBudgetBySlug.get(slug) ?? 0
    if (settledCount === 1 && settledSats === dispatchedSats) {
      matchedSlugs.push(slug)
    } else if (settledSats !== dispatchedSats) {
      budgetMismatches.push({ dispatchedSats, requestedSlug: slug, settledSats })
    }
  }

  const unexpectedSlugs: string[] = []
  const duplicateSlugs: string[] = []
  for (const [slug, count] of settledCountBySlug) {
    if (!dispatchedBudgetBySlug.has(slug)) {
      unexpectedSlugs.push(slug)
    }
    if (count > 1) {
      duplicateSlugs.push(slug)
    }
  }

  if (campaign.campaignRef.trim().length === 0) {
    discrepancies.push('campaignRef must be non-empty.')
  }
  if (unsettledSlugs.length > 0) {
    discrepancies.push(
      `dispatched job(s) never settled: ${unsettledSlugs.join(', ')}`,
    )
  }
  if (unexpectedSlugs.length > 0) {
    discrepancies.push(
      `settlement(s) for job(s) this campaign never dispatched: ${unexpectedSlugs.join(', ')}`,
    )
  }
  if (duplicateSlugs.length > 0) {
    discrepancies.push(
      `job(s) settled more than once: ${duplicateSlugs.join(', ')}`,
    )
  }
  for (const mismatch of budgetMismatches) {
    discrepancies.push(
      `per-job escrow drift on "${mismatch.requestedSlug}": dispatched ${mismatch.dispatchedSats} sat(s) but settled ${mismatch.settledSats}`,
    )
  }

  return {
    budgetMismatches,
    campaignRef: campaign.campaignRef,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    discrepancies,
    duplicateSlugs,
    matchedSlugs,
    ok: discrepancies.length === 0,
    unexpectedSlugs,
    unsettledSlugs,
  }
}
