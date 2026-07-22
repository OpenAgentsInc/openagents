import { existsSync, readFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import {
  computeComplexity,
  type ComplexityAssessment,
  type ParsedConversation,
} from "../../../scripts/coherence-core.ts"
import {
  analyzeFullAutoRunReport,
  type FullAutoRunAnalysis,
} from "./full-auto-run-analyzer.ts"
import {
  FULL_AUTO_RUN_REPORT_SCHEMA,
  FullAutoRunReportSchema,
  type FullAutoRunReport,
} from "./full-auto-run-report.ts"
import {
  FULL_AUTO_RUN_REGISTRY_SCHEMA,
  FullAutoRunSchema,
  type FullAutoRun,
} from "./full-auto-run-registry.ts"
import { fullAutoPlanProgressSummary } from "./full-auto-plan.ts"

/**
 * META-3 (#9182): the hillclimb grading baseline. Scores durable Full Auto
 * routing/dispatch receipts against the D1-D7 autonomy rubric in
 * `docs/analysis/2026-07-22-full-auto-autonomy-decision-quality-rubric.md`,
 * so later meta-agent routing/decomposition changes are MEASURABLE against a
 * dated baseline artifact. Measurement only -- this module holds no optimizer,
 * no dispatch, no prompt mutation, and no release/public-claim authority.
 *
 * Inputs are exactly the durable records the sibling analyzer already reads:
 * `FullAutoRunReport` rows (full-auto-run-report.ts, `<userData>/full-auto/
 * run-reports.json`) joined read-only with the matching `FullAutoRun` registry
 * rows (`<userData>/full-auto/runs.json`) for the HANDS-1..4 autonomy block
 * (objective source, persistent plan, host-verification verdicts). The
 * analyzer itself (`analyzeFullAutoRunReport`, FA-RUN-05 #8973) is CONSUMED,
 * never forked: its typed findings are the drift/repetition/churn/evidence
 * signals the rubric dimensions read.
 *
 * Honesty rules (the core discipline of this module):
 *  - A dimension is scored ONLY where the rubric signal is mechanically
 *    derivable from the typed records. Anything else is `not_measured` with a
 *    typed reason -- never fabricated, never defaulted to 0 or to the design
 *    scores in the rubric doc's Part 4.
 *  - Several dimensions have a MECHANICAL CEILING below the rubric's 4 (their
 *    top descriptors need semantic judgment or record fields that do not exist
 *    yet). Each scored dimension carries that ceiling explicitly so a later
 *    reader cannot mistake "3" for "hit the rubric maximum".
 *  - The COH-01 coherence-screen-v2 tooling (scripts/coherence-core.ts) is
 *    reused for the complexity tier, but run reports deliberately carry NO
 *    transcript text, tool calls, or sub-agent events (redaction discipline),
 *    so the derived tier is labeled a LOWER BOUND and the coherence screen's
 *    user-signal score is honestly `not_measured` over these records.
 *  - Cost/latency per VERIFIED outcome divides exact usage by host-verified
 *    outcomes only. Unknown usage or zero verified outcomes is `not_measured`
 *    (`known: false` is never conflated with 0 -- same rule as the report's
 *    own usage block).
 *
 * Everything emitted is public-safe by construction: digests, counts, rates,
 * system-minted refs, and typed enum/reason literals -- never objective text,
 * prompts, workspace paths, account identity, or secrets.
 */
export const FULL_AUTO_RUN_GRADE_SCHEMA = "openagents.desktop.full_auto_run_grade.v1" as const
export const FULL_AUTO_GRADING_BASELINE_SCHEMA = "openagents.desktop.full_auto_grading_baseline.v1" as const

/** The metric identity this baseline scores against -- pinned, never
 * "latest", so two baselines are comparable only when this matches. */
export const FULL_AUTO_GRADING_METRIC = "full-auto-decision-v1" as const

export const FULL_AUTO_GRADING_MAX_RUNS = 500

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Digest = Schema.String.check(Schema.isLengthBetween(64, 64))
const Count = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const Score = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(4))
const Basis = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300))

// -----------------------------------------------------------------------
// Rubric dimensions and per-dimension scores.
// -----------------------------------------------------------------------

export const RubricDimensionSchema = Schema.Literals([
  "D1_complexity",
  "D2_coherence",
  "D3_foresight",
  "D4_groundedness",
  "D5_selectivity",
  "D6_self_verification",
  "D7_recoverability",
])
export type RubricDimension = typeof RubricDimensionSchema.Type

export const RUBRIC_DIMENSIONS: ReadonlyArray<RubricDimension> = RubricDimensionSchema.literals

/** Typed reasons a dimension (or the cost metric) could not be measured from
 * the records. New reasons are additive literals, never free text. */
export const NotMeasuredReasonSchema = Schema.Literals([
  "no_turns_recorded",
  "run_record_unavailable",
  "no_completed_turns_and_no_refs",
  "no_completion_observed_and_no_host_verification",
  "no_blocker_observed",
  "usage_unknown",
  "zero_verified_outcomes",
  "run_not_ended",
  "transcript_signals_not_in_run_records",
])
export type NotMeasuredReason = typeof NotMeasuredReasonSchema.Type

/** A measured score always names its mechanical ceiling: the highest score
 * this derivation COULD have produced from the available records. A ceiling
 * below 4 documents a record-shape/semantic gap, not a low-quality run. */
export const MeasuredDimensionScoreSchema = Schema.Struct({
  measured: Schema.Literal(true),
  score: Score,
  mechanicalCeiling: Score,
  basis: Basis,
  /** `turnRef`/`handoffRef`/finding-kind evidence already present on the
   * analyzed report -- exact pointers, never re-derived text. */
  evidenceRefs: Schema.Array(Ref).check(Schema.isMaxLength(20)),
})
export type MeasuredDimensionScore = typeof MeasuredDimensionScoreSchema.Type

export const UnmeasuredDimensionScoreSchema = Schema.Struct({
  measured: Schema.Literal(false),
  reason: NotMeasuredReasonSchema,
  basis: Basis,
})
export type UnmeasuredDimensionScore = typeof UnmeasuredDimensionScoreSchema.Type

export const DimensionScoreSchema = Schema.Union([
  MeasuredDimensionScoreSchema,
  UnmeasuredDimensionScoreSchema,
])
export type DimensionScore = typeof DimensionScoreSchema.Type

const measured = (
  score: number,
  mechanicalCeiling: number,
  basis: string,
  evidenceRefs: ReadonlyArray<string> = [],
): DimensionScore => ({
  measured: true,
  score,
  mechanicalCeiling,
  basis,
  evidenceRefs: evidenceRefs.slice(0, 20),
})

const notMeasured = (reason: NotMeasuredReason, basis: string): DimensionScore => ({
  measured: false,
  reason,
  basis,
})

// -----------------------------------------------------------------------
// Coherence @ complexity (COH-01 reuse) -- lower-bound tier, honest screen.
// -----------------------------------------------------------------------

export const RunComplexityLowerBoundSchema = Schema.Struct({
  metric: Schema.Literal("coherence-screen-v2"),
  /** Deterministic COH-01 `computeComplexity` output over the run-report
   * fields that map onto its features. A LOWER BOUND: the report carries no
   * tool-call, file-change, or sub-agent counts, so those features are zero
   * by record absence, not by observation. */
  lowerBound: Schema.Literal(true),
  score: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(100)),
  tier: Schema.Literals(["C0", "C1", "C2", "C3", "C4"]),
  /** The coherence-screen user-signal score cannot run over run reports:
   * they carry no user/transcript text by redaction design. Fixed literal so
   * the honesty is structural. */
  coherenceScreenMeasured: Schema.Literal(false),
  coherenceScreenReason: Schema.Literal("transcript_signals_not_in_run_records"),
})
export type RunComplexityLowerBound = typeof RunComplexityLowerBoundSchema.Type

/** Map the run report's fields onto the COH-01 `ParsedConversation` feature
 * surface. Absent features are zero (lower bound), never guessed. */
export const runReportComplexity = (report: FullAutoRunReport): ComplexityAssessment => {
  const resolvedTurns = report.turns.filter((turn) => turn.disposition !== null)
  const parsed: ParsedConversation = {
    source: "multi-harness",
    path: `run:${report.runRef}`,
    userTurnCount: 1 + report.ownerActions.length,
    assistantTurnCount: resolvedTurns.length,
    toolCallCount: 0,
    fileChangeCount: 0,
    interruptCount: report.turns.filter((turn) => turn.disposition === "owner_interrupted").length,
    firstTimestamp: report.startedAt ?? report.createdAt,
    signals: [],
    toolKinds: [],
    models: [...new Set(report.turns.flatMap((turn) => (turn.model === undefined ? [] : [turn.model])))].sort(),
    subAgentStarts: 0,
    subAgentInteractions: 0,
    distinctSubAgents: 0,
  }
  return computeComplexity(parsed)
}

// -----------------------------------------------------------------------
// Cost / latency per verified outcome.
// -----------------------------------------------------------------------

export const RunCostPerVerifiedOutcomeSchema = Schema.Union([
  Schema.Struct({
    measured: Schema.Literal(true),
    verifiedOutcomes: Count,
    totalTokens: Count,
    tokensPerVerifiedOutcome: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
    costUsdPerVerifiedOutcome: Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))),
    wallClockMsPerVerifiedOutcome: Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))),
  }),
  Schema.Struct({
    measured: Schema.Literal(false),
    reason: NotMeasuredReasonSchema,
    verifiedOutcomes: Count,
  }),
])
export type RunCostPerVerifiedOutcome = typeof RunCostPerVerifiedOutcomeSchema.Type

/** Host-verified outcomes only: `verified` refs on the report plus a PASSED
 * HANDS-2 host verification on the run record. Claimed refs and provider
 * self-reports never count -- verification is the label supply (#9173). */
export const countVerifiedOutcomes = (
  report: FullAutoRunReport,
  run: FullAutoRun | null,
): number =>
  report.verifiedRefs.filter((ref) => ref.verification === "verified").length +
  (run?.autonomy?.lastVerification?.status === "passed" ? 1 : 0)

const deriveCostPerVerifiedOutcome = (
  report: FullAutoRunReport,
  run: FullAutoRun | null,
): RunCostPerVerifiedOutcome => {
  const verifiedOutcomes = countVerifiedOutcomes(report, run)
  if (!report.usage.totalTokensKnown || report.usage.totalTokens === null) {
    return { measured: false, reason: "usage_unknown", verifiedOutcomes }
  }
  if (verifiedOutcomes === 0) {
    return { measured: false, reason: "zero_verified_outcomes", verifiedOutcomes }
  }
  const wallClockMs =
    report.startedAt !== undefined && report.endedAt !== undefined
      ? Math.max(0, Date.parse(report.endedAt) - Date.parse(report.startedAt))
      : null
  return {
    measured: true,
    verifiedOutcomes,
    totalTokens: report.usage.totalTokens,
    tokensPerVerifiedOutcome: report.usage.totalTokens / verifiedOutcomes,
    costUsdPerVerifiedOutcome:
      report.usage.costUsdKnown && report.usage.costUsd !== null
        ? report.usage.costUsd / verifiedOutcomes
        : null,
    wallClockMsPerVerifiedOutcome: wallClockMs === null ? null : wallClockMs / verifiedOutcomes,
  }
}

// -----------------------------------------------------------------------
// Per-dimension derivations. Each one is a pure function of the report, its
// analyzer output, and the (nullable) run record. Rubric text:
// docs/analysis/2026-07-22-full-auto-autonomy-decision-quality-rubric.md.
// -----------------------------------------------------------------------

const findingCount = (analysis: FullAutoRunAnalysis, kind: string): number =>
  analysis.findings.filter((finding) => finding.kind === kind).length

const findingRefs = (analysis: FullAutoRunAnalysis, kind: string): ReadonlyArray<string> =>
  analysis.findings.filter((finding) => finding.kind === kind).flatMap((finding) => finding.evidenceRefs)

/** D1: mechanical ceiling 3 -- score 4 needs host-orchestrated sub-agents and
 * handoff sequencing, which no run-record field can evidence today. */
const gradeComplexity = (
  report: FullAutoRunReport,
  analysis: FullAutoRunAnalysis,
  run: FullAutoRun | null,
): DimensionScore => {
  if (report.turns.length === 0) {
    return notMeasured("no_turns_recorded", "the report carries zero turn entries, so no complexity is observable")
  }
  const plan = run?.autonomy?.plan
  const planTracked =
    plan !== undefined && plan.steps.length >= 2 && plan.steps.some((step) => step.status !== "pending")
  if (planTracked) {
    return measured(3, 3, "host-tracked plan with advanced sub-task state carried across turns (HANDS-3)")
  }
  if (analysis.turns.resolved >= 2) {
    return measured(2, 3, "sustained multi-turn work on one objective; the host carried no sub-task decomposition")
  }
  return measured(1, 3, "at most one resolved turn; no cross-turn structure is evidenced")
}

/** D2: mechanical ceiling 3 -- score 4 needs a semantic causal-link judgment
 * over transcript content the records deliberately never carry. */
const gradeCoherence = (
  report: FullAutoRunReport,
  analysis: FullAutoRunAnalysis,
  run: FullAutoRun | null,
): DimensionScore => {
  if (report.turns.length === 0) {
    return notMeasured("no_turns_recorded", "the report carries zero turn entries, so no line of work is observable")
  }
  const drift = findingCount(analysis, "objective_drift_revision")
  const churn = findingCount(analysis, "low_value_churn")
  const repeats = findingCount(analysis, "repeated_disposition_pattern")
  const refs = [
    ...findingRefs(analysis, "objective_drift_revision"),
    ...findingRefs(analysis, "low_value_churn"),
    ...findingRefs(analysis, "repeated_disposition_pattern"),
  ]
  if (churn > 0 && drift > 0) {
    return measured(0, 3, "value-aware churn plus objective drift: the turns thrash rather than cohere", refs)
  }
  if (churn > 0 || drift > 0 || repeats >= 2) {
    return measured(1, 3, "drift/churn/repeated-disposition findings mark frequent drift in the line of work", refs)
  }
  const plan = run?.autonomy?.plan
  if (plan !== undefined && fullAutoPlanProgressSummary(plan).done >= 1) {
    return measured(3, 3, "turns advance a carried plan (completed steps) with no drift or churn findings")
  }
  return measured(2, 3, "the objective anchor held (no drift/churn findings) but no carried cross-turn work memory is evidenced")
}

/** D3: mechanical ceiling 3 -- score 4 needs live reordering on new evidence,
 * which is not derivable from the stored plan snapshot alone. */
const gradeForesight = (run: FullAutoRun | null): DimensionScore => {
  if (run === null) {
    return notMeasured("run_record_unavailable", "no run-registry record was joined, so the autonomy plan state is unknown")
  }
  const plan = run.autonomy?.plan
  if (plan === undefined) {
    return measured(1, 3, "no persisted plan: the host prompt is explicitly myopic (one next step now)")
  }
  const summary = fullAutoPlanProgressSummary(plan)
  if (summary.done + summary.skipped + summary.inProgress === 0) {
    return measured(2, 3, "a persisted plan exists but no step ever advanced beyond pending")
  }
  return measured(3, 3, "the host tracks a dependency-ordered plan and steps advanced through it (HANDS-3)")
}

/** D4: mechanical ceiling 3 -- score 4 needs the host to REJECT ungrounded
 * actions, a gate that does not exist in the records. */
const gradeGroundedness = (
  report: FullAutoRunReport,
  analysis: FullAutoRunAnalysis,
): DimensionScore => {
  const verifiedRefs = report.verifiedRefs.filter((ref) => ref.verification === "verified")
  if (verifiedRefs.length > 0) {
    return measured(
      3,
      3,
      "host-verified repository/evidence refs are on file for this run",
      verifiedRefs.flatMap((ref) => (ref.turnRef === undefined ? [] : [ref.turnRef])),
    )
  }
  const claimedRefs = report.verifiedRefs.filter((ref) => ref.verification === "claimed")
  if (claimedRefs.length > 0) {
    return measured(
      2,
      3,
      "claimed (unverified) commit/evidence refs exist; the host did not verify the grounding",
      claimedRefs.flatMap((ref) => (ref.turnRef === undefined ? [] : [ref.turnRef])),
    )
  }
  if (analysis.turns.completed > 0) {
    return measured(1, 3, "turns completed but produced zero grounded refs (missing_evidence)", findingRefs(analysis, "missing_evidence"))
  }
  return notMeasured("no_completed_turns_and_no_refs", "no completed turn and no refs: grounding was never exercised")
}

/** D5: mechanical ceiling 1 -- the run registry's objective-source literals
 * are owner/caller/legacy only. No typed "system_selected" source exists, so
 * a HANDS-1 selection outcome cannot be attributed on the run record yet
 * (record-shape gap; see `FULL_AUTO_GRADING_RECORD_SHAPE_GAPS`). */
const gradeSelectivity = (run: FullAutoRun | null): DimensionScore => {
  if (run === null) {
    return notMeasured("run_record_unavailable", "no run-registry record was joined, so the objective source is unknown")
  }
  if (run.objectiveSource === "legacy_migration") {
    return measured(1, 1, "legacy generic objective: the system worked an unranked useful-thing objective")
  }
  return measured(0, 1, `owner-supplied objective (source: ${run.objectiveSource}): the system did not pick the work`)
}

/** D6: mechanical ceiling 4 -- a failed host verdict on a run held out of
 * `completed` is direct evidence the gate blocked a false completion. */
const gradeSelfVerification = (
  report: FullAutoRunReport,
  analysis: FullAutoRunAnalysis,
  run: FullAutoRun | null,
): DimensionScore => {
  const verification = run?.autonomy?.lastVerification
  if (verification !== undefined) {
    if (verification.status === "failed" && report.state !== "completed") {
      return measured(4, 4, "host verification failed and the run was NOT admitted to completed: the gate blocked a false completion")
    }
    if (verification.status === "passed" || verification.status === "failed") {
      return measured(3, 4, `the host executed the done-condition check and recorded a typed ${verification.status} verdict (HANDS-2)`)
    }
    return measured(
      report.verifiedRefs.length > 0 ? 2 : 1,
      4,
      `a host verification was attempted but could not verify (${verification.status})`,
    )
  }
  if (report.state === "completed") {
    if (analysis.evidence.unverifiedCompletionRisk) {
      return measured(1, 4, "self-reported completion with zero refs; only the post-hoc unverified_completion_risk flag fired")
    }
    return measured(2, 4, "self-reported completion with attached refs but no executed done-condition check")
  }
  return notMeasured(
    "no_completion_observed_and_no_host_verification",
    "the run never reached completed and no host verification ran, so self-verification was never exercised",
  )
}

/** D7: mechanical ceiling 3 -- score 4 needs value-aware stall detection
 * evidence (a typed churn pause), which no current record ties to recovery. */
const gradeRecoverability = (
  report: FullAutoRunReport,
  analysis: FullAutoRunAnalysis,
): DimensionScore => {
  const failures = analysis.turns.failed + report.failedAttempts
  if (failures === 0 && analysis.liveness.gapCount === 0) {
    return notMeasured("no_blocker_observed", "no failure or liveness gap occurred, so recovery was never exercised")
  }
  const rotated = (report.rotationHistory?.length ?? 0) > 0
  const recoveryActed = Object.entries(analysis.liveness.recoveryActionCounts).some(
    ([action, count]) => action !== "none" && count > 0,
  )
  const sorted = [...report.turns].toSorted((left, right) => left.createdAt.localeCompare(right.createdAt))
  let sawFailure = false
  let completedAfterFailure = false
  for (const turn of sorted) {
    if (turn.disposition === "failed") sawFailure = true
    else if (turn.disposition === "completed" && sawFailure) completedAfterFailure = true
  }
  if (rotated || recoveryActed || completedAfterFailure) {
    return measured(
      3,
      3,
      "the run recovered from failure without owner rescue (rotation, typed recovery action, or completed work after a failed turn)",
    )
  }
  if (analysis.control.systemForcedStopCount > 0) {
    return measured(2, 3, "failures ended in a typed system-forced stop rather than an observed recovery", findingRefs(analysis, "system_forced_stop"))
  }
  return measured(1, 3, "failures occurred with no recorded recovery, rotation, or typed stop")
}

// -----------------------------------------------------------------------
// The per-run grade.
// -----------------------------------------------------------------------

export const FullAutoRunGradeSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_GRADE_SCHEMA),
  metric: Schema.Literal(FULL_AUTO_GRADING_METRIC),
  runRef: Ref,
  objectiveDigest: Digest,
  doneConditionDigest: Digest,
  state: Schema.String,
  /** Whether a run-registry record was joined -- several dimensions need it. */
  runRecordJoined: Schema.Boolean,
  dimensions: Schema.Struct({
    D1_complexity: DimensionScoreSchema,
    D2_coherence: DimensionScoreSchema,
    D3_foresight: DimensionScoreSchema,
    D4_groundedness: DimensionScoreSchema,
    D5_selectivity: DimensionScoreSchema,
    D6_self_verification: DimensionScoreSchema,
    D7_recoverability: DimensionScoreSchema,
  }),
  complexity: RunComplexityLowerBoundSchema,
  costPerVerifiedOutcome: RunCostPerVerifiedOutcomeSchema,
  /** Analyzer finding kinds -> counts, copied for traceability. */
  findingCounts: Schema.Record(Schema.String, Count),
  gradedAt: Schema.String,
})
export type FullAutoRunGrade = typeof FullAutoRunGradeSchema.Type
const decodeFullAutoRunGrade = Schema.decodeUnknownSync(FullAutoRunGradeSchema)

export type GradeFullAutoRunInput = Readonly<{
  report: FullAutoRunReport
  /** The joined run-registry record, or null when none exists (the grade
   * degrades honestly: plan/objective-source/verification dimensions become
   * `not_measured`/record-limited). */
  run?: FullAutoRun | null
  /** A precomputed analyzer output; computed via `analyzeFullAutoRunReport`
   * when absent. */
  analysis?: FullAutoRunAnalysis
  now?: () => Date
}>

export const gradeFullAutoRun = (input: GradeFullAutoRunInput): FullAutoRunGrade => {
  const now = input.now ?? (() => new Date())
  const run = input.run ?? null
  const report = input.report
  const analysis = input.analysis ?? analyzeFullAutoRunReport(report, now)

  const complexityAssessment = runReportComplexity(report)
  const findingCounts: Record<string, number> = {}
  for (const finding of analysis.findings) {
    findingCounts[finding.kind] = (findingCounts[finding.kind] ?? 0) + 1
  }

  return decodeFullAutoRunGrade({
    schema: FULL_AUTO_RUN_GRADE_SCHEMA,
    metric: FULL_AUTO_GRADING_METRIC,
    runRef: report.runRef,
    objectiveDigest: report.objectiveDigest,
    doneConditionDigest: report.doneConditionDigest,
    state: report.state,
    runRecordJoined: run !== null,
    dimensions: {
      D1_complexity: gradeComplexity(report, analysis, run),
      D2_coherence: gradeCoherence(report, analysis, run),
      D3_foresight: gradeForesight(run),
      D4_groundedness: gradeGroundedness(report, analysis),
      D5_selectivity: gradeSelectivity(run),
      D6_self_verification: gradeSelfVerification(report, analysis, run),
      D7_recoverability: gradeRecoverability(report, analysis),
    },
    complexity: {
      metric: "coherence-screen-v2",
      lowerBound: true,
      score: complexityAssessment.score,
      tier: complexityAssessment.tier,
      coherenceScreenMeasured: false,
      coherenceScreenReason: "transcript_signals_not_in_run_records",
    },
    costPerVerifiedOutcome: deriveCostPerVerifiedOutcome(report, run),
    findingCounts,
    gradedAt: now().toISOString(),
  })
}

// -----------------------------------------------------------------------
// The aggregate baseline artifact.
// -----------------------------------------------------------------------

export const DimensionAggregateSchema = Schema.Struct({
  dimension: RubricDimensionSchema,
  measuredCount: Count,
  notMeasuredCount: Count,
  /** Null when nothing was measured -- never a fabricated 0. */
  meanScore: Schema.NullOr(Schema.Number),
  minScore: Schema.NullOr(Score),
  maxScore: Schema.NullOr(Score),
  scoreCounts: Schema.Record(Schema.String, Count),
  notMeasuredReasons: Schema.Record(Schema.String, Count),
})
export type DimensionAggregate = typeof DimensionAggregateSchema.Type

export const FullAutoGradingBaselineSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_GRADING_BASELINE_SCHEMA),
  metric: Schema.Literal(FULL_AUTO_GRADING_METRIC),
  generatedAt: Schema.String,
  runCount: Count,
  runRecordJoinedCount: Count,
  dimensions: Schema.Array(DimensionAggregateSchema).check(Schema.isMaxLength(7)),
  /** Coherence @ complexity over the derivable proxies: complexity-tier
   * counts (lower bound) plus the mean D2 score weighted by each run's
   * complexity score -- the COH-01 weighting rule applied to the rubric's
   * own coherence dimension, since the transcript-level coherence screen
   * cannot run over these records. */
  coherenceAtComplexity: Schema.Struct({
    tierCounts: Schema.Record(Schema.String, Count),
    complexityWeightedMeanD2: Schema.NullOr(Schema.Number),
    coherenceScreenMeasured: Schema.Literal(false),
    coherenceScreenReason: Schema.Literal("transcript_signals_not_in_run_records"),
  }),
  cost: Schema.Struct({
    runsWithKnownUsage: Count,
    runsWithVerifiedOutcomes: Count,
    totalVerifiedOutcomes: Count,
    /** Aggregated only over runs where BOTH usage and verified outcomes are
     * known; null otherwise. */
    tokensPerVerifiedOutcome: Schema.NullOr(Schema.Number),
    costUsdPerVerifiedOutcome: Schema.NullOr(Schema.Number),
    wallClockMsPerVerifiedOutcome: Schema.NullOr(Schema.Number),
  }),
  /** Record-shape gaps this baseline surfaced -- fields the rubric needs that
   * the durable records do not carry yet. Static typed strings. */
  recordShapeGaps: Schema.Array(Schema.String).check(Schema.isMaxLength(10)),
  grades: Schema.Array(FullAutoRunGradeSchema).check(Schema.isMaxLength(FULL_AUTO_GRADING_MAX_RUNS)),
})
export type FullAutoGradingBaseline = typeof FullAutoGradingBaselineSchema.Type
const decodeFullAutoGradingBaseline = Schema.decodeUnknownSync(FullAutoGradingBaselineSchema)

/** The record-shape gaps found while building this grader. Kept as data on
 * the artifact so every baseline names what it could not measure and why. */
export const FULL_AUTO_GRADING_RECORD_SHAPE_GAPS: ReadonlyArray<string> = [
  "objectiveSource has no system_selected literal, so a HANDS-1 selected objective cannot be attributed (D5 ceiling 1)",
  "run reports carry no tool-call/file-change/sub-agent counts, so the coherence-screen-v2 complexity tier is a lower bound and the coherence screen itself cannot run (D1/D2)",
  "run reports carry no per-turn action-taxonomy rows, so churn detection only runs when a caller supplies actions (D2/D7)",
  "no current writer populates report usage totals, so cost per verified outcome stays not_measured on real runs until usage ingestion lands",
]

export type BuildFullAutoGradingBaselineInput = Readonly<{
  entries: ReadonlyArray<Readonly<{ report: FullAutoRunReport; run?: FullAutoRun | null }>>
  now?: () => Date
}>

export const buildFullAutoGradingBaseline = (
  input: BuildFullAutoGradingBaselineInput,
): FullAutoGradingBaseline => {
  const now = input.now ?? (() => new Date())
  const grades = input.entries
    .slice(0, FULL_AUTO_GRADING_MAX_RUNS)
    .map((entry) => gradeFullAutoRun({ report: entry.report, run: entry.run ?? null, now }))

  const dimensions: Array<DimensionAggregate> = RUBRIC_DIMENSIONS.map((dimension) => {
    const scores = grades.map((grade) => grade.dimensions[dimension])
    const measuredScores = scores.filter((score): score is MeasuredDimensionScore => score.measured)
    const scoreCounts: Record<string, number> = {}
    for (const score of measuredScores) {
      const key = String(score.score)
      scoreCounts[key] = (scoreCounts[key] ?? 0) + 1
    }
    const notMeasuredReasons: Record<string, number> = {}
    for (const score of scores) {
      if (!score.measured) notMeasuredReasons[score.reason] = (notMeasuredReasons[score.reason] ?? 0) + 1
    }
    const values = measuredScores.map((score) => score.score)
    return {
      dimension,
      measuredCount: measuredScores.length,
      notMeasuredCount: scores.length - measuredScores.length,
      meanScore: values.length === 0 ? null : values.reduce((sum, value) => sum + value, 0) / values.length,
      minScore: values.length === 0 ? null : Math.min(...values),
      maxScore: values.length === 0 ? null : Math.max(...values),
      scoreCounts,
      notMeasuredReasons,
    }
  })

  const tierCounts: Record<string, number> = {}
  let weightSum = 0
  let weightedD2Sum = 0
  let d2Weighted = false
  for (const grade of grades) {
    tierCounts[grade.complexity.tier] = (tierCounts[grade.complexity.tier] ?? 0) + 1
    const d2 = grade.dimensions.D2_coherence
    if (d2.measured) {
      const weight = Math.max(grade.complexity.score, 1)
      weightSum += weight
      weightedD2Sum += d2.score * weight
      d2Weighted = true
    }
  }

  let runsWithKnownUsage = 0
  let runsWithVerifiedOutcomes = 0
  let totalVerifiedOutcomes = 0
  let tokensSum = 0
  let tokensOutcomes = 0
  let costSum = 0
  let costOutcomes = 0
  let wallClockSum = 0
  let wallClockOutcomes = 0
  for (const grade of grades) {
    const cost = grade.costPerVerifiedOutcome
    totalVerifiedOutcomes += cost.verifiedOutcomes
    if (cost.verifiedOutcomes > 0) runsWithVerifiedOutcomes += 1
    if (cost.measured) {
      runsWithKnownUsage += 1
      tokensSum += cost.totalTokens
      tokensOutcomes += cost.verifiedOutcomes
      if (cost.costUsdPerVerifiedOutcome !== null) {
        costSum += cost.costUsdPerVerifiedOutcome * cost.verifiedOutcomes
        costOutcomes += cost.verifiedOutcomes
      }
      if (cost.wallClockMsPerVerifiedOutcome !== null) {
        wallClockSum += cost.wallClockMsPerVerifiedOutcome * cost.verifiedOutcomes
        wallClockOutcomes += cost.verifiedOutcomes
      }
    } else if (cost.reason === "zero_verified_outcomes") {
      // Usage was known but yielded no verified outcome -- still a run with
      // known usage for the aggregate denominator honesty.
      runsWithKnownUsage += 1
    }
  }

  return decodeFullAutoGradingBaseline({
    schema: FULL_AUTO_GRADING_BASELINE_SCHEMA,
    metric: FULL_AUTO_GRADING_METRIC,
    generatedAt: now().toISOString(),
    runCount: grades.length,
    runRecordJoinedCount: grades.filter((grade) => grade.runRecordJoined).length,
    dimensions,
    coherenceAtComplexity: {
      tierCounts,
      complexityWeightedMeanD2: d2Weighted && weightSum > 0 ? weightedD2Sum / weightSum : null,
      coherenceScreenMeasured: false,
      coherenceScreenReason: "transcript_signals_not_in_run_records",
    },
    cost: {
      runsWithKnownUsage,
      runsWithVerifiedOutcomes,
      totalVerifiedOutcomes,
      tokensPerVerifiedOutcome: tokensOutcomes > 0 ? tokensSum / tokensOutcomes : null,
      costUsdPerVerifiedOutcome: costOutcomes > 0 ? costSum / costOutcomes : null,
      wallClockMsPerVerifiedOutcome: wallClockOutcomes > 0 ? wallClockSum / wallClockOutcomes : null,
    },
    recordShapeGaps: FULL_AUTO_GRADING_RECORD_SHAPE_GAPS,
    grades,
  })
}

// -----------------------------------------------------------------------
// Read-only loaders over the durable stores. These deliberately do NOT use
// the stores' own `open*` functions: those quarantine (rename) a corrupt
// file on decode failure, and a grader must never mutate the stores it
// measures. A missing or undecodable file yields an empty list plus a typed
// note the CLI surfaces.
// -----------------------------------------------------------------------

const RunReportFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_REPORT_SCHEMA),
  reports: Schema.Array(FullAutoRunReportSchema),
})
const decodeRunReportFile = Schema.decodeUnknownSync(RunReportFileSchema)

const RunRegistryFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_REGISTRY_SCHEMA),
  runs: Schema.Array(FullAutoRunSchema),
})
const decodeRunRegistryFile = Schema.decodeUnknownSync(RunRegistryFileSchema)

export type ReadOnlyLoadResult<T> = Readonly<{
  values: ReadonlyArray<T>
  /** `missing` -- the file does not exist (no activity yet); `undecodable`
   * -- it exists but failed schema decode (surfaced, never quarantined). */
  issue: "none" | "missing" | "undecodable"
  filePath: string
}>

export const readFullAutoRunReports = (userDataDir: string): ReadOnlyLoadResult<FullAutoRunReport> => {
  const filePath = path.join(userDataDir, "full-auto", "run-reports.json")
  if (!existsSync(filePath)) return { values: [], issue: "missing", filePath }
  try {
    return { values: decodeRunReportFile(JSON.parse(readFileSync(filePath, "utf8"))).reports, issue: "none", filePath }
  } catch {
    return { values: [], issue: "undecodable", filePath }
  }
}

export const readFullAutoRuns = (userDataDir: string): ReadOnlyLoadResult<FullAutoRun> => {
  const filePath = path.join(userDataDir, "full-auto", "runs.json")
  if (!existsSync(filePath)) return { values: [], issue: "missing", filePath }
  try {
    return { values: decodeRunRegistryFile(JSON.parse(readFileSync(filePath, "utf8"))).runs, issue: "none", filePath }
  } catch {
    return { values: [], issue: "undecodable", filePath }
  }
}

// -----------------------------------------------------------------------
// Human-readable rendering -- bounded, public-safe (digests/counts/refs).
// -----------------------------------------------------------------------

const formatScore = (score: DimensionScore): string =>
  score.measured
    ? `${score.score}/4 (ceiling ${score.mechanicalCeiling}) -- ${score.basis}`
    : `not_measured (${score.reason}) -- ${score.basis}`

const formatMean = (value: number | null): string => (value === null ? "n/a" : value.toFixed(2))

export const renderFullAutoGradingBaselineMarkdown = (baseline: FullAutoGradingBaseline): string => {
  const lines: Array<string> = [
    `# Full Auto grading baseline (${FULL_AUTO_GRADING_METRIC})`,
    "",
    `- Generated: ${baseline.generatedAt}`,
    `- Runs graded: ${baseline.runCount} (run record joined: ${baseline.runRecordJoinedCount})`,
    "- Authority: analysis only. This artifact cannot admit a release or a public claim.",
    "",
    "## Dimension aggregates",
    "",
    "| Dimension | Measured | Not measured | Mean | Min | Max |",
    "| --- | --- | --- | --- | --- | --- |",
  ]
  for (const aggregate of baseline.dimensions) {
    lines.push(
      `| ${aggregate.dimension} | ${aggregate.measuredCount} | ${aggregate.notMeasuredCount} | ${formatMean(aggregate.meanScore)} | ${aggregate.minScore ?? "n/a"} | ${aggregate.maxScore ?? "n/a"} |`,
    )
  }
  lines.push(
    "",
    "## Coherence @ complexity",
    "",
    `- Complexity tier counts (COH-01 lower bound): ${JSON.stringify(baseline.coherenceAtComplexity.tierCounts)}`,
    `- Complexity-weighted mean D2: ${formatMean(baseline.coherenceAtComplexity.complexityWeightedMeanD2)}`,
    "- The coherence-screen-v2 user-signal score is not measured: run records carry no transcript signals.",
    "",
    "## Cost per verified outcome",
    "",
    `- Runs with known usage: ${baseline.cost.runsWithKnownUsage}; runs with verified outcomes: ${baseline.cost.runsWithVerifiedOutcomes}; total verified outcomes: ${baseline.cost.totalVerifiedOutcomes}`,
    `- Tokens per verified outcome: ${formatMean(baseline.cost.tokensPerVerifiedOutcome)}`,
    `- Cost (USD) per verified outcome: ${formatMean(baseline.cost.costUsdPerVerifiedOutcome)}`,
    `- Wall clock (ms) per verified outcome: ${formatMean(baseline.cost.wallClockMsPerVerifiedOutcome)}`,
    "",
    "## Record-shape gaps",
    "",
  )
  for (const gap of baseline.recordShapeGaps) lines.push(`- ${gap}`)
  lines.push("", "## Per-run grades", "")
  for (const grade of baseline.grades) {
    lines.push(
      `### ${grade.runRef}`,
      "",
      `- State: ${grade.state}; objective digest: ${grade.objectiveDigest.slice(0, 12)}…; run record joined: ${grade.runRecordJoined}`,
      `- Complexity (lower bound): ${grade.complexity.tier} (${grade.complexity.score})`,
    )
    for (const dimension of RUBRIC_DIMENSIONS) {
      lines.push(`- ${dimension}: ${formatScore(grade.dimensions[dimension])}`)
    }
    const cost = grade.costPerVerifiedOutcome
    lines.push(
      cost.measured
        ? `- Cost: ${cost.tokensPerVerifiedOutcome.toFixed(1)} tokens per verified outcome (${cost.verifiedOutcomes} verified)`
        : `- Cost: not_measured (${cost.reason}; verified outcomes: ${cost.verifiedOutcomes})`,
      "",
    )
  }
  return `${lines.join("\n").replace(/\n+$/, "")}\n`
}
