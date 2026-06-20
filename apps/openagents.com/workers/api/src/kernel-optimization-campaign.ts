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

/** A target identity: the model + device + hardware a verdict/job names. */
export type KernelOptimizationTarget = Readonly<{
  targetModel: string
  device: string
  hardwareRef: string
}>

/**
 * One per-job disagreement between the target a settlement's parity verdict
 * names and the target the dispatched job (named by the same `requestedSlug`)
 * actually optimizes — a "target swap".
 */
export type KernelOptimizationTargetMismatch = Readonly<{
  requestedSlug: string
  dispatchedTarget: KernelOptimizationTarget
  settledVerdictTarget: KernelOptimizationTarget
}>

/**
 * One per-job disagreement between the OP a settlement's parity verdict claims
 * to have optimized and the op the dispatched job (named by the same
 * `requestedSlug`) actually targets — an "op swap". Distinct from a target
 * swap: two jobs can share the exact (model, device, hardware) target while
 * optimizing different ops, so the model/device/hardware target reconciler is
 * blind to this; only the op binds them apart.
 */
export type KernelOptimizationOpMismatch = Readonly<{
  requestedSlug: string
  dispatchedOpRef: string
  settledVerdictOpRef: string
}>

/**
 * One per-job disagreement between the NAMED-BASELINE tok/s the dispatched job
 * promised to beat and the baseline tok/s the settlement's parity verdict
 * actually measured its speedup against — a "baseline swap".
 *
 * The work definition requires the optimized kernel to beat a NAMED baseline on
 * the declared hardware. The parity verdict carries the baseline it was scored
 * against (`baselineTokensPerSecond`), but it does not know the named baseline
 * the job was dispatched with — the verifier computes the speedup against
 * whatever baseline record it was handed. So a settlement can carry a verdict
 * whose baseline is WEAKER than the dispatched named baseline (e.g. dispatched
 * to beat 328 tok/s, but the verdict measured a speedup against a cherry-picked
 * 200 tok/s baseline), manufacturing an "improvement" that never cleared the
 * named floor — while the (model/device/hardware) target and the op both still
 * match. This binds the verdict's baseline back to the dispatched named baseline.
 */
export type KernelOptimizationBaselineMismatch = Readonly<{
  requestedSlug: string
  dispatchedBaselineTokensPerSecond: number
  settledVerdictBaselineTokensPerSecond: number
}>

/**
 * The result of binding each settlement's parity verdict back to the SPECIFIC
 * dispatched job it claims to settle, by target.
 *
 * The slug-keyed `reconcileKernelOptimizationCampaignPerJob` confirms a
 * settlement is filed under a dispatched slug with the dispatched escrow, but it
 * never inspects the parity verdict the settlement carries: a verdict for a
 * DIFFERENT target (e.g. a verified accept on the cheap `qwen-3.5-0.5b` job)
 * could be filed under another job's slug with that job's exact budget and clear
 * both the totals and per-job reconcilers. This report closes that end-to-end
 * wiring gap — it checks the verdict's own `target` (model/device/hardware)
 * equals the target the dispatched job under that slug actually optimizes.
 */
export type KernelOptimizationCampaignTargetReconciliation = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  /** True iff every settled verdict's target matched its dispatched job. */
  ok: boolean
  /** Slugs whose settled verdict target matched the dispatched job's target. */
  matchedSlugs: ReadonlyArray<string>
  /** Slugs settled but not dispatched by this campaign (target unknowable). */
  unmatchedSlugs: ReadonlyArray<string>
  /** Slugs whose settled verdict optimizes a different target than dispatched. */
  targetMismatches: ReadonlyArray<KernelOptimizationTargetMismatch>
  /** Human-readable reasons the reconciliation failed; empty iff `ok`. */
  discrepancies: ReadonlyArray<string>
}>

/**
 * The result of binding each settlement's parity verdict back to the SPECIFIC
 * dispatched job it claims to settle, by OP.
 *
 * `reconcileKernelOptimizationCampaignTargets` binds by (model, device,
 * hardware), but a campaign can dispatch several ops for the SAME target — e.g.
 * `rmsnorm` and `attention.flash` both on `qwen-3.5-0.5b`/`cuda`/`a10g`. A
 * settlement filed under the `rmsnorm` job's slug, with that job's exact budget,
 * but carrying a verdict that optimized `attention.flash`, clears the per-job
 * (slug + escrow) AND the model/device/hardware target reconcilers: the
 * accounting conserves and the coarse target matches, yet the wrong op was paid.
 * This report checks the verdict's own `optimizedOpRef` equals the op the
 * dispatched job under that slug actually targets (`kernelRef`).
 */
export type KernelOptimizationCampaignOpReconciliation = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  /** True iff every settled verdict's op matched its dispatched job. */
  ok: boolean
  /** Slugs whose settled verdict op matched the dispatched job's op. */
  matchedSlugs: ReadonlyArray<string>
  /** Slugs settled but not dispatched by this campaign (op unknowable). */
  unmatchedSlugs: ReadonlyArray<string>
  /** Slugs whose settled verdict optimizes a different op than dispatched. */
  opMismatches: ReadonlyArray<KernelOptimizationOpMismatch>
  /** Human-readable reasons the reconciliation failed; empty iff `ok`. */
  discrepancies: ReadonlyArray<string>
}>

/**
 * The result of binding each settlement's parity verdict back to the SPECIFIC
 * dispatched job it claims to settle, by NAMED BASELINE.
 *
 * `reconcileKernelOptimizationCampaignTargets` and `...Ops` bind a verdict to
 * its job by (model, device, hardware) and by op, but neither checks WHICH
 * baseline the verdict actually measured its speedup against. The dispatched job
 * names a baseline tok/s the optimized kernel must beat; the verdict carries the
 * baseline it was scored against. A settlement filed under a job's slug, with
 * that job's exact budget, the right target, and the right op, but carrying a
 * verdict whose baseline is WEAKER than the dispatched named baseline, clears
 * the per-job, totals, target, AND op reconcilers — yet the "improvement" was
 * scored against a baseline the job never named, so the named floor was never
 * cleared. This report checks the verdict's own `baselineTokensPerSecond` equals
 * the named-baseline tok/s the dispatched job under that slug promised to beat.
 */
export type KernelOptimizationCampaignBaselineReconciliation = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  /** True iff every settled verdict scored against its dispatched baseline. */
  ok: boolean
  /** Slugs whose settled verdict baseline matched the dispatched named baseline. */
  matchedSlugs: ReadonlyArray<string>
  /** Slugs settled but not dispatched by this campaign (baseline unknowable). */
  unmatchedSlugs: ReadonlyArray<string>
  /** Slugs whose settled verdict scored against a different baseline tok/s. */
  baselineMismatches: ReadonlyArray<KernelOptimizationBaselineMismatch>
  /** Human-readable reasons the reconciliation failed; empty iff `ok`. */
  discrepancies: ReadonlyArray<string>
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

/** Normalize a target for comparison: trim + lowercase every component. */
const normalizeTarget = (
  target: KernelOptimizationTarget,
): KernelOptimizationTarget => ({
  device: target.device.trim().toLowerCase(),
  hardwareRef: target.hardwareRef.trim().toLowerCase(),
  targetModel: target.targetModel.trim().toLowerCase(),
})

const sameTarget = (
  a: KernelOptimizationTarget,
  b: KernelOptimizationTarget,
): boolean => {
  const na = normalizeTarget(a)
  const nb = normalizeTarget(b)
  return (
    na.targetModel === nb.targetModel &&
    na.device === nb.device &&
    na.hardwareRef === nb.hardwareRef
  )
}

/**
 * Bind each settlement's parity verdict back to the specific dispatched job it
 * claims to settle, BY TARGET.
 *
 * This is the end-to-end wiring the prior reconcilers explicitly deferred: the
 * slug-keyed and totals-level reconcilers trust the `requestedSlug` a settlement
 * is filed under, but never inspect the verdict that settlement carries. A
 * settlement filed under job A's slug with job A's exact budget but carrying a
 * parity verdict for target B (e.g. a verified accept on a different, cheaper
 * model) clears BOTH of them — the accounting conserves while the wrong work is
 * paid. This reconciler recomputes each dispatched job's requested slug from the
 * campaign spec (via the same dispatch encoder, so it cannot drift from the
 * actual slug) to learn that slug's true target, then checks the verdict's own
 * `target` (model/device/hardware) matches it.
 *
 * It takes the campaign SPEC (not the built campaign) because the dispatched
 * request bodies do not carry the structured target — only the spec does, and
 * the spec is what produced the slugs. It moves no money and never throws: a
 * disagreement is a finding to surface. The `ok` gate must hold before the
 * verified-work rail releases any payout/refund.
 */
export const reconcileKernelOptimizationCampaignTargets = (
  spec: KernelOptimizationCampaignSpec,
  items: ReadonlyArray<KernelOptimizationKeyedSettlementItem>,
): KernelOptimizationCampaignTargetReconciliation => {
  // Build the campaign once: this validates the spec (non-empty, unique targets,
  // unique slugs) and yields the slug per job in spec order.
  const campaign = buildKernelOptimizationCampaign(spec)

  // Map each unique requested slug to the structured target the dispatched job
  // under it actually optimizes. spec.jobs[i] <-> campaign.requests[i] by order.
  const dispatchedTargetBySlug = new Map<string, KernelOptimizationTarget>()
  spec.jobs.forEach((job, index) => {
    const slug = campaign.requests[index]?.requestedSlug ?? ''
    dispatchedTargetBySlug.set(slug, {
      device: job.device,
      hardwareRef: job.hardwareRef,
      targetModel: job.targetModel,
    })
  })

  const matchedSlugs: string[] = []
  const unmatchedSlugs: string[] = []
  const targetMismatches: KernelOptimizationTargetMismatch[] = []

  for (const item of items) {
    const slug = item.requestedSlug.trim()
    const dispatchedTarget = dispatchedTargetBySlug.get(slug)
    if (dispatchedTarget === undefined) {
      unmatchedSlugs.push(slug)
      continue
    }
    const settledVerdictTarget = item.verdict.target
    if (sameTarget(dispatchedTarget, settledVerdictTarget)) {
      matchedSlugs.push(slug)
    } else {
      targetMismatches.push({
        dispatchedTarget,
        requestedSlug: slug,
        settledVerdictTarget,
      })
    }
  }

  const discrepancies: string[] = []
  if (unmatchedSlugs.length > 0) {
    discrepancies.push(
      `settlement(s) for slug(s) this campaign never dispatched: ${unmatchedSlugs.join(', ')}`,
    )
  }
  for (const mismatch of targetMismatches) {
    const want = normalizeTarget(mismatch.dispatchedTarget)
    const got = normalizeTarget(mismatch.settledVerdictTarget)
    discrepancies.push(
      `target swap on "${mismatch.requestedSlug}": dispatched ${want.targetModel}/${want.device}/${want.hardwareRef} but settled verdict optimizes ${got.targetModel}/${got.device}/${got.hardwareRef}`,
    )
  }

  return {
    campaignRef: campaign.campaignRef,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    discrepancies,
    matchedSlugs,
    ok: discrepancies.length === 0,
    targetMismatches,
    unmatchedSlugs,
  }
}

/**
 * Bind each settlement's parity verdict back to the specific dispatched job it
 * claims to settle, BY OP.
 *
 * This closes the gap `reconcileKernelOptimizationCampaignTargets` explicitly
 * defers: that reconciler binds on (model, device, hardware), but a single
 * campaign can dispatch several ops against the SAME target (e.g. `rmsnorm` and
 * `attention.flash` both on `qwen-3.5-0.5b`/`cuda`/`a10g`). A settlement filed
 * under the `rmsnorm` job's slug, with that job's exact budget, carrying a
 * verdict that actually optimized `attention.flash`, clears the per-job
 * (slug + escrow) reconciler AND the coarse target reconciler — the accounting
 * conserves and the model/device/hardware match — yet the wrong op was paid.
 *
 * Like the target reconciler it recomputes each dispatched job's requested slug
 * from the campaign spec (via the same dispatch encoder, so it cannot drift from
 * the real slug) to learn that slug's true op (`kernelRef`), then checks the
 * settled verdict's own `optimizedOpRef` matches it (trim + case-normalized). A
 * blank verdict op never matches a dispatched op, so it is reported as a
 * mismatch. It takes the campaign SPEC (only the spec carries the structured op)
 * and the keyed settlement items; it moves no money and never throws. The `ok`
 * gate must hold before the verified-work rail releases any per-op payout/refund.
 */
export const reconcileKernelOptimizationCampaignOps = (
  spec: KernelOptimizationCampaignSpec,
  items: ReadonlyArray<KernelOptimizationKeyedSettlementItem>,
): KernelOptimizationCampaignOpReconciliation => {
  // Build the campaign once: validates the spec (non-empty, unique targets,
  // unique slugs) and yields the slug per job in spec order.
  const campaign = buildKernelOptimizationCampaign(spec)

  // Map each unique requested slug to the op the dispatched job under it targets.
  // spec.jobs[i] <-> campaign.requests[i] by order.
  const dispatchedOpBySlug = new Map<string, string>()
  spec.jobs.forEach((job, index) => {
    const slug = campaign.requests[index]?.requestedSlug ?? ''
    dispatchedOpBySlug.set(slug, job.kernelRef.trim().toLowerCase())
  })

  const matchedSlugs: string[] = []
  const unmatchedSlugs: string[] = []
  const opMismatches: KernelOptimizationOpMismatch[] = []

  for (const item of items) {
    const slug = item.requestedSlug.trim()
    const dispatchedOp = dispatchedOpBySlug.get(slug)
    if (dispatchedOp === undefined) {
      unmatchedSlugs.push(slug)
      continue
    }
    const settledOp = item.verdict.optimizedOpRef.trim().toLowerCase()
    if (settledOp.length > 0 && settledOp === dispatchedOp) {
      matchedSlugs.push(slug)
    } else {
      opMismatches.push({
        dispatchedOpRef: dispatchedOp,
        requestedSlug: slug,
        settledVerdictOpRef: item.verdict.optimizedOpRef.trim(),
      })
    }
  }

  const discrepancies: string[] = []
  if (unmatchedSlugs.length > 0) {
    discrepancies.push(
      `settlement(s) for slug(s) this campaign never dispatched: ${unmatchedSlugs.join(', ')}`,
    )
  }
  for (const mismatch of opMismatches) {
    const got =
      mismatch.settledVerdictOpRef.length === 0
        ? '(blank)'
        : mismatch.settledVerdictOpRef
    discrepancies.push(
      `op swap on "${mismatch.requestedSlug}": dispatched op "${mismatch.dispatchedOpRef}" but settled verdict optimizes "${got}"`,
    )
  }

  return {
    campaignRef: campaign.campaignRef,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    discrepancies,
    matchedSlugs,
    ok: discrepancies.length === 0,
    opMismatches,
    unmatchedSlugs,
  }
}

/**
 * Bind each settlement's parity verdict back to the specific dispatched job it
 * claims to settle, BY NAMED BASELINE.
 *
 * This closes the gap the target and op reconcilers leave open: both bind a
 * verdict to its dispatched job by identity (model/device/hardware, then op),
 * but neither checks the baseline the speedup was actually measured against. The
 * work definition requires the optimized kernel to beat a NAMED baseline tok/s
 * on declared hardware. The parity verifier scores the speedup against whatever
 * baseline record it is handed and does not know the named baseline the job was
 * dispatched with, so a settlement can carry a verdict scored against a WEAKER
 * baseline than the dispatched one (e.g. dispatched to beat 328 tok/s but the
 * verdict measured against a cherry-picked 200 tok/s baseline). That settlement
 * clears the per-job, totals, target, AND op reconcilers — same slug, escrow,
 * target, and op — yet the named throughput floor the job promised was never
 * cleared.
 *
 * Like the target/op reconcilers it recomputes each dispatched job's requested
 * slug from the campaign spec (via the same dispatch encoder, so it cannot drift
 * from the real slug) to learn that slug's named baseline tok/s, then checks the
 * settled verdict's own `baselineTokensPerSecond` equals it exactly (the named
 * baseline is a fixed number the job carried, not a fresh measurement, so any
 * deviation is a swap). It takes the campaign SPEC (only the spec carries the
 * named baseline) and the keyed settlement items; it moves no money and never
 * throws. The `ok` gate must hold before the verified-work rail releases any
 * payout/refund.
 */
export const reconcileKernelOptimizationCampaignBaselines = (
  spec: KernelOptimizationCampaignSpec,
  items: ReadonlyArray<KernelOptimizationKeyedSettlementItem>,
): KernelOptimizationCampaignBaselineReconciliation => {
  // Build the campaign once: validates the spec (non-empty, unique targets,
  // unique slugs) and yields the slug per job in spec order.
  const campaign = buildKernelOptimizationCampaign(spec)

  // Map each unique requested slug to the named baseline tok/s the dispatched
  // job under it promised to beat. spec.jobs[i] <-> campaign.requests[i] by order.
  const dispatchedBaselineBySlug = new Map<string, number>()
  spec.jobs.forEach((job, index) => {
    const slug = campaign.requests[index]?.requestedSlug ?? ''
    dispatchedBaselineBySlug.set(slug, job.baselineTokensPerSecond)
  })

  const matchedSlugs: string[] = []
  const unmatchedSlugs: string[] = []
  const baselineMismatches: KernelOptimizationBaselineMismatch[] = []

  for (const item of items) {
    const slug = item.requestedSlug.trim()
    const dispatchedBaseline = dispatchedBaselineBySlug.get(slug)
    if (dispatchedBaseline === undefined) {
      unmatchedSlugs.push(slug)
      continue
    }
    const settledBaseline = item.verdict.baselineTokensPerSecond
    if (settledBaseline === dispatchedBaseline) {
      matchedSlugs.push(slug)
    } else {
      baselineMismatches.push({
        dispatchedBaselineTokensPerSecond: dispatchedBaseline,
        requestedSlug: slug,
        settledVerdictBaselineTokensPerSecond: settledBaseline,
      })
    }
  }

  const discrepancies: string[] = []
  if (unmatchedSlugs.length > 0) {
    discrepancies.push(
      `settlement(s) for slug(s) this campaign never dispatched: ${unmatchedSlugs.join(', ')}`,
    )
  }
  for (const mismatch of baselineMismatches) {
    discrepancies.push(
      `baseline swap on "${mismatch.requestedSlug}": dispatched named baseline ${mismatch.dispatchedBaselineTokensPerSecond} tok/s but settled verdict scored its speedup against ${mismatch.settledVerdictBaselineTokensPerSecond} tok/s`,
    )
  }

  return {
    baselineMismatches,
    campaignRef: campaign.campaignRef,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    discrepancies,
    matchedSlugs,
    ok: discrepancies.length === 0,
    unmatchedSlugs,
  }
}

/**
 * The single all-gates-must-hold verdict for releasing an at-scale campaign's
 * payouts and refunds.
 *
 * Each of the five reconcilers above guards a DIFFERENT at-scale failure mode,
 * and none subsumes another:
 *
 *   - totals (`reconcileKernelOptimizationCampaignSettlement`): job count + total
 *     escrow conserve, and the settlement is for THIS campaign.
 *   - per-job (`reconcileKernelOptimizationCampaignPerJob`): every dispatched
 *     slug settled exactly once with its dispatched escrow (catches offsetting
 *     drift the totals are blind to).
 *   - target (`reconcileKernelOptimizationCampaignTargets`): each settled
 *     verdict's model/device/hardware matches the slug it is filed under.
 *   - op (`reconcileKernelOptimizationCampaignOps`): each settled verdict's op
 *     matches the slug it is filed under (catches same-target op swaps).
 *   - baseline (`reconcileKernelOptimizationCampaignBaselines`): each settled
 *     verdict scored its speedup against the named baseline the slug was
 *     dispatched with (catches a speedup measured against a weaker baseline than
 *     the named floor, which target + op + per-job + totals are all blind to).
 *
 * Releasing money safely requires ALL FIVE to hold. Until now a caller had to
 * remember to run each one and AND the results by hand; forgetting any single
 * gate (most easily the op or baseline reconciler, which only bite in
 * many-ops-per-target or cherry-picked-baseline runs) silently re-opens exactly
 * the drift that gate exists to catch. This gate makes the "safe to release"
 * decision atomic and auditable: it runs all five against ONE (spec, keyed
 * settlement) pair, derives the campaign and settlement once, and is `ok` iff
 * every constituent report is `ok`. It also surfaces the settlement ledger so
 * payout/refund totals are visible alongside the verdict.
 *
 * It moves no money and never throws on a reconciliation finding (those are
 * reported); it only re-raises a `KernelOptimizationDispatchError` when the spec
 * itself is structurally invalid (the same contract the campaign builder
 * enforces), because an unbuildable campaign has nothing to release.
 */
export type KernelOptimizationCampaignReleaseGate = Readonly<{
  classId: typeof KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID
  campaignRef: string
  /** True iff EVERY constituent reconciler is ok — safe to release at scale. */
  ok: boolean
  /** The settlement ledger the gate evaluated (payout/refund totals). */
  settlement: KernelOptimizationCampaignSettlement
  /** Totals-level (job count + escrow conservation) reconciliation. */
  totals: KernelOptimizationCampaignReconciliation
  /** Per-job (slug + escrow) reconciliation. */
  perJob: KernelOptimizationCampaignPerJobReconciliation
  /** Per-job verdict-target (model/device/hardware) reconciliation. */
  targets: KernelOptimizationCampaignTargetReconciliation
  /** Per-job verdict-op reconciliation. */
  ops: KernelOptimizationCampaignOpReconciliation
  /** Per-job verdict-named-baseline reconciliation. */
  baselines: KernelOptimizationCampaignBaselineReconciliation
  /**
   * Every constituent discrepancy, each prefixed with the gate that raised it,
   * so a single read tells an operator exactly which gate(s) failed and why.
   */
  discrepancies: ReadonlyArray<string>
}>

/**
 * Compose all four campaign reconcilers (plus the settlement ledger) into one
 * release gate whose `ok` must hold before any at-scale payout or refund.
 *
 * Pass the campaign SPEC (the target/op reconcilers need the structured targets
 * only the spec carries) and the slug-keyed settlement items (budget + parity
 * verdict per dispatched slug). The campaign fan-out and the settlement summary
 * are derived internally so the four gates provably evaluate the SAME campaign
 * and the SAME settlement — a caller cannot accidentally reconcile mismatched
 * pairs.
 */
export const evaluateKernelOptimizationCampaignRelease = (
  spec: KernelOptimizationCampaignSpec,
  items: ReadonlyArray<KernelOptimizationKeyedSettlementItem>,
): KernelOptimizationCampaignReleaseGate => {
  // Build once: validates the spec (non-empty, unique targets, unique slugs) and
  // yields the dispatch set every gate below reconciles against. An unbuildable
  // spec throws here — there is nothing to release.
  const campaign = buildKernelOptimizationCampaign(spec)

  // The keyed items are a superset of plain settlement items, so they summarize
  // directly; this is the SAME ledger the totals gate reconciles against.
  const settlement = summarizeKernelOptimizationCampaignSettlement(
    campaign.campaignRef,
    items,
  )

  const totals = reconcileKernelOptimizationCampaignSettlement(
    campaign,
    settlement,
  )
  const perJob = reconcileKernelOptimizationCampaignPerJob(campaign, items)
  const targets = reconcileKernelOptimizationCampaignTargets(spec, items)
  const ops = reconcileKernelOptimizationCampaignOps(spec, items)
  const baselines = reconcileKernelOptimizationCampaignBaselines(spec, items)

  const discrepancies: string[] = [
    ...totals.discrepancies.map((d) => `totals: ${d}`),
    ...perJob.discrepancies.map((d) => `per-job: ${d}`),
    ...targets.discrepancies.map((d) => `target: ${d}`),
    ...ops.discrepancies.map((d) => `op: ${d}`),
    ...baselines.discrepancies.map((d) => `baseline: ${d}`),
  ]

  return {
    baselines,
    campaignRef: campaign.campaignRef,
    classId: KERNEL_OPTIMIZATION_CAMPAIGN_CLASS_ID,
    discrepancies,
    ok: totals.ok && perJob.ok && targets.ok && ops.ok && baselines.ok,
    ops,
    perJob,
    settlement,
    targets,
    totals,
  }
}
