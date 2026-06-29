// Khala QUANTIZATION EVAL GATE (book P1-7 / #6090).
//
// THE GATE (book Ch.5, in our own words):
// ---------------------------------------
// Quantization is only allowed to claim the public model if it is PROVEN against
// product evals — never assumed safe from a throughput number. The gate scores a
// quantized lane against the ORIGINAL precision on EXECUTED checks (reusing the
// P0-4 executed Khala-code verifier — `AcceptanceVerdict` — not a regex over
// source) and computes the cost-per-accepted-outcome delta. The verdict:
//
//   PASS the quantized lane ONLY when accepted-outcome quality HOLDS (within an
//   agreed bound), OR — if quality drops slightly — when cost-per-accepted-outcome
//   IMPROVES enough to be a net win. A throughput/cost win that lowers the
//   accepted-outcome rate without improving cost-per-accepted-outcome is a LOSS.
//
// REUSE, DO NOT FORK:
//   - the executed-acceptance verdict shape (`AcceptanceVerdict` from the P0-4
//     verifier) is the per-sample quality signal — an EXECUTED pass/fail, never a
//     source heuristic;
//   - the benchmark report's cost-per-accepted-outcome math (P1-5) is the cost
//     metric — we aggregate the same way (total cost basis / accepted outcomes,
//     null when zero accepted, never a fake 0);
//   - the benchmark runner could drive a real quantized vs original sweep; THAT
//     real sweep is FLAG/OWNER/COMPUTE-GATED (see `RealQuantSweepNotArmedError`).
//     By default the gate scores DETERMINISTIC fixture comparison sets — no real
//     quantized serving, no spend.
//
// HONESTY: a fixture comparison is labeled `decisionGrade:false`. A real,
// production-promoting gate decision requires the owner-armed real sweep over
// realistic traffic. The default path proves the GATE LOGIC, not a real lane.
//
// PURE: no Worker, no clock, no randomness, no IO. Same inputs => same verdict.
import type { AcceptanceVerdict } from './acceptance-runner/verdict'
import type {
  KhalaPrecisionMode,
  KhalaQuantizationBackend,
  KhalaQuantizationScope,
} from './khala-quantization'
import {
  isAggressiveScope,
  isQuantizedPrecision,
} from './khala-quantization'

// ---------------------------------------------------------------------------
// A single comparison sample: the SAME task scored on both precisions.
// ---------------------------------------------------------------------------

// One executed comparison: a single Khala-code task run on BOTH the original
// precision and the candidate quantized precision, each producing an EXECUTED
// acceptance verdict + a measured cost basis. Pairing per-task means the gate
// compares like-for-like (same prompt, same acceptance suite) — the book's
// "change one variable" discipline applied to precision.
export type QuantizationComparisonSample = Readonly<{
  // A stable id for the task being compared (e.g. an artifact-gen scenario id).
  taskId: string
  // The EXECUTED acceptance verdict on the ORIGINAL (full) precision. `executed`
  // is always true on this shape — it is only produced after a real headless run
  // (or a deterministic fixture verdict for the default gate path).
  originalVerdict: AcceptanceVerdict
  // The EXECUTED acceptance verdict on the CANDIDATE quantized precision.
  quantizedVerdict: AcceptanceVerdict
  // Cost basis in msat for the original-precision run (P1-5 cost metric).
  originalCostBasisMsat: number
  // Cost basis in msat for the quantized run (typically lower — the throughput
  // win — which is exactly why quality must be proven to hold).
  quantizedCostBasisMsat: number
}>

// The owner-armed evidence bundle required before a passing gate may be called
// DECISION GRADE. This is still pure data: the real serving, acceptance runner,
// and storage of dereferenceable evidence happen outside this module.
export type RealQuantSweepEvidence = Readonly<{
  schemaVersion: 'openagents.khala.real-quant-sweep-evidence.v1'
  // Dereferenceable, public-safe closeout for this sweep. This is the ref the
  // quantization metadata may later store as `evalGateRef`.
  evidenceRef: string
  // Explicit owner approval/cap ref for running real original+quantized compute.
  ownerApprovalRef: string
  // The realistic traffic/workload set used for the comparison.
  workloadRef: string
  originalModelId: string
  quantizedModelId: string
  // Original should be full precision; quantized must be a reduced precision.
  originalPrecision: KhalaPrecisionMode
  quantizedPrecision: KhalaPrecisionMode
  quantizationBackend: KhalaQuantizationBackend
  quantizationBackendVersion: string
  // Must match the comparison samples fed to the gate.
  sampleCount: number
  // Evidence refs for the executed verifier, latency, and cost basis. All must
  // be public-safe refs, not raw prompts, secrets, or private traces.
  acceptanceVerifierRef: string
  latencyEvidenceRef: string
  costEvidenceRef: string
  publicSafeEvidenceRefs: ReadonlyArray<string>
}>

export type QuantizationDecisionGradeBlocker =
  | 'real_sweep_evidence_missing'
  | 'evidence_ref_missing'
  | 'owner_approval_ref_missing'
  | 'workload_ref_missing'
  | 'model_context_missing'
  | 'original_precision_not_full'
  | 'quantized_precision_not_reduced'
  | 'same_precision_compared'
  | 'acceptance_verifier_ref_missing'
  | 'latency_evidence_ref_missing'
  | 'cost_evidence_ref_missing'
  | 'public_safe_evidence_refs_missing'
  | 'sample_count_mismatch'

const nonBlank = (value: string): boolean => value.trim().length > 0

export const decisionGradeBlockersForRealQuantSweepEvidence = (input: {
  evidence?: RealQuantSweepEvidence | undefined
  sampleCount: number
}): ReadonlyArray<QuantizationDecisionGradeBlocker> => {
  const { evidence, sampleCount } = input
  if (evidence === undefined) {
    return ['real_sweep_evidence_missing']
  }

  const blockers: Array<QuantizationDecisionGradeBlocker> = []
  if (!nonBlank(evidence.evidenceRef)) blockers.push('evidence_ref_missing')
  if (!nonBlank(evidence.ownerApprovalRef)) {
    blockers.push('owner_approval_ref_missing')
  }
  if (!nonBlank(evidence.workloadRef)) blockers.push('workload_ref_missing')
  if (
    !nonBlank(evidence.originalModelId) ||
    !nonBlank(evidence.quantizedModelId)
  ) {
    blockers.push('model_context_missing')
  }
  if (evidence.originalPrecision !== 'unquantized') {
    blockers.push('original_precision_not_full')
  }
  if (!isQuantizedPrecision(evidence.quantizedPrecision)) {
    blockers.push('quantized_precision_not_reduced')
  }
  if (evidence.originalPrecision === evidence.quantizedPrecision) {
    blockers.push('same_precision_compared')
  }
  if (!nonBlank(evidence.acceptanceVerifierRef)) {
    blockers.push('acceptance_verifier_ref_missing')
  }
  if (!nonBlank(evidence.latencyEvidenceRef)) {
    blockers.push('latency_evidence_ref_missing')
  }
  if (!nonBlank(evidence.costEvidenceRef)) {
    blockers.push('cost_evidence_ref_missing')
  }
  if (
    evidence.publicSafeEvidenceRefs.length === 0 ||
    evidence.publicSafeEvidenceRefs.some(ref => !nonBlank(ref))
  ) {
    blockers.push('public_safe_evidence_refs_missing')
  }
  if (evidence.sampleCount !== sampleCount || evidence.sampleCount <= 0) {
    blockers.push('sample_count_mismatch')
  }
  return blockers
}

// ---------------------------------------------------------------------------
// The gate policy (the agreed bounds).
// ---------------------------------------------------------------------------

// The thresholds the gate decision uses. Defaults are conservative: a quantized
// lane must hold accepted-outcome rate within a small absolute drop, and any
// allowed drop must be paid for by a cost-per-accepted-outcome improvement.
export type QuantizationGatePolicy = Readonly<{
  // The maximum ABSOLUTE accepted-outcome-rate drop tolerated when the rate drops
  // (e.g. 0.02 = 2 percentage points). A drop strictly within this AND offset by
  // a cost-per-accepted-outcome win can still pass; a larger drop never passes.
  maxAcceptedRateDropAbs: number
  // The minimum FRACTIONAL cost-per-accepted-outcome improvement required to
  // "buy back" an accepted-rate drop (e.g. 0.15 = the quantized lane must be at
  // least 15% cheaper per accepted outcome). Only consulted when the rate dropped.
  minCostPerAcceptedImprovementFrac: number
  // Whether an AGGRESSIVE quantization scope (KV-cache / attention) requires an
  // explicit owner acknowledgement to pass even when the metrics hold (book
  // policy: weights-only / FP8 before aggressive KV/attention quant). Default true.
  requireAckForAggressiveScope: boolean
}>

export const DEFAULT_QUANT_GATE_POLICY: QuantizationGatePolicy = {
  maxAcceptedRateDropAbs: 0.02,
  minCostPerAcceptedImprovementFrac: 0.15,
  requireAckForAggressiveScope: true,
}

// ---------------------------------------------------------------------------
// The gate decision result.
// ---------------------------------------------------------------------------

// Why the gate reached its decision (stable, neutral refs).
export type QuantizationGateReason =
  // Accepted-outcome rate held within the agreed bound — clean pass.
  | 'accepted_rate_held'
  // Accepted rate dropped but cost-per-accepted-outcome improved enough — net win.
  | 'cost_per_accepted_improved_offsets_drop'
  // Accepted rate dropped beyond the bound — quality loss, FAIL.
  | 'accepted_rate_dropped_beyond_bound'
  // Accepted rate dropped within the bound but cost-per-accepted did not improve
  // enough to pay for it — not a net win, FAIL.
  | 'accepted_rate_dropped_without_cost_win'
  // An aggressive scope (KV-cache / attention) lacked the required owner ack.
  | 'aggressive_scope_requires_ack'
  // No accepted outcomes on the ORIGINAL precision — there is nothing to hold; the
  // comparison is undefined and cannot pass.
  | 'no_baseline_accepted_outcomes'
  // No comparison samples at all.
  | 'no_comparison_samples'

export type QuantizationGateResult = Readonly<{
  schemaVersion: 'openagents.khala.quant-eval-gate.v1'
  // The headline: may the quantized lane be promoted to claim the public model?
  passed: boolean
  reason: QuantizationGateReason
  detail: string
  // How many task comparisons fed the decision.
  sampleCount: number
  // Accepted-outcome rate on each precision (accepted / attempted EXECUTED
  // checks). null when no executed attempts existed on that side.
  originalAcceptedRate: number | null
  quantizedAcceptedRate: number | null
  // The ABSOLUTE accepted-rate delta (quantized − original). Negative = a drop.
  // null when either rate is null.
  acceptedRateDeltaAbs: number | null
  // Cost-per-accepted-outcome (msat) on each precision (P1-5 metric). null when a
  // side had zero accepted outcomes (dividing would fabricate a number).
  originalCostPerAcceptedMsat: number | null
  quantizedCostPerAcceptedMsat: number | null
  // The FRACTIONAL cost-per-accepted improvement (positive = quantized cheaper
  // per accepted outcome). null when either side is null.
  costPerAcceptedImprovementFrac: number | null
  // Whether the candidate scope is aggressive (KV-cache / attention).
  aggressiveScope: boolean
  // Whether this is a decision-grade gate (an owner-armed real sweep over real
  // traffic) vs the default fixture gate (proves the gate logic only).
  decisionGrade: boolean
  // The dereferenceable real-sweep evidence ref backing a decision-grade result.
  // Null for fixtures, unarmed sweeps, or invalid evidence.
  realSweepEvidenceRef: string | null
  // Why a requested decision-grade result was downgraded to fixture/logical
  // evidence. Empty when decisionGrade is true or decision-grade was not asked.
  decisionGradeBlockers: ReadonlyArray<QuantizationDecisionGradeBlocker>
}>

// ---------------------------------------------------------------------------
// Aggregation helpers (reuse the P1-5 accepted-outcome + cost math shape).
// ---------------------------------------------------------------------------

// Count accepted outcomes (verified executed verdicts) and attempts across one
// side of the comparison. An `AcceptanceVerdict` is EXECUTED by construction; we
// count one ATTEMPT per verdict and one ACCEPTED per `verified` verdict.
const tallySide = (
  verdicts: ReadonlyArray<AcceptanceVerdict>,
  costs: ReadonlyArray<number>,
): {
  attempted: number
  accepted: number
  totalCostMsat: number
} => {
  let attempted = 0
  let accepted = 0
  for (const verdict of verdicts) {
    attempted += 1
    if (verdict.verified) {
      accepted += 1
    }
  }
  const totalCostMsat = costs.reduce((sum, cost) => sum + cost, 0)
  return { attempted, accepted, totalCostMsat }
}

// Accepted-outcome RATE: accepted / attempted. null when nothing was attempted
// (the honest absence, never a fabricated 0).
const acceptedRate = (accepted: number, attempted: number): number | null =>
  attempted === 0 ? null : accepted / attempted

// Cost-per-accepted-outcome (msat): total cost / accepted. null when zero
// accepted — undefined cost-per-outcome is itself a finding, NOT a 0 (this is the
// exact rule the P1-5 report follows).
const costPerAccepted = (
  totalCostMsat: number,
  accepted: number,
): number | null => (accepted === 0 ? null : totalCostMsat / accepted)

// ---------------------------------------------------------------------------
// The gate.
// ---------------------------------------------------------------------------

export type QuantizationGateInput = Readonly<{
  // The per-task paired comparisons (original vs quantized, executed).
  samples: ReadonlyArray<QuantizationComparisonSample>
  // The candidate quantization scope (drives the aggressive-scope policy).
  scope: KhalaQuantizationScope
  // Owner acknowledgement of an aggressive scope (KV-cache / attention). Required
  // to pass an aggressive-scope candidate when the policy demands it.
  aggressiveScopeAck?: boolean | undefined
  // Whether this result is decision-grade (an owner-armed real sweep). Default
  // false — the fixture gate proves the LOGIC, not a real lane.
  decisionGrade?: boolean | undefined
  // Required when `decisionGrade:true`. Without this structured real-sweep
  // closeout, the gate may pass logically but is downgraded to
  // `decisionGrade:false`.
  realSweepEvidence?: RealQuantSweepEvidence | undefined
  // The policy bounds; defaults to the conservative `DEFAULT_QUANT_GATE_POLICY`.
  policy?: QuantizationGatePolicy | undefined
}>

// Run the quantization eval gate over a comparison set. PURE: same input =>
// same result. The decision encodes the book's principle exactly: a quantized
// lane passes only when accepted-outcome quality HOLDS, or a quality drop is
// strictly bought back by a cost-per-accepted-outcome improvement.
export const runQuantizationEvalGate = (
  input: QuantizationGateInput,
): QuantizationGateResult => {
  const policy = input.policy ?? DEFAULT_QUANT_GATE_POLICY
  const requestedDecisionGrade = input.decisionGrade ?? false
  const decisionGradeBlockers = requestedDecisionGrade
    ? decisionGradeBlockersForRealQuantSweepEvidence({
        evidence: input.realSweepEvidence,
        sampleCount: input.samples.length,
      })
    : []
  const decisionGrade =
    requestedDecisionGrade && decisionGradeBlockers.length === 0
  const aggressiveScope = isAggressiveScope(input.scope)

  const base = {
    schemaVersion: 'openagents.khala.quant-eval-gate.v1' as const,
    sampleCount: input.samples.length,
    aggressiveScope,
    decisionGrade,
    realSweepEvidenceRef: decisionGrade
      ? (input.realSweepEvidence?.evidenceRef ?? null)
      : null,
    decisionGradeBlockers,
  }

  if (input.samples.length === 0) {
    return {
      ...base,
      passed: false,
      reason: 'no_comparison_samples',
      detail:
        'No quantized-vs-original comparison samples were provided; the gate cannot prove the quantized lane and fails closed.',
      originalAcceptedRate: null,
      quantizedAcceptedRate: null,
      acceptedRateDeltaAbs: null,
      originalCostPerAcceptedMsat: null,
      quantizedCostPerAcceptedMsat: null,
      costPerAcceptedImprovementFrac: null,
    }
  }

  const originalSide = tallySide(
    input.samples.map(s => s.originalVerdict),
    input.samples.map(s => s.originalCostBasisMsat),
  )
  const quantizedSide = tallySide(
    input.samples.map(s => s.quantizedVerdict),
    input.samples.map(s => s.quantizedCostBasisMsat),
  )

  const originalAcceptedRate = acceptedRate(
    originalSide.accepted,
    originalSide.attempted,
  )
  const quantizedAcceptedRate = acceptedRate(
    quantizedSide.accepted,
    quantizedSide.attempted,
  )
  const originalCostPerAcceptedMsat = costPerAccepted(
    originalSide.totalCostMsat,
    originalSide.accepted,
  )
  const quantizedCostPerAcceptedMsat = costPerAccepted(
    quantizedSide.totalCostMsat,
    quantizedSide.accepted,
  )

  const acceptedRateDeltaAbs =
    originalAcceptedRate === null || quantizedAcceptedRate === null
      ? null
      : quantizedAcceptedRate - originalAcceptedRate

  // Fractional cost-per-accepted improvement: positive = quantized cheaper per
  // accepted outcome. (original − quantized) / original. null when either is null.
  const costPerAcceptedImprovementFrac =
    originalCostPerAcceptedMsat === null ||
    quantizedCostPerAcceptedMsat === null ||
    originalCostPerAcceptedMsat === 0
      ? null
      : (originalCostPerAcceptedMsat - quantizedCostPerAcceptedMsat) /
        originalCostPerAcceptedMsat

  const metrics = {
    originalAcceptedRate,
    quantizedAcceptedRate,
    acceptedRateDeltaAbs,
    originalCostPerAcceptedMsat,
    quantizedCostPerAcceptedMsat,
    costPerAcceptedImprovementFrac,
  }

  // No baseline accepted outcomes: there is no quality bar to hold against, so
  // the comparison is undefined and cannot pass.
  if (originalAcceptedRate === null || originalSide.accepted === 0) {
    return {
      ...base,
      ...metrics,
      passed: false,
      reason: 'no_baseline_accepted_outcomes',
      detail:
        'The original precision produced no accepted outcomes, so there is no quality baseline to hold against. The gate cannot prove the quantized lane and fails closed.',
    }
  }

  // Aggressive scope without the required ack fails closed regardless of metrics
  // (book policy: weights-only / FP8 before aggressive KV/attention quant).
  if (
    aggressiveScope &&
    policy.requireAckForAggressiveScope &&
    input.aggressiveScopeAck !== true
  ) {
    return {
      ...base,
      ...metrics,
      passed: false,
      reason: 'aggressive_scope_requires_ack',
      detail:
        'Candidate uses an aggressive quantization scope (KV-cache / attention). Policy requires explicit owner acknowledgement before such a lane may be promoted; absent the ack the gate fails closed. Prefer weights-only / FP8 first.',
    }
  }

  // The quantized rate should never be null here (samples exist + attempts > 0),
  // but guard defensively: a null quantized rate cannot demonstrate held quality.
  if (quantizedAcceptedRate === null || acceptedRateDeltaAbs === null) {
    return {
      ...base,
      ...metrics,
      passed: false,
      reason: 'accepted_rate_dropped_beyond_bound',
      detail:
        'The quantized lane produced no measurable accepted-outcome rate to compare against the baseline; it cannot demonstrate held quality and fails closed.',
    }
  }

  // Quality HELD: the accepted-outcome rate did not drop (delta >= 0) or dropped
  // by a negligible amount below the tolerance floor. Clean pass.
  if (acceptedRateDeltaAbs >= 0) {
    return {
      ...base,
      ...metrics,
      passed: true,
      reason: 'accepted_rate_held',
      detail:
        'The quantized lane held (or improved) the accepted-outcome rate vs the original precision; quality is preserved and the lane may be promoted.',
    }
  }

  // From here the accepted-outcome rate DROPPED (delta < 0).
  const dropAbs = -acceptedRateDeltaAbs

  // A drop beyond the absolute bound is a quality LOSS no cost win can buy back.
  if (dropAbs > policy.maxAcceptedRateDropAbs) {
    return {
      ...base,
      ...metrics,
      passed: false,
      reason: 'accepted_rate_dropped_beyond_bound',
      detail:
        'The quantized lane dropped the accepted-outcome rate beyond the agreed bound. A throughput/cost win cannot offset a quality loss this large; the gate fails. This is the book’s rule: a faster lane that drops accepted outcomes is a loss.',
    }
  }

  // A drop WITHIN the bound is allowed only if cost-per-accepted-outcome improved
  // enough to be a net win (the book: a quality drop is acceptable only if
  // cost-per-accepted-outcome improves).
  if (
    costPerAcceptedImprovementFrac !== null &&
    costPerAcceptedImprovementFrac >= policy.minCostPerAcceptedImprovementFrac
  ) {
    return {
      ...base,
      ...metrics,
      passed: true,
      reason: 'cost_per_accepted_improved_offsets_drop',
      detail:
        'The quantized lane dropped the accepted-outcome rate slightly (within the agreed bound) but improved cost-per-accepted-outcome by enough to be a net win; the lane may be promoted.',
    }
  }

  // A drop within the bound but no sufficient cost win is NOT a net win.
  return {
    ...base,
    ...metrics,
    passed: false,
    reason: 'accepted_rate_dropped_without_cost_win',
    detail:
      'The quantized lane dropped the accepted-outcome rate without a sufficient cost-per-accepted-outcome improvement to offset it; it is not a net win and the gate fails.',
  }
}

// ---------------------------------------------------------------------------
// Real quantized-vs-original sweep — FLAG/OWNER/COMPUTE-GATED, default OFF.
// ---------------------------------------------------------------------------

// Raised when a real quantized-vs-original sweep is requested without owner
// arming. A typed refusal (not a silent fallback) so a test or un-armed
// environment can NEVER stand up real quantized serving or spend compute. The
// gate LOGIC above runs on deterministic fixture comparison sets by default; the
// real sweep that actually serves a quantized model and runs the executed
// verifier on both precisions is owner/compute-gated.
export class RealQuantSweepNotArmedError extends Error {
  readonly _tag = 'RealQuantSweepNotArmedError'
  constructor() {
    super(
      'Real quantized-vs-original eval sweep is owner/compute-gated and not armed. ' +
        'It stands up real quantized serving and runs the executed verifier on both ' +
        'precisions (spend + compute). Set armRealQuantSweep:true (owner-confirmed) to run it.',
    )
    this.name = 'RealQuantSweepNotArmedError'
  }
}

// The injected executor a real sweep would call to produce the executed
// comparison samples from REAL quantized + original serving. This module does NOT
// implement it (the live serving + executed verifier runs live in the owner-armed
// sweep + the out-of-Worker acceptance runner); it types the gate so the default
// path is provably serving-free and spend-free.
export type RealQuantSweepExecutor = () => ReadonlyArray<QuantizationComparisonSample>

export type RealQuantSweepOptions = Readonly<{
  // MUST be exactly true to run a real sweep. Anything else refuses.
  armRealQuantSweep: boolean
  executor?: RealQuantSweepExecutor | undefined
}>

// Produce the comparison samples for a real sweep, or throw the typed refusal.
// When armed (and given an executor) it delegates; unarmed it throws so no real
// quantized serving / spend can ever happen on the default path.
export const collectRealQuantSweepSamples = (
  options: RealQuantSweepOptions,
): ReadonlyArray<QuantizationComparisonSample> => {
  const armed =
    options.armRealQuantSweep === true && options.executor !== undefined
  if (!armed || options.executor === undefined) {
    throw new RealQuantSweepNotArmedError()
  }
  return options.executor()
}
