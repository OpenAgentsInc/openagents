import { Schema as S } from 'effect'

import {
  BenchmarkLane as BenchmarkLaneSchema,
  type BenchmarkLane,
} from '../benchmark'
import {
  assertPylonGepaMetricCallPublicRefs,
} from '../../pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from '../../public-ref-format'
import {
  CoordinatorCandidateRef,
  GymFanoutMode,
  GymSamplingSettings,
  GymToolSetRef,
  ProgramSignatureComposition,
  TERMINAL_BENCH_GYM_EXPERIMENT,
  type CoordinatorCandidateRef as CoordinatorCandidateRefType,
  type GymExperiment,
  type GymFanoutMode as GymFanoutModeType,
} from './experiment'
import {
  GymTrainingConsumer,
  type GymTrainingConsumer as GymTrainingConsumerType,
} from './flywheel'
import {
  GLM_REAP_TERMINAL_BENCH_MODEL_ID,
  KHALA_PUBLIC_MODEL_ID,
} from './harbor-dispatch'
import {
  GymTerminalBenchComparisonReport,
  OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS,
  type GymTerminalBenchComparisonReport as GymTerminalBenchComparisonReportType,
  type GymTerminalBenchProfileComparisonRow,
} from './terminal-bench-comparison'

export const GYM_TERMINAL_BENCH_KHALA_POLICY_PROFILE_SCHEMA =
  'openagents.gym.terminal_bench_khala_policy_profile.v1'
export const GYM_TERMINAL_BENCH_KHALA_ORCHESTRATION_REPORT_SCHEMA =
  'openagents.gym.terminal_bench_khala_orchestration_report.v1'
export const GYM_TERMINAL_BENCH_KHALA_FLYWHEEL_PROJECTION_SCHEMA =
  'openagents.gym.terminal_bench_khala_flywheel_projection.v1'

export const GymTerminalBenchKhalaPolicyProfileRef = S.Literals([
  'khala-terminal-bench-heuristic-v0',
  'khala-terminal-bench-trinity-v1',
  'khala-terminal-bench-conductor-v2',
])
export type GymTerminalBenchKhalaPolicyProfileRef =
  typeof GymTerminalBenchKhalaPolicyProfileRef.Type

export const GymTerminalBenchKhalaComparisonOutcome = S.Literals([
  'beats_on_solve_rate',
  'beats_on_cost_per_accepted_outcome',
  'no_win',
  'blocked',
  'not_measured',
])
export type GymTerminalBenchKhalaComparisonOutcome =
  typeof GymTerminalBenchKhalaComparisonOutcome.Type

export const GymTerminalBenchKhalaFlywheelState = S.Literals([
  'ready_for_training',
  'blocked',
])
export type GymTerminalBenchKhalaFlywheelState =
  typeof GymTerminalBenchKhalaFlywheelState.Type

const EvidenceOnlyAuthority = S.Struct({
  publicClaimAllowed: S.Literal(false),
  runtimePromotionAllowed: S.Literal(false),
  payoutAllowed: S.Literal(false),
  settlementAllowed: S.Literal(false),
  providerMutationAllowed: S.Literal(false),
})
export type EvidenceOnlyAuthority = typeof EvidenceOnlyAuthority.Type

const evidenceOnlyAuthority: EvidenceOnlyAuthority = {
  publicClaimAllowed: false,
  runtimePromotionAllowed: false,
  payoutAllowed: false,
  settlementAllowed: false,
  providerMutationAllowed: false,
}

export const GymTerminalBenchKhalaFanoutProfile = S.Struct({
  mode: GymFanoutMode,
  lanes: S.Array(BenchmarkLaneSchema),
  concurrency: S.Number,
  bestOfN: S.Number,
  verifierPick: S.Boolean,
})
export type GymTerminalBenchKhalaFanoutProfile =
  typeof GymTerminalBenchKhalaFanoutProfile.Type

export const GymTerminalBenchKhalaPolicyProfile = S.Struct({
  schemaVersion: S.Literal(GYM_TERMINAL_BENCH_KHALA_POLICY_PROFILE_SCHEMA),
  policyProfileRef: GymTerminalBenchKhalaPolicyProfileRef,
  label: S.String,
  coordinator: CoordinatorCandidateRef,
  fanout: GymTerminalBenchKhalaFanoutProfile,
  tools: GymToolSetRef,
  modules: ProgramSignatureComposition,
  sampling: GymSamplingSettings,
  serving: S.Struct({
    quantization: S.Struct({
      mode: S.Literals(['none', 'int8', 'fp8', 'nf4']),
      engineRef: S.optional(S.String),
    }),
    speculation: S.Struct({
      mode: S.Literals(['none', 'eagle', 'medusa', 'ngram']),
      draftModelRef: S.optional(S.String),
    }),
  }),
  trainingConsumers: S.Array(GymTrainingConsumer),
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  publicSafe: S.Literal(true),
  rawArtifactsIncluded: S.Literal(false),
})
export type GymTerminalBenchKhalaPolicyProfile =
  typeof GymTerminalBenchKhalaPolicyProfile.Type

export const GymTerminalBenchKhalaSolveRateComparison = S.Struct({
  outcome: GymTerminalBenchKhalaComparisonOutcome,
  rawBaselineSolveRate: S.NullOr(S.Number),
  khalaSolveRate: S.NullOr(S.Number),
  deltaBps: S.NullOr(S.Number),
})
export type GymTerminalBenchKhalaSolveRateComparison =
  typeof GymTerminalBenchKhalaSolveRateComparison.Type

export const GymTerminalBenchKhalaCostComparison = S.Struct({
  outcome: GymTerminalBenchKhalaComparisonOutcome,
  rawBaselineCostPerAcceptedOutcomeMsat: S.NullOr(S.Number),
  khalaCostPerAcceptedOutcomeMsat: S.NullOr(S.Number),
  improvementBps: S.NullOr(S.Number),
})
export type GymTerminalBenchKhalaCostComparison =
  typeof GymTerminalBenchKhalaCostComparison.Type

export const GymTerminalBenchKhalaFlywheelProjection = S.Struct({
  schemaVersion: S.Literal(GYM_TERMINAL_BENCH_KHALA_FLYWHEEL_PROJECTION_SCHEMA),
  projectionRef: S.String,
  reportRef: S.String,
  policyProfileRef: GymTerminalBenchKhalaPolicyProfileRef,
  state: GymTerminalBenchKhalaFlywheelState,
  consumers: S.Array(GymTrainingConsumer),
  rewardBundleRefs: S.Array(S.String),
  leaderboardProjectionRefs: S.Array(S.String),
  flywheelEvaluationRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  authority: EvidenceOnlyAuthority,
})
export type GymTerminalBenchKhalaFlywheelProjection =
  typeof GymTerminalBenchKhalaFlywheelProjection.Type

export const GymTerminalBenchKhalaOrchestrationReport = S.Struct({
  schemaVersion: S.Literal(
    GYM_TERMINAL_BENCH_KHALA_ORCHESTRATION_REPORT_SCHEMA,
  ),
  reportRef: S.String,
  generatedAt: S.String,
  datasetRef: S.Literal('terminal-bench@2.0'),
  rawBaselineReportRef: S.String,
  khalaReportRef: S.String,
  rawBaselineProfileRef: S.String,
  khalaProfileRef: S.String,
  rawBaselineAttribution: S.String,
  khalaAttribution: S.String,
  policyProfile: GymTerminalBenchKhalaPolicyProfile,
  primaryOutcome: GymTerminalBenchKhalaComparisonOutcome,
  outcomes: S.Array(GymTerminalBenchKhalaComparisonOutcome),
  beatsSolveRate: S.Boolean,
  beatsCostPerAcceptedOutcome: S.Boolean,
  solveRateComparison: GymTerminalBenchKhalaSolveRateComparison,
  costComparison: GymTerminalBenchKhalaCostComparison,
  decisionGrade: S.Boolean,
  publicSafe: S.Literal(true),
  rawArtifactsIncluded: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  flywheelProjection: GymTerminalBenchKhalaFlywheelProjection,
  authority: EvidenceOnlyAuthority,
})
export type GymTerminalBenchKhalaOrchestrationReport =
  typeof GymTerminalBenchKhalaOrchestrationReport.Type

export class GymTerminalBenchKhalaOrchestrationUnsafe extends S.TaggedErrorClass<GymTerminalBenchKhalaOrchestrationUnsafe>()(
  'GymTerminalBenchKhalaOrchestrationUnsafe',
  {
    reason: S.String,
  },
) {}

const TRAINING_CONSUMERS: ReadonlyArray<GymTrainingConsumerType> = [
  'gepa',
  'trinity',
  'conductor',
]

const decodePolicyProfile = S.decodeUnknownSync(
  GymTerminalBenchKhalaPolicyProfile,
)
const decodePolicyProfileRef = S.decodeUnknownSync(
  GymTerminalBenchKhalaPolicyProfileRef,
)
const decodeComparisonReport = S.decodeUnknownSync(
  GymTerminalBenchComparisonReport,
)
const decodeOrchestrationReport = S.decodeUnknownSync(
  GymTerminalBenchKhalaOrchestrationReport,
)
const decodeFlywheelProjection = S.decodeUnknownSync(
  GymTerminalBenchKhalaFlywheelProjection,
)

const assertPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  try {
    assertPylonGepaMetricCallPublicRefs(label, normalized)
  } catch (error) {
    throw new GymTerminalBenchKhalaOrchestrationUnsafe({
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

const uniqueOutcomes = (
  outcomes: ReadonlyArray<GymTerminalBenchKhalaComparisonOutcome>,
): ReadonlyArray<GymTerminalBenchKhalaComparisonOutcome> =>
  Array.from(new Set(outcomes))

const bpsDelta = (left: number | null, right: number | null): number | null =>
  left === null || right === null ? null : Math.round((right - left) * 10_000)

const costImprovementBps = (
  rawBaselineCost: number | null,
  khalaCost: number | null,
): number | null => {
  if (
    rawBaselineCost === null ||
    khalaCost === null ||
    rawBaselineCost <= 0
  ) {
    return null
  }
  return Math.round(((rawBaselineCost - khalaCost) / rawBaselineCost) * 10_000)
}

const policyProfile = (input: {
  policyProfileRef: GymTerminalBenchKhalaPolicyProfileRef
  label: string
  coordinator: CoordinatorCandidateRefType
  mode: GymFanoutModeType
  lanes: ReadonlyArray<BenchmarkLane>
  concurrency: number
  bestOfN: number
  verifierPick: boolean
  sourceRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
}): GymTerminalBenchKhalaPolicyProfile =>
  decodePolicyProfile({
    schemaVersion: GYM_TERMINAL_BENCH_KHALA_POLICY_PROFILE_SCHEMA,
    policyProfileRef: input.policyProfileRef,
    label: input.label,
    coordinator: input.coordinator,
    fanout: {
      mode: input.mode,
      lanes: input.lanes,
      concurrency: input.concurrency,
      bestOfN: input.bestOfN,
      verifierPick: input.verifierPick,
    },
    tools: 'khala-code-tools',
    modules: {
      mode: input.coordinator === 'heuristic-v0' ? 'none' : 'starter-catalog',
      signatureRefs:
        input.coordinator === 'heuristic-v0'
          ? []
          : [`program-signature.${input.policyProfileRef}`],
      moduleRefs:
        input.coordinator === 'heuristic-v0'
          ? []
          : [`module.${input.policyProfileRef}.router_policy`],
    },
    sampling: {
      temperature: 0.2,
      reasoningEffort: 'off',
      maxTokens: 8192,
      transport: 'streaming',
    },
    serving: {
      quantization: { mode: 'none' },
      speculation: { mode: 'ngram', draftModelRef: 'glm-52.mtp2' },
    },
    trainingConsumers: TRAINING_CONSUMERS,
    sourceRefs: input.sourceRefs,
    caveatRefs: input.caveatRefs,
    publicSafe: true,
    rawArtifactsIncluded: false,
  })

export const TERMINAL_BENCH_KHALA_POLICY_PROFILES: Readonly<
  Record<
    GymTerminalBenchKhalaPolicyProfileRef,
    GymTerminalBenchKhalaPolicyProfile
  >
> = {
  'khala-terminal-bench-heuristic-v0': policyProfile({
    policyProfileRef: 'khala-terminal-bench-heuristic-v0',
    label: 'Khala Terminal-Bench heuristic router',
    coordinator: 'heuristic-v0',
    mode: 'verifier-pick',
    lanes: ['khala', 'glm-52'],
    concurrency: 2,
    bestOfN: 2,
    verifierPick: true,
    sourceRefs: [
      'source.gym.terminal_bench.policy.heuristic_v0',
      'source.gym.terminal_bench.glm_reap_profile_catalog',
    ],
    caveatRefs: [
      'caveat.gym.terminal_bench.khala_policy_fixture_until_owner_armed',
    ],
  }),
  'khala-terminal-bench-trinity-v1': policyProfile({
    policyProfileRef: 'khala-terminal-bench-trinity-v1',
    label: 'Khala Terminal-Bench TRINITY best-of-N router',
    coordinator: 'trinity-v1',
    mode: 'best-of-n',
    lanes: ['khala', 'glm-52', 'gpt-oss-120b', 'vertex-gemini', 'fireworks'],
    concurrency: 5,
    bestOfN: 5,
    verifierPick: true,
    sourceRefs: [
      'source.gym.terminal_bench.policy.trinity_v1',
      'source.gym.flywheel.trinity_rollout_reward',
    ],
    caveatRefs: [
      'caveat.gym.terminal_bench.best_of_n_cost_must_beat_raw_baseline',
    ],
  }),
  'khala-terminal-bench-conductor-v2': policyProfile({
    policyProfileRef: 'khala-terminal-bench-conductor-v2',
    label: 'Khala Terminal-Bench Conductor verifier-pick router',
    coordinator: 'conductor-v2',
    mode: 'verifier-pick',
    lanes: [
      'khala',
      'glm-52',
      'gpt-oss-20b',
      'gpt-oss-120b',
      'vertex-gemini',
      'fireworks',
    ],
    concurrency: 6,
    bestOfN: 6,
    verifierPick: true,
    sourceRefs: [
      'source.gym.terminal_bench.policy.conductor_v2',
      'source.gym.flywheel.conductor_runtime_candidate',
    ],
    caveatRefs: [
      'caveat.gym.terminal_bench.conductor_candidate_shadow_only',
    ],
  }),
}

export const resolveGymTerminalBenchKhalaPolicyProfile = (
  profileRef: unknown,
): GymTerminalBenchKhalaPolicyProfile => {
  const ref = decodePolicyProfileRef(profileRef)
  return decodePolicyProfile(TERMINAL_BENCH_KHALA_POLICY_PROFILES[ref])
}

export const buildGymTerminalBenchKhalaPolicyExperiment = (
  profileRef: GymTerminalBenchKhalaPolicyProfileRef,
): GymExperiment => {
  const profile = resolveGymTerminalBenchKhalaPolicyProfile(profileRef)
  return {
    ...TERMINAL_BENCH_GYM_EXPERIMENT,
    id: `gym-${profile.policyProfileRef}`,
    policy: {
      coordinator: profile.coordinator,
      fanout: {
        lanes: profile.fanout.lanes,
        mode: profile.fanout.mode,
        concurrency: profile.fanout.concurrency,
      },
      tools: profile.tools,
      modules: profile.modules,
      sampling: profile.sampling,
      serving: profile.serving,
    },
    budget: {
      spendCapMsat: 0,
      maxBillableSamples: 0,
      seam: 'fixture',
    },
  }
}

const bestRow = (
  rows: ReadonlyArray<GymTerminalBenchProfileComparisonRow>,
): GymTerminalBenchProfileComparisonRow | null =>
  [...rows]
    .sort((left, right) => {
      if (left.decisionGrade !== right.decisionGrade) {
        return left.decisionGrade ? -1 : 1
      }
      const solveDelta =
        (right.fullDenominatorSolveRate ?? -1) -
        (left.fullDenominatorSolveRate ?? -1)
      if (solveDelta !== 0) {
        return solveDelta
      }
      const leftCost =
        left.costPerAcceptedOutcomeMsat ?? Number.MAX_SAFE_INTEGER
      const rightCost =
        right.costPerAcceptedOutcomeMsat ?? Number.MAX_SAFE_INTEGER
      const costDelta = leftCost - rightCost
      if (costDelta !== 0) {
        return costDelta
      }
      return left.profileRef.localeCompare(right.profileRef)
    })[0] ?? null

const selectRawBaselineRow = (
  report: GymTerminalBenchComparisonReportType,
): GymTerminalBenchProfileComparisonRow | null =>
  bestRow(
    report.rows.filter(
      row => row.lane === 'glm-52' && row.model === GLM_REAP_TERMINAL_BENCH_MODEL_ID,
    ),
  )

const selectKhalaRow = (
  report: GymTerminalBenchComparisonReportType,
): GymTerminalBenchProfileComparisonRow | null =>
  bestRow(
    report.rows.filter(
      row => row.lane === 'khala' && row.model === KHALA_PUBLIC_MODEL_ID,
    ),
  )

const rowBlockers = (
  label: 'raw_baseline' | 'khala',
  row: GymTerminalBenchProfileComparisonRow | null,
): ReadonlyArray<string> => {
  if (row === null) {
    return [`blocker.gym.terminal_bench.${label}_row_missing`]
  }
  return uniqueRefs([
    ...(!row.decisionGrade
      ? [`blocker.gym.terminal_bench.${label}_not_decision_grade`]
      : []),
    ...(!row.officialFullTaskSet ||
    row.totalTasks !== OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS
      ? [`blocker.gym.terminal_bench.${label}_official_full_task_set_required`]
      : []),
    ...row.blockers,
  ])
}

const reportBlockers = (
  label: 'raw_baseline' | 'khala',
  report: GymTerminalBenchComparisonReportType,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...(!report.decisionGrade
      ? [`blocker.gym.terminal_bench.${label}_report_not_decision_grade`]
      : []),
    ...report.blockers,
  ])

const solveRateComparison = (
  rawBaseline: GymTerminalBenchProfileComparisonRow,
  khala: GymTerminalBenchProfileComparisonRow,
  blocked: boolean,
): GymTerminalBenchKhalaSolveRateComparison => {
  const deltaBps = bpsDelta(
    rawBaseline.fullDenominatorSolveRate,
    khala.fullDenominatorSolveRate,
  )
  const outcome: GymTerminalBenchKhalaComparisonOutcome = blocked
    ? 'blocked'
    : deltaBps === null
      ? 'not_measured'
      : deltaBps > 0
        ? 'beats_on_solve_rate'
        : 'no_win'
  return {
    outcome,
    rawBaselineSolveRate: rawBaseline.fullDenominatorSolveRate,
    khalaSolveRate: khala.fullDenominatorSolveRate,
    deltaBps,
  }
}

const costComparison = (
  rawBaseline: GymTerminalBenchProfileComparisonRow,
  khala: GymTerminalBenchProfileComparisonRow,
  blocked: boolean,
): GymTerminalBenchKhalaCostComparison => {
  const improvementBps = costImprovementBps(
    rawBaseline.costPerAcceptedOutcomeMsat,
    khala.costPerAcceptedOutcomeMsat,
  )
  const outcome: GymTerminalBenchKhalaComparisonOutcome = blocked
    ? 'blocked'
    : improvementBps === null
      ? 'not_measured'
      : improvementBps > 0
        ? 'beats_on_cost_per_accepted_outcome'
        : 'no_win'
  return {
    outcome,
    rawBaselineCostPerAcceptedOutcomeMsat:
      rawBaseline.costPerAcceptedOutcomeMsat,
    khalaCostPerAcceptedOutcomeMsat: khala.costPerAcceptedOutcomeMsat,
    improvementBps,
  }
}

const primaryOutcome = (
  input: Readonly<{
    blocked: boolean
    solve: GymTerminalBenchKhalaSolveRateComparison
    cost: GymTerminalBenchKhalaCostComparison
  }>,
): GymTerminalBenchKhalaComparisonOutcome => {
  if (input.blocked) {
    return 'blocked'
  }
  if (input.solve.outcome === 'beats_on_solve_rate') {
    return 'beats_on_solve_rate'
  }
  if (input.cost.outcome === 'beats_on_cost_per_accepted_outcome') {
    return 'beats_on_cost_per_accepted_outcome'
  }
  if (
    input.solve.outcome === 'not_measured' ||
    input.cost.outcome === 'not_measured'
  ) {
    return 'not_measured'
  }
  return 'no_win'
}

const outcomesFor = (
  input: Readonly<{
    primary: GymTerminalBenchKhalaComparisonOutcome
    solve: GymTerminalBenchKhalaSolveRateComparison
    cost: GymTerminalBenchKhalaCostComparison
  }>,
): ReadonlyArray<GymTerminalBenchKhalaComparisonOutcome> => {
  if (input.primary === 'blocked') {
    return ['blocked']
  }
  const wins = uniqueOutcomes([input.solve.outcome, input.cost.outcome]).filter(
    outcome =>
      outcome === 'beats_on_solve_rate' ||
      outcome === 'beats_on_cost_per_accepted_outcome',
  )
  return wins.length > 0 ? wins : [input.primary]
}

const flywheelBlockers = (input: {
  decisionGrade: boolean
  hasKhalaWin: boolean
  rewardBundleRefs: ReadonlyArray<string>
}): ReadonlyArray<string> =>
  uniqueRefs([
    ...(!input.decisionGrade
      ? ['blocker.gym.terminal_bench.khala_orchestration_not_decision_grade']
      : []),
    ...(!input.hasKhalaWin
      ? ['blocker.gym.terminal_bench.khala_orchestration_no_win']
      : []),
    ...(input.rewardBundleRefs.length === 0
      ? ['blocker.gym.terminal_bench.khala_orchestration_reward_bundle_missing']
      : []),
  ])

export const buildGymTerminalBenchKhalaFlywheelProjection = (input: {
  reportRef: string
  policyProfileRef: GymTerminalBenchKhalaPolicyProfileRef
  decisionGrade: boolean
  hasKhalaWin: boolean
  rewardBundleRefs?: ReadonlyArray<string> | undefined
  leaderboardProjectionRefs?: ReadonlyArray<string> | undefined
  flywheelEvaluationRefs?: ReadonlyArray<string> | undefined
}): GymTerminalBenchKhalaFlywheelProjection => {
  const rewardBundleRefs = assertPublicRefs(
    'Terminal-Bench Khala reward bundle refs',
    input.rewardBundleRefs ?? [],
  )
  const leaderboardProjectionRefs = assertPublicRefs(
    'Terminal-Bench Khala leaderboard projection refs',
    input.leaderboardProjectionRefs ?? [],
  )
  const flywheelEvaluationRefs = assertPublicRefs(
    'Terminal-Bench Khala flywheel evaluation refs',
    input.flywheelEvaluationRefs ?? [],
  )
  const blockerRefs = flywheelBlockers({
    decisionGrade: input.decisionGrade,
    hasKhalaWin: input.hasKhalaWin,
    rewardBundleRefs,
  })

  return decodeFlywheelProjection({
    schemaVersion: GYM_TERMINAL_BENCH_KHALA_FLYWHEEL_PROJECTION_SCHEMA,
    projectionRef: refFor(
      'projection.gym.terminal_bench.khala_flywheel',
      [input.reportRef, input.policyProfileRef],
      'projection',
    ),
    reportRef: input.reportRef,
    policyProfileRef: input.policyProfileRef,
    state: blockerRefs.length === 0 ? 'ready_for_training' : 'blocked',
    consumers: TRAINING_CONSUMERS,
    rewardBundleRefs,
    leaderboardProjectionRefs,
    flywheelEvaluationRefs,
    blockerRefs,
    caveatRefs: [
      'caveat.gym.terminal_bench.flywheel_evidence_only',
      'caveat.gym.terminal_bench.no_runtime_promotion_authority',
      'caveat.gym.terminal_bench.no_payout_or_settlement_authority',
    ],
    authority: evidenceOnlyAuthority,
  })
}

export const buildGymTerminalBenchKhalaOrchestrationReport = (input: {
  generatedAt: string
  rawBaselineReport: unknown
  khalaReport: unknown
  policyProfileRef: GymTerminalBenchKhalaPolicyProfileRef
  rewardBundleRefs?: ReadonlyArray<string> | undefined
  leaderboardProjectionRefs?: ReadonlyArray<string> | undefined
  flywheelEvaluationRefs?: ReadonlyArray<string> | undefined
  evidenceRefs?: ReadonlyArray<string> | undefined
}): GymTerminalBenchKhalaOrchestrationReport => {
  const rawBaselineReport = decodeComparisonReport(input.rawBaselineReport)
  const khalaReport = decodeComparisonReport(input.khalaReport)
  const policyProfile = resolveGymTerminalBenchKhalaPolicyProfile(
    input.policyProfileRef,
  )
  assertPublicRefs('Terminal-Bench Khala report refs', [
    rawBaselineReport.reportRef,
    khalaReport.reportRef,
    ...rawBaselineReport.rows.flatMap(row => row.evidenceRefs),
    ...khalaReport.rows.flatMap(row => row.evidenceRefs),
    ...(input.evidenceRefs ?? []),
  ])
  const rawBaselineRow = selectRawBaselineRow(rawBaselineReport)
  const khalaRow = selectKhalaRow(khalaReport)
  const blockerRefs = uniqueRefs([
    ...reportBlockers('raw_baseline', rawBaselineReport),
    ...reportBlockers('khala', khalaReport),
    ...rowBlockers('raw_baseline', rawBaselineRow),
    ...rowBlockers('khala', khalaRow),
  ])

  const blocked = blockerRefs.length > 0
  const fallbackRawRow = rawBaselineRow ?? rawBaselineReport.rows[0]
  const fallbackKhalaRow = khalaRow ?? khalaReport.rows[0]
  if (fallbackRawRow === undefined || fallbackKhalaRow === undefined) {
    throw new GymTerminalBenchKhalaOrchestrationUnsafe({
      reason:
        'Terminal-Bench Khala orchestration comparison requires at least one raw baseline row and one Khala row.',
    })
  }
  const solve = solveRateComparison(fallbackRawRow, fallbackKhalaRow, blocked)
  const cost = costComparison(fallbackRawRow, fallbackKhalaRow, blocked)
  const primary = primaryOutcome({ blocked, solve, cost })
  const outcomes = outcomesFor({ primary, solve, cost })
  const hasKhalaWin =
    outcomes.includes('beats_on_solve_rate') ||
    outcomes.includes('beats_on_cost_per_accepted_outcome')
  const decisionGrade = !blocked
  const reportRef = refFor(
    'report.gym.terminal_bench.khala_orchestration',
    [
      rawBaselineReport.reportRef,
      khalaReport.reportRef,
      policyProfile.policyProfileRef,
    ],
    'report',
  )
  const flywheelProjection = buildGymTerminalBenchKhalaFlywheelProjection({
    reportRef,
    policyProfileRef: policyProfile.policyProfileRef,
    decisionGrade,
    hasKhalaWin,
    rewardBundleRefs: input.rewardBundleRefs,
    leaderboardProjectionRefs: input.leaderboardProjectionRefs,
    flywheelEvaluationRefs: input.flywheelEvaluationRefs,
  })

  return decodeOrchestrationReport({
    schemaVersion: GYM_TERMINAL_BENCH_KHALA_ORCHESTRATION_REPORT_SCHEMA,
    reportRef,
    generatedAt: input.generatedAt,
    datasetRef: 'terminal-bench@2.0',
    rawBaselineReportRef: rawBaselineReport.reportRef,
    khalaReportRef: khalaReport.reportRef,
    rawBaselineProfileRef: fallbackRawRow.profileRef,
    khalaProfileRef: fallbackKhalaRow.profileRef,
    rawBaselineAttribution: fallbackRawRow.serving.attribution,
    khalaAttribution: fallbackKhalaRow.serving.attribution,
    policyProfile,
    primaryOutcome: primary,
    outcomes,
    beatsSolveRate: outcomes.includes('beats_on_solve_rate'),
    beatsCostPerAcceptedOutcome: outcomes.includes(
      'beats_on_cost_per_accepted_outcome',
    ),
    solveRateComparison: solve,
    costComparison: cost,
    decisionGrade,
    publicSafe: true,
    rawArtifactsIncluded: false,
    evidenceRefs: assertPublicRefs(
      'Terminal-Bench Khala orchestration evidence refs',
      [
        rawBaselineReport.reportRef,
        khalaReport.reportRef,
        ...policyProfile.sourceRefs,
        ...flywheelProjection.rewardBundleRefs,
        ...flywheelProjection.leaderboardProjectionRefs,
        ...flywheelProjection.flywheelEvaluationRefs,
        ...(input.evidenceRefs ?? []),
      ],
    ),
    blockerRefs,
    caveatRefs: uniqueRefs([
      ...rawBaselineReport.caveats,
      ...khalaReport.caveats,
      ...policyProfile.caveatRefs,
      'caveat.gym.terminal_bench.raw_baseline_is_zai_glm_reap_not_serving_vendor',
      'caveat.gym.terminal_bench.khala_win_requires_owner_armed_full_task_set',
      'caveat.gym.terminal_bench.flywheel_evidence_only',
    ]),
    flywheelProjection,
    authority: evidenceOnlyAuthority,
  })
}
