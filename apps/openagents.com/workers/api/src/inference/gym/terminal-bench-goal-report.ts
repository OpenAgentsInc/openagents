import { Schema as S } from 'effect'

import {
  assertPylonGepaMetricCallPublicRefs,
} from '../../pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from '../../public-ref-format'
import {
  GLM_REAP_TERMINAL_BENCH_MODEL_ID,
} from './harbor-dispatch'
import {
  GymTerminalBenchComparisonReport,
  OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS,
  type GymTerminalBenchComparisonReport as GymTerminalBenchComparisonReportType,
  type GymTerminalBenchProfileComparisonRow,
} from './terminal-bench-comparison'
import {
  GymTerminalBenchKhalaOrchestrationReport,
  type GymTerminalBenchKhalaOrchestrationReport as GymTerminalBenchKhalaOrchestrationReportType,
} from './terminal-bench-khala-orchestration'

export const GYM_TERMINAL_BENCH_6253_GOAL_REPORT_SCHEMA =
  'openagents.gym.terminal_bench_6253_goal_report.v1'

export const GymTerminalBench6253GlmReplicationStatus = S.Literals([
  'replicated_at_or_above_claim',
  'honest_gap_documented',
  'blocked',
])
export type GymTerminalBench6253GlmReplicationStatus =
  typeof GymTerminalBench6253GlmReplicationStatus.Type

export const GymTerminalBench6253InferenceComparisonStatus = S.Literals([
  'comparison_table_ready',
  'blocked',
])
export type GymTerminalBench6253InferenceComparisonStatus =
  typeof GymTerminalBench6253InferenceComparisonStatus.Type

export const GymTerminalBench6253KhalaOutcomeStatus = S.Literals([
  'khala_beats_raw_baseline',
  'no_win_documented',
  'not_measured',
  'blocked',
])
export type GymTerminalBench6253KhalaOutcomeStatus =
  typeof GymTerminalBench6253KhalaOutcomeStatus.Type

const EvidenceOnlyAuthority = S.Struct({
  publicClaimAllowed: S.Literal(false),
  runtimePromotionAllowed: S.Literal(false),
  payoutAllowed: S.Literal(false),
  settlementAllowed: S.Literal(false),
  providerMutationAllowed: S.Literal(false),
})
type EvidenceOnlyAuthority = typeof EvidenceOnlyAuthority.Type

const evidenceOnlyAuthority: EvidenceOnlyAuthority = {
  publicClaimAllowed: false,
  runtimePromotionAllowed: false,
  payoutAllowed: false,
  settlementAllowed: false,
  providerMutationAllowed: false,
}

export const GymTerminalBench6253GlmReplication = S.Struct({
  status: GymTerminalBench6253GlmReplicationStatus,
  bestProfileRef: S.NullOr(S.String),
  fullDenominatorSolveRate: S.NullOr(S.Number),
  gapToClaimBps: S.NullOr(S.Number),
  replicationClaimSatisfied: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type GymTerminalBench6253GlmReplication =
  typeof GymTerminalBench6253GlmReplication.Type

export const GymTerminalBench6253InferenceComparison = S.Struct({
  status: GymTerminalBench6253InferenceComparisonStatus,
  comparedProfileRefs: S.Array(S.String),
  decisionGradeProfileRefs: S.Array(S.String),
  bestSolveRateProfileRef: S.NullOr(S.String),
  bestCostProfileRef: S.NullOr(S.String),
  rowCount: S.Number,
  decisionGradeRowCount: S.Number,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type GymTerminalBench6253InferenceComparison =
  typeof GymTerminalBench6253InferenceComparison.Type

export const GymTerminalBench6253KhalaOutcome = S.Struct({
  status: GymTerminalBench6253KhalaOutcomeStatus,
  orchestrationReportRef: S.String,
  policyProfileRef: S.String,
  primaryOutcome: S.String,
  beatsSolveRate: S.Boolean,
  beatsCostPerAcceptedOutcome: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type GymTerminalBench6253KhalaOutcome =
  typeof GymTerminalBench6253KhalaOutcome.Type

export const GymTerminalBench6253GoalReport = S.Struct({
  schemaVersion: S.Literal(GYM_TERMINAL_BENCH_6253_GOAL_REPORT_SCHEMA),
  reportRef: S.String,
  generatedAt: S.String,
  issueRef: S.Literal('github.openagents.issue.6253'),
  datasetRef: S.Literal('terminal-bench@2.0'),
  officialTotalTasks: S.Literal(OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS),
  sourceComparisonReportRef: S.String,
  sourceKhalaOrchestrationReportRef: S.String,
  glmReplication: GymTerminalBench6253GlmReplication,
  inferenceMethodComparison: GymTerminalBench6253InferenceComparison,
  khalaOrchestration: GymTerminalBench6253KhalaOutcome,
  acceptanceSatisfied: S.Boolean,
  decisionGrade: S.Boolean,
  publicSafe: S.Literal(true),
  rawArtifactsIncluded: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  authority: EvidenceOnlyAuthority,
})
export type GymTerminalBench6253GoalReport =
  typeof GymTerminalBench6253GoalReport.Type

export class GymTerminalBench6253GoalReportUnsafe extends S.TaggedErrorClass<GymTerminalBench6253GoalReportUnsafe>()(
  'GymTerminalBench6253GoalReportUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeComparisonReport = S.decodeUnknownSync(
  GymTerminalBenchComparisonReport,
)
const decodeKhalaReport = S.decodeUnknownSync(
  GymTerminalBenchKhalaOrchestrationReport,
)
const decodeGoalReport = S.decodeUnknownSync(GymTerminalBench6253GoalReport)

const assertPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const urlRef = normalized.find(ref => /^[a-z][a-z0-9+.-]*:\/\//i.test(ref))
  if (urlRef !== undefined) {
    throw new GymTerminalBench6253GoalReportUnsafe({
      reason: `${label} must contain public-safe refs, not raw URLs.`,
    })
  }
  try {
    assertPylonGepaMetricCallPublicRefs(label, normalized)
  } catch (error) {
    throw new GymTerminalBench6253GoalReportUnsafe({
      reason:
        error instanceof Error
          ? error.message
          : `${label} contains unsafe refs.`,
    })
  }
  return normalized
}

const refFor = (
  prefix: string,
  values: ReadonlyArray<string>,
  fallback: string,
): string =>
  `${prefix}.${publicRefSegment(values.join('.'), fallback)}`

const bestGlmDecisionRow = (
  rows: ReadonlyArray<GymTerminalBenchProfileComparisonRow>,
): GymTerminalBenchProfileComparisonRow | null =>
  rows
    .filter(
      row =>
        row.decisionGrade &&
        row.lane === 'glm-52' &&
        row.model === GLM_REAP_TERMINAL_BENCH_MODEL_ID &&
        row.officialFullTaskSet,
    )
    .sort((left, right) => {
      const solveDelta =
        (right.fullDenominatorSolveRate ?? -1) -
        (left.fullDenominatorSolveRate ?? -1)
      if (solveDelta !== 0) {
        return solveDelta
      }
      return left.profileRef.localeCompare(right.profileRef)
    })[0] ?? null

const buildGlmReplication = (
  comparisonReport: GymTerminalBenchComparisonReportType,
): GymTerminalBench6253GlmReplication => {
  const bestRow = bestGlmDecisionRow(comparisonReport.rows)
  const blockerRefs = uniqueRefs([
    ...(!comparisonReport.decisionGrade
      ? ['blocker.gym.terminal_bench.6253.comparison_not_decision_grade']
      : []),
    ...(bestRow === null
      ? ['blocker.gym.terminal_bench.6253.glm_decision_row_missing']
      : []),
    ...comparisonReport.blockers,
  ])
  const replicationClaimSatisfied =
    bestRow?.replicationClaimSatisfied === true &&
    comparisonReport.replicationClaimSatisfied
  const status: GymTerminalBench6253GlmReplicationStatus =
    blockerRefs.length > 0
      ? 'blocked'
      : replicationClaimSatisfied
        ? 'replicated_at_or_above_claim'
        : 'honest_gap_documented'

  return {
    status,
    bestProfileRef: bestRow?.profileRef ?? null,
    fullDenominatorSolveRate: bestRow?.fullDenominatorSolveRate ?? null,
    gapToClaimBps: bestRow?.gapToClaimBps ?? null,
    replicationClaimSatisfied,
    blockerRefs,
    caveatRefs: uniqueRefs([
      ...comparisonReport.caveats,
      ...(bestRow?.caveats ?? []),
    ]),
  }
}

const buildInferenceComparison = (
  comparisonReport: GymTerminalBenchComparisonReportType,
): GymTerminalBench6253InferenceComparison => {
  const decisionRows = comparisonReport.rows.filter(row => row.decisionGrade)
  const comparedProfileRefs = uniqueRefs(
    comparisonReport.rows.map(row => row.profileRef),
  )
  const decisionGradeProfileRefs = uniqueRefs(
    decisionRows.map(row => row.profileRef),
  )
  const blockerRefs = uniqueRefs([
    ...(comparedProfileRefs.length < 2
      ? ['blocker.gym.terminal_bench.6253.inference_method_rows_required']
      : []),
    ...(decisionGradeProfileRefs.length < 2
      ? [
          'blocker.gym.terminal_bench.6253.inference_method_decision_rows_required',
        ]
      : []),
    ...comparisonReport.blockers,
  ])

  return {
    status: blockerRefs.length === 0 ? 'comparison_table_ready' : 'blocked',
    comparedProfileRefs,
    decisionGradeProfileRefs,
    bestSolveRateProfileRef: comparisonReport.bestDecisionGradeSolveRateProfileRef,
    bestCostProfileRef: comparisonReport.bestDecisionGradeCostProfileRef,
    rowCount: comparisonReport.rows.length,
    decisionGradeRowCount: decisionRows.length,
    blockerRefs,
    caveatRefs: comparisonReport.caveats,
  }
}

const khalaStatus = (
  khalaReport: GymTerminalBenchKhalaOrchestrationReportType,
): GymTerminalBench6253KhalaOutcomeStatus => {
  if (!khalaReport.decisionGrade || khalaReport.blockerRefs.length > 0) {
    return 'blocked'
  }
  if (khalaReport.beatsSolveRate || khalaReport.beatsCostPerAcceptedOutcome) {
    return 'khala_beats_raw_baseline'
  }
  if (khalaReport.primaryOutcome === 'no_win') {
    return 'no_win_documented'
  }
  return 'not_measured'
}

const buildKhalaOutcome = (
  khalaReport: GymTerminalBenchKhalaOrchestrationReportType,
): GymTerminalBench6253KhalaOutcome => ({
  status: khalaStatus(khalaReport),
  orchestrationReportRef: khalaReport.reportRef,
  policyProfileRef: khalaReport.policyProfile.policyProfileRef,
  primaryOutcome: khalaReport.primaryOutcome,
  beatsSolveRate: khalaReport.beatsSolveRate,
  beatsCostPerAcceptedOutcome: khalaReport.beatsCostPerAcceptedOutcome,
  blockerRefs: khalaReport.blockerRefs,
  caveatRefs: khalaReport.caveatRefs,
})

const acceptanceSatisfied = (input: {
  glm: GymTerminalBench6253GlmReplication
  inference: GymTerminalBench6253InferenceComparison
  khala: GymTerminalBench6253KhalaOutcome
}): boolean =>
  (input.glm.status === 'replicated_at_or_above_claim' ||
    input.glm.status === 'honest_gap_documented') &&
  input.inference.status === 'comparison_table_ready' &&
  (input.khala.status === 'khala_beats_raw_baseline' ||
    input.khala.status === 'no_win_documented')

export const buildGymTerminalBench6253GoalReport = (input: {
  generatedAt: string
  comparisonReport: unknown
  khalaOrchestrationReport: unknown
  evidenceRefs?: ReadonlyArray<string> | undefined
}): GymTerminalBench6253GoalReport => {
  const comparisonReport = decodeComparisonReport(input.comparisonReport)
  const khalaReport = decodeKhalaReport(input.khalaOrchestrationReport)
  if (khalaReport.rawBaselineReportRef !== comparisonReport.reportRef) {
    throw new GymTerminalBench6253GoalReportUnsafe({
      reason:
        'Issue #6253 goal report requires the Khala orchestration raw baseline to reference the supplied Terminal-Bench comparison report.',
    })
  }
  const glmReplication = buildGlmReplication(comparisonReport)
  const inferenceMethodComparison = buildInferenceComparison(comparisonReport)
  const khalaOrchestration = buildKhalaOutcome(khalaReport)
  const blockerRefs = uniqueRefs([
    ...glmReplication.blockerRefs,
    ...inferenceMethodComparison.blockerRefs,
    ...khalaOrchestration.blockerRefs,
    ...(khalaOrchestration.status === 'not_measured'
      ? ['blocker.gym.terminal_bench.6253.khala_outcome_not_measured']
      : []),
  ])
  const satisfied = acceptanceSatisfied({
    glm: glmReplication,
    inference: inferenceMethodComparison,
    khala: khalaOrchestration,
  })
  const reportRef = refFor(
    'report.gym.terminal_bench.issue_6253',
    [comparisonReport.reportRef, khalaReport.reportRef],
    'report',
  )

  return decodeGoalReport({
    schemaVersion: GYM_TERMINAL_BENCH_6253_GOAL_REPORT_SCHEMA,
    reportRef,
    generatedAt: input.generatedAt,
    issueRef: 'github.openagents.issue.6253',
    datasetRef: 'terminal-bench@2.0',
    officialTotalTasks: OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS,
    sourceComparisonReportRef: comparisonReport.reportRef,
    sourceKhalaOrchestrationReportRef: khalaReport.reportRef,
    glmReplication,
    inferenceMethodComparison,
    khalaOrchestration,
    acceptanceSatisfied: satisfied,
    decisionGrade: satisfied && blockerRefs.length === 0,
    publicSafe: true,
    rawArtifactsIncluded: false,
    evidenceRefs: assertPublicRefs(
      'Terminal-Bench #6253 goal report evidence refs',
      [
        comparisonReport.reportRef,
        khalaReport.reportRef,
        ...comparisonReport.rows.flatMap(row => row.evidenceRefs),
        ...khalaReport.evidenceRefs,
        ...(input.evidenceRefs ?? []),
      ],
    ),
    blockerRefs,
    caveatRefs: uniqueRefs([
      ...glmReplication.caveatRefs,
      ...inferenceMethodComparison.caveatRefs,
      ...khalaOrchestration.caveatRefs,
      'caveat.gym.terminal_bench.issue_6253_goal_report_public_safe_summary_only',
      'caveat.gym.terminal_bench.issue_6253_no_public_claim_authority',
      ...(khalaOrchestration.status === 'no_win_documented'
        ? ['caveat.gym.terminal_bench.issue_6253_khala_no_win_documented']
        : []),
    ]),
    authority: evidenceOnlyAuthority,
  })
}
