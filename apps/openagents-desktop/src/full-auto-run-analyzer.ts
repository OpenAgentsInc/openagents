import { Schema } from "effect"

import { detectFullAutoChurn, type FullAutoTurnAction } from "./full-auto-churn.ts"
import type {
  FullAutoRunReport,
  FullAutoRunReportLivenessGap,
  FullAutoRunReportTurnEntry,
} from "./full-auto-run-report.ts"
import { FullAutoRunActorSchema } from "./full-auto-run-registry.ts"
import type { LocalTurnDisposition } from "./local-turn-journal.ts"

/**
 * FA-RUN-05 (#8973): the bounded, OFFLINE/PRIVATE dogfood analyzer over a
 * `FullAutoRunReport` (#8972). This module is a pure deterministic
 * measurement layer -- it never makes a network call, never reads a raw
 * transcript, and never mutates a run, a prompt, an issue, or release state.
 * Its only inputs are already-bounded, already-redaction-disciplined report
 * fields (turn identity/phase/disposition, lifecycle transitions, liveness
 * observations/gaps, provider-handoff dispositions, verified-ref
 * verification state, and known/unknown usage) -- exactly the fields
 * `full-auto-run-report.ts`'s own header comment documents as the aggregator's
 * complete, honest surface.
 *
 * SCOPE (deliberate, documented judgment calls -- see the issue's own
 * permission to make bounded choices rather than block):
 *
 *  - This is the DETERMINISTIC layer only. The issue's optional
 *    model-assisted qualitative layer is modeled here as a typed, pinned,
 *    cost-visible, advisory-only CONTRACT (`requestFullAutoRunModelAssistedReview`)
 *    that a caller must explicitly arm and supply a provider invoker for --
 *    this module never calls a live model provider itself. Wiring a real
 *    provider call is a separate, explicitly cost/consent-gated follow-up.
 *  - "Repeated reconnaissance/setup" and "repeated verification" are named in
 *    the issue as indicators derived from typed actions. The current report
 *    schema does not yet carry a typed per-turn action taxonomy (recon vs.
 *    setup vs. verify vs. edit) -- `LocalTurnRecord`/`FullAutoRunReportTurnEntry`
 *    only carry dispatch `phase` and terminal `disposition`. This module
 *    therefore measures the closest honest typed proxy available today:
 *    repeated identical TURN DISPOSITIONS in immediate succession with no
 *    intervening owner action (`repeatedDispositionRuns`/
 *    `repeated_disposition_pattern` findings). True semantic recon/verify
 *    repetition detection needs a typed action-taxonomy field upstream; this
 *    is flagged as a residual gap, not silently approximated as complete.
 *  - "Drift" is measured via the one typed signal the report actually carries
 *    for it -- `objectiveRevisionCount` -- as `objective_drift_revision`.
 *    This is a structural proxy (the objective/done-condition text changed
 *    mid-run), not a semantic judgement of whether the run's ACTIONS drifted
 *    from the objective; that stronger claim needs the model-assisted layer.
 *  - "False completion" is measured via `unverified_completion_risk`: a run
 *    that reached a `completed` state while carrying zero verified or
 *    claimed refs. This is a typed, conservative proxy, not a semantic
 *    correctness judgement of the claimed work.
 *  - Named replay comparison binds two runs by `objectiveDigest`/
 *    `doneConditionDigest` equality (the report's own typed identity, never
 *    free-text matching) rather than a separate `TestDefinition` registry --
 *    that registry does not exist in the repository yet. Callers may attach
 *    an opaque `testDefinitionRef`/`sourceRevisionRange` label to a
 *    comparison; this module does not look either up or enforce them.
 */
export const FULL_AUTO_RUN_ANALYSIS_SCHEMA = "openagents.desktop.full_auto_run_analysis.v1" as const
export const FULL_AUTO_RUN_COMPARISON_SCHEMA = "openagents.desktop.full_auto_run_comparison.v1" as const
export const FULL_AUTO_RUN_MODEL_ASSISTED_REVIEW_SCHEMA =
  "openagents.desktop.full_auto_run_model_assisted_review.v1" as const

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Summary = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200))
const Count = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
/** A rate in [0, 1], or `null` when the denominator is zero -- "no data" is
 * never conflated with "0". */
const Rate = Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(1)))
const DurationMs = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))

// -----------------------------------------------------------------------
// Small pure helpers.
// -----------------------------------------------------------------------

const durationOf = (fromIso: string, toIso: string): number => Math.max(0, Date.parse(toIso) - Date.parse(fromIso))

/** Population median. `null` for an empty input -- never fabricated as 0. */
export const median = (values: ReadonlyArray<number>): number | null => {
  if (values.length === 0) return null
  const sorted = [...values].toSorted((left, right) => left - right)
  const mid = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 0 ? (sorted[mid - 1]! + sorted[mid]!) / 2 : sorted[mid]!
}

// MOB-FA-02 (#8994): a phone-dispatched Pause/Resume/Stop is just as much an
// owner-directed action as a click in the Desktop UI or a CLI/MCP call --
// the owner's own phone, not a system/guardrail policy.
const OWNER_DIRECTED_ACTORS = new Set(["owner_ui", "control_api", "cli", "mcp", "mobile"] as const)

// -----------------------------------------------------------------------
// Findings: bounded, typed, code-authored -- never free transcript text.
// -----------------------------------------------------------------------

export const FullAutoRunFindingKindSchema = Schema.Literals([
  "successful_packet",
  "failed_continuation",
  "liveness_gap",
  "missing_owner_diagnosis",
  "repeated_disposition_pattern",
  "context_truncated",
  "provider_refused",
  "unverified_completion_risk",
  "missing_evidence",
  "objective_drift_revision",
  "system_forced_stop",
  /** HANDS-4 (#9175): value-aware churn -- repeated near-identical,
   * non-advancing `completed` turns, derived from the per-turn action
   * taxonomy (full-auto-churn.ts) rather than the disposition proxy above.
   * Emitted only when the caller supplies per-turn actions. */
  "low_value_churn",
  "clean_success",
])
export type FullAutoRunFindingKind = typeof FullAutoRunFindingKindSchema.Type

export const FullAutoRunFindingSeveritySchema = Schema.Literals(["info", "notable", "concerning"])
export type FullAutoRunFindingSeverity = typeof FullAutoRunFindingSeveritySchema.Type

/** `evidenceRefs` are `turnRef`/`handoffRef` values already present on the
 * report -- exact pointers a future UI can resolve, never re-derived text. */
export const FullAutoRunFindingSchema = Schema.Struct({
  kind: FullAutoRunFindingKindSchema,
  severity: FullAutoRunFindingSeveritySchema,
  summary: Summary,
  evidenceRefs: Schema.Array(Ref).check(Schema.isMaxLength(50)),
  at: Schema.optional(Schema.String),
})
export type FullAutoRunFinding = typeof FullAutoRunFindingSchema.Type

// -----------------------------------------------------------------------
// The analysis itself.
// -----------------------------------------------------------------------

export const FullAutoRunTurnMetricsSchema = Schema.Struct({
  total: Count,
  resolved: Count,
  completed: Count,
  failed: Count,
  ownerInterrupted: Count,
  restartInterrupted: Count,
  restartResumed: Count,
  unresolved: Count,
  successfulTurnRate: Rate,
  usefulWorkDispositionRate: Rate,
})
export type FullAutoRunTurnMetrics = typeof FullAutoRunTurnMetricsSchema.Type

export const FullAutoRunAutonomyMetricsSchema = Schema.Struct({
  /** Turn-count streaks between owner-attributed lifecycle actions --
   * `[3, 1, 5]` means the run executed 3 turns autonomously, the owner
   * touched it, then 1 turn, touched it again, then 5. */
  autonomousStreaks: Schema.Array(Count),
  longestAutonomousStreak: Count,
  medianAutonomousStreak: Schema.NullOr(Schema.Number),
  timeToFirstUsefulOutcomeKnown: Schema.Boolean,
  timeToFirstUsefulOutcomeMs: Schema.NullOr(DurationMs),
})
export type FullAutoRunAutonomyMetrics = typeof FullAutoRunAutonomyMetricsSchema.Type

export const FullAutoRunLivenessMetricsSchema = Schema.Struct({
  gapCount: Count,
  closedGapCount: Count,
  totalStalledMs: DurationMs,
  /** Set only when the report's final logged gap is still open (`durationMs`
   * is `null`); anchored at `report.updatedAt` (the report's own last-sync
   * time) unless the caller supplied a fresher `now`, and always labeled an
   * ESTIMATE rather than folded into `totalStalledMs`. */
  ongoingGapEstimateMs: Schema.NullOr(DurationMs),
  longestClosedGapMs: Schema.NullOr(DurationMs),
  gapCountByCause: Schema.Record(Schema.String, Count),
  recoveryActionCounts: Schema.Record(Schema.String, Count),
})
export type FullAutoRunLivenessMetrics = typeof FullAutoRunLivenessMetricsSchema.Type

export const FullAutoRunControlMetricsSchema = Schema.Struct({
  pauseRequestCount: Count,
  pauseResolvedCount: Count,
  pauseReliabilityRate: Rate,
  pauseLatenciesMs: Schema.Array(DurationMs),
  medianPauseLatencyMs: Schema.NullOr(DurationMs),
  ownerStopRequestCount: Count,
  /** Runs the state machine to `stopped` via a non-owner actor -- a safety
   * cap or failure limit, not an owner click. Always trivially "resolved"
   * (the state machine has no separate "stopping" phase), so this is a more
   * informative signal than a fabricated stop-latency number. */
  systemForcedStopCount: Count,
})
export type FullAutoRunControlMetrics = typeof FullAutoRunControlMetricsSchema.Type

export const FullAutoRunProviderMetricsSchema = Schema.Struct({
  transitionCount: Count,
  dispositionCounts: Schema.Record(Schema.String, Count),
  truncatedRate: Rate,
  refusedRate: Rate,
})
export type FullAutoRunProviderMetrics = typeof FullAutoRunProviderMetricsSchema.Type

export const FullAutoRunEvidenceMetricsSchema = Schema.Struct({
  verifiedRefCount: Count,
  claimedRefCount: Count,
  unverifiedClaimRate: Rate,
  unverifiedCompletionRisk: Schema.Boolean,
  noVerifiedRefs: Schema.Boolean,
  usageKnown: Schema.Boolean,
  costUsdKnown: Schema.Boolean,
  workspaceRefKnown: Schema.Boolean,
})
export type FullAutoRunEvidenceMetrics = typeof FullAutoRunEvidenceMetricsSchema.Type

export const FullAutoRunObjectiveMetricsSchema = Schema.Struct({
  revisionCount: Count,
  retained: Schema.Boolean,
})
export type FullAutoRunObjectiveMetrics = typeof FullAutoRunObjectiveMetricsSchema.Type

export const FullAutoRunAnalysisSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_ANALYSIS_SCHEMA),
  runRef: Ref,
  objectiveDigest: Schema.String,
  doneConditionDigest: Schema.String,
  state: Schema.String,
  turns: FullAutoRunTurnMetricsSchema,
  autonomy: FullAutoRunAutonomyMetricsSchema,
  liveness: FullAutoRunLivenessMetricsSchema,
  control: FullAutoRunControlMetricsSchema,
  provider: FullAutoRunProviderMetricsSchema,
  evidence: FullAutoRunEvidenceMetricsSchema,
  objective: FullAutoRunObjectiveMetricsSchema,
  successfulAttempts: Count,
  failedAttempts: Count,
  findings: Schema.Array(FullAutoRunFindingSchema).check(Schema.isMaxLength(1000)),
  analyzedAt: Schema.String,
})
export type FullAutoRunAnalysis = typeof FullAutoRunAnalysisSchema.Type
const decodeFullAutoRunAnalysis = Schema.decodeUnknownSync(FullAutoRunAnalysisSchema)

// -----------------------------------------------------------------------
// Metric derivation.
// -----------------------------------------------------------------------

const analyzeTurns = (turns: ReadonlyArray<FullAutoRunReportTurnEntry>): FullAutoRunTurnMetrics => {
  const total = turns.length
  const completed = turns.filter((turn) => turn.disposition === "completed").length
  const failed = turns.filter((turn) => turn.disposition === "failed").length
  const ownerInterrupted = turns.filter((turn) => turn.disposition === "owner_interrupted").length
  const restartInterrupted = turns.filter((turn) => turn.disposition === "interrupted_by_restart").length
  const restartResumed = turns.filter((turn) => turn.disposition === "resumed_after_restart").length
  const unresolved = turns.filter((turn) => turn.disposition === null).length
  const resolved = total - unresolved
  return {
    total,
    resolved,
    completed,
    failed,
    ownerInterrupted,
    restartInterrupted,
    restartResumed,
    unresolved,
    successfulTurnRate: resolved > 0 ? completed / resolved : null,
    usefulWorkDispositionRate: total > 0 ? completed / total : null,
  }
}

const analyzeAutonomy = (
  report: FullAutoRunReport,
  sortedTurns: ReadonlyArray<FullAutoRunReportTurnEntry>,
): FullAutoRunAutonomyMetrics => {
  const ownerActionTimes = report.ownerActions.map((action) => action.at).toSorted((left, right) => left.localeCompare(right))
  const streaks: Array<number> = []
  let current = 0
  let boundary = 0
  for (const turn of sortedTurns) {
    while (boundary < ownerActionTimes.length && ownerActionTimes[boundary]! <= turn.createdAt) {
      streaks.push(current)
      current = 0
      boundary++
    }
    current++
  }
  streaks.push(current)
  const nonZero = streaks.filter((streak) => streak > 0)

  const anchor = report.startedAt ?? report.createdAt
  const firstCompleted = sortedTurns.find((turn) => turn.disposition === "completed")

  return {
    autonomousStreaks: streaks,
    longestAutonomousStreak: nonZero.length > 0 ? Math.max(...nonZero) : 0,
    medianAutonomousStreak: median(nonZero),
    timeToFirstUsefulOutcomeKnown: firstCompleted !== undefined,
    timeToFirstUsefulOutcomeMs: firstCompleted === undefined ? null : durationOf(anchor, firstCompleted.createdAt),
  }
}

const analyzeLiveness = (
  report: FullAutoRunReport,
  now: () => Date,
): FullAutoRunLivenessMetrics => {
  const closed = report.livenessGaps.filter(
    (gap): gap is FullAutoRunReportLivenessGap & { durationMs: number } => gap.durationMs !== null,
  )
  const open = report.livenessGaps.find((gap) => gap.durationMs === null) ?? null
  const totalStalledMs = closed.reduce((sum, gap) => sum + gap.durationMs, 0)
  const longestClosedGapMs = closed.length > 0 ? Math.max(...closed.map((gap) => gap.durationMs)) : null

  const gapCountByCause: Record<string, number> = {}
  for (const gap of report.livenessGaps) {
    const key = gap.cause ?? "unknown_error"
    gapCountByCause[key] = (gapCountByCause[key] ?? 0) + 1
  }
  const recoveryActionCounts: Record<string, number> = {}
  for (const observation of report.livenessObservations) {
    recoveryActionCounts[observation.recoveryAction] = (recoveryActionCounts[observation.recoveryAction] ?? 0) + 1
  }

  return {
    gapCount: report.livenessGaps.length,
    closedGapCount: closed.length,
    totalStalledMs,
    ongoingGapEstimateMs: open === null ? null : durationOf(open.enteredAt, now().toISOString()),
    longestClosedGapMs,
    gapCountByCause: gapCountByCause as FullAutoRunLivenessMetrics["gapCountByCause"],
    recoveryActionCounts: recoveryActionCounts as FullAutoRunLivenessMetrics["recoveryActionCounts"],
  }
}

const analyzeControl = (report: FullAutoRunReport): FullAutoRunControlMetrics => {
  const sorted = [...report.lifecycleTransitions].toSorted((left, right) => left.at.localeCompare(right.at))
  const pauseLatenciesMs: Array<number> = []
  let pauseRequestCount = 0
  let pauseResolvedCount = 0
  let ownerStopRequestCount = 0
  let systemForcedStopCount = 0

  for (let index = 0; index < sorted.length; index++) {
    const transition = sorted[index]!
    if (transition.to === "pausing" && OWNER_DIRECTED_ACTORS.has(transition.actor as never)) {
      pauseRequestCount++
      const resolution = sorted[index + 1]
      if (resolution !== undefined && resolution.to === "paused") {
        pauseResolvedCount++
        pauseLatenciesMs.push(durationOf(transition.at, resolution.at))
      }
    }
    if (transition.to === "stopped") {
      if (OWNER_DIRECTED_ACTORS.has(transition.actor as never)) ownerStopRequestCount++
      else systemForcedStopCount++
    }
  }

  return {
    pauseRequestCount,
    pauseResolvedCount,
    pauseReliabilityRate: pauseRequestCount > 0 ? pauseResolvedCount / pauseRequestCount : null,
    pauseLatenciesMs,
    medianPauseLatencyMs: median(pauseLatenciesMs),
    ownerStopRequestCount,
    systemForcedStopCount,
  }
}

const analyzeProvider = (report: FullAutoRunReport): FullAutoRunProviderMetrics => {
  const transitionCount = report.providerTransitions.length
  const dispositionCounts: Record<string, number> = {}
  for (const transition of report.providerTransitions) {
    dispositionCounts[transition.disposition] = (dispositionCounts[transition.disposition] ?? 0) + 1
  }
  return {
    transitionCount,
    dispositionCounts: dispositionCounts as FullAutoRunProviderMetrics["dispositionCounts"],
    truncatedRate:
      transitionCount > 0 ? (dispositionCounts.truncated_with_confirmation ?? 0) / transitionCount : null,
    refusedRate: transitionCount > 0 ? (dispositionCounts.refused ?? 0) / transitionCount : null,
  }
}

const analyzeEvidence = (report: FullAutoRunReport): FullAutoRunEvidenceMetrics => {
  const verifiedRefCount = report.verifiedRefs.filter((ref) => ref.verification === "verified").length
  const claimedRefCount = report.verifiedRefs.filter((ref) => ref.verification === "claimed").length
  const totalRefs = verifiedRefCount + claimedRefCount
  return {
    verifiedRefCount,
    claimedRefCount,
    unverifiedClaimRate: totalRefs > 0 ? claimedRefCount / totalRefs : null,
    unverifiedCompletionRisk: report.state === "completed" && report.verifiedRefs.length === 0,
    noVerifiedRefs: report.verifiedRefs.length === 0,
    usageKnown: report.usage.totalTokensKnown,
    costUsdKnown: report.usage.costUsdKnown,
    workspaceRefKnown: report.workspaceRef !== undefined,
  }
}

const analyzeObjective = (report: FullAutoRunReport): FullAutoRunObjectiveMetrics => ({
  revisionCount: Math.max(0, report.objectiveRevisionCount - 1),
  retained: report.objectiveRevisionCount <= 1,
})

// -----------------------------------------------------------------------
// Findings derivation.
// -----------------------------------------------------------------------

const REPEATED_DISPOSITION_THRESHOLD = 3

const gapSeverity = (durationMs: number | null): FullAutoRunFindingSeverity => {
  if (durationMs === null) return "concerning" // still open -- always worth surfacing
  if (durationMs >= 60 * 60_000) return "concerning" // >= 1h
  if (durationMs >= 5 * 60_000) return "notable" // >= 5m
  return "info"
}

const deriveFindings = (
  report: FullAutoRunReport,
  sortedTurns: ReadonlyArray<FullAutoRunReportTurnEntry>,
  turnMetrics: FullAutoRunTurnMetrics,
  livenessMetrics: FullAutoRunLivenessMetrics,
  providerMetrics: FullAutoRunProviderMetrics,
  evidenceMetrics: FullAutoRunEvidenceMetrics,
  objectiveMetrics: FullAutoRunObjectiveMetrics,
  controlMetrics: FullAutoRunControlMetrics,
): ReadonlyArray<FullAutoRunFinding> => {
  const findings: Array<FullAutoRunFinding> = []

  for (const turn of sortedTurns) {
    if (turn.disposition === "completed") {
      findings.push({
        kind: "successful_packet",
        severity: "info",
        summary: `Turn ${turn.turnRef} completed on lane ${turn.lane}.`,
        evidenceRefs: [turn.turnRef],
        at: turn.updatedAt,
      })
    } else if (turn.disposition === "failed") {
      findings.push({
        kind: "failed_continuation",
        severity: "notable",
        summary: `Turn ${turn.turnRef} on lane ${turn.lane} failed.`,
        evidenceRefs: [turn.turnRef],
        at: turn.updatedAt,
      })
    }
  }

  for (const gap of report.livenessGaps) {
    findings.push({
      kind: "liveness_gap",
      severity: gapSeverity(gap.durationMs),
      summary:
        gap.durationMs === null
          ? `Liveness gap opened at ${gap.enteredAt} and is still open (cause: ${gap.cause ?? "unknown"}).`
          : `Liveness gap of ${gap.durationMs}ms (cause: ${gap.cause ?? "unknown"}).`,
      evidenceRefs: [],
      at: gap.enteredAt,
    })

    const diagnosedByOwner = report.ownerActions.some(
      (action) => action.at >= gap.enteredAt && (gap.exitedAt === null || action.at <= gap.exitedAt),
    )
    if (!diagnosedByOwner) {
      findings.push({
        kind: "missing_owner_diagnosis",
        severity: "concerning",
        summary: `No owner-attributed action was recorded during the liveness gap starting at ${gap.enteredAt}.`,
        evidenceRefs: [],
        at: gap.enteredAt,
      })
    }
  }

  // Repeated-disposition run-length encoding -- the typed proxy for
  // "repeated reconnaissance/setup" / "repeated verification" documented in
  // this module's header comment.
  let runDisposition: LocalTurnDisposition | null = null
  let runStart: FullAutoRunReportTurnEntry | null = null
  let runLast: FullAutoRunReportTurnEntry | null = null
  let runLength = 0
  const flushRun = (): void => {
    if (runDisposition !== null && runStart !== null && runLength >= REPEATED_DISPOSITION_THRESHOLD) {
      findings.push({
        kind: "repeated_disposition_pattern",
        severity: "notable",
        summary: `${runLength} consecutive turns resolved as "${runDisposition}" without an intervening owner action.`,
        evidenceRefs:
          runLast !== null && runLast.turnRef !== runStart.turnRef
            ? [runStart.turnRef, runLast.turnRef]
            : [runStart.turnRef],
        at: runStart.createdAt,
      })
    }
  }
  for (const turn of sortedTurns) {
    if (turn.disposition !== null && turn.disposition === runDisposition) {
      runLength++
      runLast = turn
    } else {
      flushRun()
      runDisposition = turn.disposition
      runStart = turn.disposition === null ? null : turn
      runLast = turn.disposition === null ? null : turn
      runLength = turn.disposition === null ? 0 : 1
    }
  }
  flushRun()

  for (const transition of report.providerTransitions) {
    if (transition.disposition === "truncated_with_confirmation") {
      findings.push({
        kind: "context_truncated",
        severity: "notable",
        summary: `Provider handoff ${transition.handoffRef} truncated context on ${transition.from} -> ${transition.to}.`,
        evidenceRefs: [transition.handoffRef],
        at: transition.at,
      })
    } else if (transition.disposition === "refused") {
      findings.push({
        kind: "provider_refused",
        severity: "concerning",
        summary: `Provider handoff ${transition.handoffRef} was refused (${transition.refusalReason ?? "unspecified"}).`,
        evidenceRefs: [transition.handoffRef],
        at: transition.at,
      })
    }
  }

  if (evidenceMetrics.unverifiedCompletionRisk) {
    findings.push({
      kind: "unverified_completion_risk",
      severity: "concerning",
      summary: "Run reached a completed state with zero verified or claimed refs.",
      evidenceRefs: [],
    })
  }
  if (evidenceMetrics.noVerifiedRefs && (turnMetrics.completed > 0 || report.successfulAttempts > 0)) {
    findings.push({
      kind: "missing_evidence",
      severity: "notable",
      summary: "The run recorded successful attempts but no verified or claimed refs are on file.",
      evidenceRefs: [],
    })
  }
  if (!evidenceMetrics.usageKnown) {
    findings.push({
      kind: "missing_evidence",
      severity: "info",
      summary: "Provider token usage is unknown for this run (not zero -- unmeasured).",
      evidenceRefs: [],
    })
  }

  if (!objectiveMetrics.retained) {
    findings.push({
      kind: "objective_drift_revision",
      severity: "notable",
      summary: `The objective/done-condition was revised ${objectiveMetrics.revisionCount} time(s) after run creation.`,
      evidenceRefs: [],
    })
  }

  if (controlMetrics.systemForcedStopCount > 0) {
    findings.push({
      kind: "system_forced_stop",
      severity: "concerning",
      summary: `The run was stopped by a non-owner actor ${controlMetrics.systemForcedStopCount} time(s) (safety cap or failure limit).`,
      evidenceRefs: [],
    })
  }

  const hasAnyConcern = findings.some(
    (finding) =>
      finding.kind === "failed_continuation" ||
      finding.kind === "liveness_gap" ||
      finding.kind === "missing_owner_diagnosis" ||
      finding.kind === "context_truncated" ||
      finding.kind === "provider_refused" ||
      finding.kind === "unverified_completion_risk" ||
      finding.kind === "missing_evidence" ||
      finding.kind === "system_forced_stop",
  )
  if (report.state === "completed" && !hasAnyConcern && turnMetrics.total > 0) {
    findings.push({
      kind: "clean_success",
      severity: "info",
      summary: `Run completed with ${turnMetrics.completed}/${turnMetrics.total} turns successful and no stall, truncation, refusal, or evidence gaps recorded.`,
      evidenceRefs: [],
    })
  }

  return findings
}

// -----------------------------------------------------------------------
// The top-level pure entry point.
// -----------------------------------------------------------------------

/** Pure derivation over one `FullAutoRunReport`. `now` is used ONLY to
 * estimate an ongoing (still-open) liveness gap's current duration; every
 * other metric is a deterministic function of the report's own recorded
 * timestamps. */
export const analyzeFullAutoRunReport = (
  report: FullAutoRunReport,
  now: () => Date = () => new Date(),
  /** HANDS-4 (#9175): OPTIONAL per-turn action taxonomy rows. When supplied
   * (an autonomy run), the analyzer adds a value-aware `low_value_churn`
   * finding derived from the real action signatures, not the disposition
   * proxy. Absent -> no churn finding, exactly the prior behavior. */
  actions?: ReadonlyArray<FullAutoTurnAction>,
): FullAutoRunAnalysis => {
  const sortedTurns = [...report.turns].toSorted((left, right) => left.createdAt.localeCompare(right.createdAt))
  const turns = analyzeTurns(report.turns)
  const autonomy = analyzeAutonomy(report, sortedTurns)
  const liveness = analyzeLiveness(report, now)
  const control = analyzeControl(report)
  const provider = analyzeProvider(report)
  const evidence = analyzeEvidence(report)
  const objective = analyzeObjective(report)
  const baseFindings = deriveFindings(report, sortedTurns, turns, liveness, provider, evidence, objective, control)
  const churn = actions === undefined ? { churn: false, consecutive: 0, signature: null } : detectFullAutoChurn({ actions })
  const findings: ReadonlyArray<FullAutoRunFinding> = churn.churn
    ? [
        ...baseFindings,
        {
          kind: "low_value_churn" as const,
          severity: "concerning" as const,
          summary: `${churn.consecutive} consecutive completed turns did the same non-advancing work (signature "${churn.signature}").`,
          evidenceRefs: (actions ?? []).slice(-churn.consecutive).map((action) => action.turnRef).slice(0, 50),
        },
      ]
    : baseFindings

  return decodeFullAutoRunAnalysis({
    schema: FULL_AUTO_RUN_ANALYSIS_SCHEMA,
    runRef: report.runRef,
    objectiveDigest: report.objectiveDigest,
    doneConditionDigest: report.doneConditionDigest,
    state: report.state,
    turns,
    autonomy,
    liveness,
    control,
    provider,
    evidence,
    objective,
    successfulAttempts: report.successfulAttempts,
    failedAttempts: report.failedAttempts,
    findings,
    analyzedAt: now().toISOString(),
  })
}

// -----------------------------------------------------------------------
// Named replay comparison.
// -----------------------------------------------------------------------

export const FullAutoRunComparisonMetricSchema = Schema.Struct({
  metric: Schema.String,
  baselineValue: Schema.NullOr(Schema.Number),
  candidateValue: Schema.NullOr(Schema.Number),
  deltaValue: Schema.NullOr(Schema.Number),
  direction: Schema.Literals(["improved", "regressed", "unchanged", "unknown"]),
})
export type FullAutoRunComparisonMetric = typeof FullAutoRunComparisonMetricSchema.Type

export const FullAutoRunComparisonRefusalReasonSchema = Schema.Literals([
  "objective_mismatch",
  "done_condition_mismatch",
])
export type FullAutoRunComparisonRefusalReason = typeof FullAutoRunComparisonRefusalReasonSchema.Type

export const FullAutoRunComparisonSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_COMPARISON_SCHEMA),
  comparable: Schema.Boolean,
  refusalReason: Schema.optional(FullAutoRunComparisonRefusalReasonSchema),
  baselineRunRef: Ref,
  candidateRunRef: Ref,
  /** Opaque, caller-supplied labels -- never looked up or enforced against a
   * `TestDefinition` registry, which does not exist yet (see module header). */
  testDefinitionRef: Schema.optional(Schema.String),
  sourceRevisionRange: Schema.optional(Schema.String),
  metrics: Schema.Array(FullAutoRunComparisonMetricSchema),
  baselineFindingCounts: Schema.Record(Schema.String, Count),
  candidateFindingCounts: Schema.Record(Schema.String, Count),
  recommendations: Schema.Array(Summary).check(Schema.isMaxLength(10)),
  comparedAt: Schema.String,
})
export type FullAutoRunComparison = typeof FullAutoRunComparisonSchema.Type
const decodeFullAutoRunComparison = Schema.decodeUnknownSync(FullAutoRunComparisonSchema)

/** `higherIsBetter: true` means an increase is `"improved"`. */
const RATE_METRIC_DIRECTIONS: ReadonlyArray<
  Readonly<{ metric: string; higherIsBetter: boolean; get: (analysis: FullAutoRunAnalysis) => number | null }>
> = [
  { metric: "successfulTurnRate", higherIsBetter: true, get: (a) => a.turns.successfulTurnRate },
  { metric: "usefulWorkDispositionRate", higherIsBetter: true, get: (a) => a.turns.usefulWorkDispositionRate },
  { metric: "longestAutonomousStreak", higherIsBetter: true, get: (a) => a.autonomy.longestAutonomousStreak },
  { metric: "medianAutonomousStreak", higherIsBetter: true, get: (a) => a.autonomy.medianAutonomousStreak },
  { metric: "totalStalledMs", higherIsBetter: false, get: (a) => a.liveness.totalStalledMs },
  { metric: "gapCount", higherIsBetter: false, get: (a) => a.liveness.gapCount },
  { metric: "unverifiedClaimRate", higherIsBetter: false, get: (a) => a.evidence.unverifiedClaimRate },
  { metric: "truncatedRate", higherIsBetter: false, get: (a) => a.provider.truncatedRate },
  { metric: "refusedRate", higherIsBetter: false, get: (a) => a.provider.refusedRate },
  { metric: "pauseReliabilityRate", higherIsBetter: true, get: (a) => a.control.pauseReliabilityRate },
]

const directionOf = (
  baselineValue: number | null,
  candidateValue: number | null,
  higherIsBetter: boolean,
): FullAutoRunComparisonMetric["direction"] => {
  if (baselineValue === null || candidateValue === null) return "unknown"
  if (baselineValue === candidateValue) return "unchanged"
  const improved = higherIsBetter ? candidateValue > baselineValue : candidateValue < baselineValue
  return improved ? "improved" : "regressed"
}

const countFindingsByKind = (
  findings: ReadonlyArray<FullAutoRunFinding>,
): Record<string, number> => {
  const counts: Record<string, number> = {}
  for (const finding of findings) counts[finding.kind] = (counts[finding.kind] ?? 0) + 1
  return counts
}

/** Compares two already-computed analyses. Refuses (rather than fabricates)
 * a comparison when the two runs' typed objective/done-condition identity
 * digests disagree -- the honest proxy for "not the same test" absent a
 * dedicated `TestDefinition` registry (see module header). */
export const compareFullAutoRunAnalyses = (
  input: Readonly<{
    baseline: Readonly<{ report: FullAutoRunReport; analysis: FullAutoRunAnalysis }>
    candidate: Readonly<{ report: FullAutoRunReport; analysis: FullAutoRunAnalysis }>
    testDefinitionRef?: string
    sourceRevisionRange?: string
    now?: () => Date
  }>,
): FullAutoRunComparison => {
  const now = input.now ?? (() => new Date())
  const { baseline, candidate } = input

  const refusalReason: FullAutoRunComparisonRefusalReason | undefined =
    baseline.report.objectiveDigest !== candidate.report.objectiveDigest
      ? "objective_mismatch"
      : baseline.report.doneConditionDigest !== candidate.report.doneConditionDigest
        ? "done_condition_mismatch"
        : undefined

  if (refusalReason !== undefined) {
    return decodeFullAutoRunComparison({
      schema: FULL_AUTO_RUN_COMPARISON_SCHEMA,
      comparable: false,
      refusalReason,
      baselineRunRef: baseline.report.runRef,
      candidateRunRef: candidate.report.runRef,
      ...(input.testDefinitionRef === undefined ? {} : { testDefinitionRef: input.testDefinitionRef }),
      ...(input.sourceRevisionRange === undefined ? {} : { sourceRevisionRange: input.sourceRevisionRange }),
      metrics: [],
      baselineFindingCounts: {},
      candidateFindingCounts: {},
      recommendations: [],
      comparedAt: now().toISOString(),
    })
  }

  const metrics: Array<FullAutoRunComparisonMetric> = RATE_METRIC_DIRECTIONS.map(({ metric, higherIsBetter, get }) => {
    const baselineValue = get(baseline.analysis)
    const candidateValue = get(candidate.analysis)
    return {
      metric,
      baselineValue,
      candidateValue,
      deltaValue: baselineValue === null || candidateValue === null ? null : candidateValue - baselineValue,
      direction: directionOf(baselineValue, candidateValue, higherIsBetter),
    }
  })

  const recommendations: Array<string> = []
  for (const metricResult of metrics) {
    if (metricResult.direction === "regressed") {
      recommendations.push(`Investigate regression in ${metricResult.metric} (candidate vs. baseline).`)
    }
  }
  if (candidate.analysis.control.systemForcedStopCount > baseline.analysis.control.systemForcedStopCount) {
    recommendations.push("Candidate run was force-stopped by a safety cap more often than the baseline.")
  }

  return decodeFullAutoRunComparison({
    schema: FULL_AUTO_RUN_COMPARISON_SCHEMA,
    comparable: true,
    baselineRunRef: baseline.report.runRef,
    candidateRunRef: candidate.report.runRef,
    ...(input.testDefinitionRef === undefined ? {} : { testDefinitionRef: input.testDefinitionRef }),
    ...(input.sourceRevisionRange === undefined ? {} : { sourceRevisionRange: input.sourceRevisionRange }),
    metrics,
    baselineFindingCounts: countFindingsByKind(baseline.analysis.findings),
    candidateFindingCounts: countFindingsByKind(candidate.analysis.findings),
    recommendations: recommendations.slice(0, 10),
    comparedAt: now().toISOString(),
  })
}

// -----------------------------------------------------------------------
// Optional model-assisted qualitative review -- a typed CONTRACT only. This
// module never calls a live provider; a caller must inject `invoke`.
// `armed` is a fixed `true` literal so a request cannot be constructed by
// accident, and the result is structurally advisory-only: it carries no
// field capable of mutating a prompt, run, issue, or release decision.
// -----------------------------------------------------------------------

export const FullAutoRunModelAssistedFindingCategorySchema = Schema.Literals([
  "packet_choice",
  "duplication",
  "drift",
  "unclear_ui_state",
  "missing_evidence",
  "false_completion",
])
export type FullAutoRunModelAssistedFindingCategory = typeof FullAutoRunModelAssistedFindingCategorySchema.Type

export const FullAutoRunModelAssistedFindingSchema = Schema.Struct({
  category: FullAutoRunModelAssistedFindingCategorySchema,
  summary: Summary,
  evidenceRefs: Schema.Array(Ref).check(Schema.isMaxLength(20)),
})
export type FullAutoRunModelAssistedFinding = typeof FullAutoRunModelAssistedFindingSchema.Type

export const FullAutoRunModelAssistedReviewRequestSchema = Schema.Struct({
  runRef: Ref,
  /** A pinned evaluator/prompt version -- never "latest"/unversioned. */
  evaluatorVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  /** Must be exactly `true` -- a request literally cannot decode without
   * explicit arming. */
  armed: Schema.Literal(true),
  costConsent: Schema.Struct({
    acknowledgedBy: FullAutoRunActorSchema,
    acknowledgedAt: Schema.String,
  }),
})
export type FullAutoRunModelAssistedReviewRequest = typeof FullAutoRunModelAssistedReviewRequestSchema.Type
const decodeFullAutoRunModelAssistedReviewRequest = Schema.decodeUnknownSync(
  FullAutoRunModelAssistedReviewRequestSchema,
)

export const FullAutoRunModelAssistedReviewResultSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_MODEL_ASSISTED_REVIEW_SCHEMA),
  runRef: Ref,
  evaluatorVersion: Schema.String,
  /** Fixed `true` -- structurally advice, never applied automatically. */
  advisory: Schema.Literal(true),
  costUsd: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  findings: Schema.Array(FullAutoRunModelAssistedFindingSchema).check(Schema.isMaxLength(50)),
  generatedAt: Schema.String,
})
export type FullAutoRunModelAssistedReviewResult = typeof FullAutoRunModelAssistedReviewResultSchema.Type
const decodeFullAutoRunModelAssistedReviewResult = Schema.decodeUnknownSync(
  FullAutoRunModelAssistedReviewResultSchema,
)

/** The caller-supplied provider call. This module has no knowledge of any
 * particular provider, prompt template, or API -- that binding, its cost
 * accounting, and its own consent UI are a separate, later piece of work. */
export type FullAutoRunModelAssistedInvoker = (
  request: FullAutoRunModelAssistedReviewRequest,
  report: FullAutoRunReport,
  analysis: FullAutoRunAnalysis,
) => Promise<Readonly<{ costUsd: number; findings: ReadonlyArray<FullAutoRunModelAssistedFinding> }>>

/** Never called automatically by `analyzeFullAutoRunReport` or anything else
 * in this module. A caller must construct an explicitly `armed: true`
 * request and supply `invoke`. The result cannot self-apply: it has no
 * field that changes a prompt, run, issue, or release/public-claim state --
 * it is data for a human or a separate, explicitly authorized surface to
 * act on. */
export const requestFullAutoRunModelAssistedReview = async (
  input: Readonly<{
    request: FullAutoRunModelAssistedReviewRequest
    report: FullAutoRunReport
    analysis: FullAutoRunAnalysis
    invoke: FullAutoRunModelAssistedInvoker
    now?: () => Date
  }>,
): Promise<FullAutoRunModelAssistedReviewResult> => {
  const request = decodeFullAutoRunModelAssistedReviewRequest(input.request)
  if (request.runRef !== input.report.runRef) {
    throw new Error(
      `model-assisted review request runRef (${request.runRef}) does not match the supplied report's runRef (${input.report.runRef})`,
    )
  }
  const now = input.now ?? (() => new Date())
  const outcome = await input.invoke(request, input.report, input.analysis)
  return decodeFullAutoRunModelAssistedReviewResult({
    schema: FULL_AUTO_RUN_MODEL_ASSISTED_REVIEW_SCHEMA,
    runRef: request.runRef,
    evaluatorVersion: request.evaluatorVersion,
    advisory: true,
    costUsd: outcome.costUsd,
    findings: outcome.findings,
    generatedAt: now().toISOString(),
  })
}
