import { Schema as S } from 'effect'

import { NOT_MEASURED, type MeasuredNumber } from '../khala-telemetry'
import {
  BenchmarkLane as BenchmarkLaneSchema,
  type BenchmarkLane,
} from '../benchmark'
import {
  GymHarborTerminalBenchModelId,
  type GymHarborTerminalBenchRun,
  GymTerminalBenchProfileRef,
  GymTerminalBenchQuantization,
  GymTerminalBenchReplicaTopology,
  GymTerminalBenchSpeculationMode,
  HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
  type GymHarborTerminalBenchModelId as GymHarborTerminalBenchModelIdType,
  type GymTerminalBenchProfileRef as GymTerminalBenchProfileRefType,
} from './harbor-dispatch'
import {
  GymHarborTerminalBenchRewardReport,
  type GymHarborTerminalBenchRewardReport as GymHarborTerminalBenchRewardReportType,
} from './harbor-reward'

export const GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA =
  'openagents.gym.terminal_bench_comparison_report.v1'
export const OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS = 89

const ComparisonMeasuredNumber = S.Union([S.Number, S.Literal(NOT_MEASURED)])

export const GymTerminalBenchExternalClaimTarget = S.Struct({
  targetRef: S.String,
  kind: S.Literal('external_claim'),
  label: S.String,
  datasetRef: S.Literal('terminal-bench@2.0'),
  officialTotalTasks: S.Literal(OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS),
  claimedFullDenominatorSolveRate: S.Number,
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type GymTerminalBenchExternalClaimTarget =
  typeof GymTerminalBenchExternalClaimTarget.Type

export const GLM_REAP_TERMINAL_BENCH_691_TARGET =
  S.decodeUnknownSync(GymTerminalBenchExternalClaimTarget)({
    targetRef: 'claim.external.glm_reap_terminal_bench_2_691',
    kind: 'external_claim',
    label: 'GLM-5.2 REAP claimed 69.1% on Terminal-Bench 2.0',
    datasetRef: 'terminal-bench@2.0',
    officialTotalTasks: OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS,
    claimedFullDenominatorSolveRate: 0.691,
    sourceRefs: [
      'source.huggingface.0xsero.glm_5_2_504b',
      'source.x.glm_reap_terminal_bench_2_691_claim',
    ],
    caveatRefs: [
      'caveat.external_claim.not_openagents_result',
      'caveat.external_claim.requires_source_review',
    ],
  })

export const GymTerminalBenchThroughputMeasurement = S.Struct({
  profileRef: GymTerminalBenchProfileRef,
  measurementRef: S.String,
  ttftMs: ComparisonMeasuredNumber,
  totalWallClockMs: ComparisonMeasuredNumber,
  perceivedTps: ComparisonMeasuredNumber,
  interTokenLatencyMs: ComparisonMeasuredNumber,
  aggregateTps: ComparisonMeasuredNumber,
})
export type GymTerminalBenchThroughputMeasurement =
  typeof GymTerminalBenchThroughputMeasurement.Type

export const GymTerminalBenchComparisonServing = S.Struct({
  publicLabel: S.String,
  sourceModelRef: S.String,
  attribution: S.String,
  hardwareProfile: S.String,
  tensorParallelism: S.Number,
  replicaTopology: GymTerminalBenchReplicaTopology,
  contextWindowTokens: S.Number,
  quantization: GymTerminalBenchQuantization,
  speculationMode: GymTerminalBenchSpeculationMode,
  sampler: S.Struct({
    minP: S.NullOr(S.Number),
    repetitionPenalty: S.Number,
    enableThinking: S.Boolean,
  }),
})
export type GymTerminalBenchComparisonServing =
  typeof GymTerminalBenchComparisonServing.Type

export const GymTerminalBenchProfileComparisonRow = S.Struct({
  profileRef: GymTerminalBenchProfileRef,
  lane: BenchmarkLaneSchema,
  model: GymHarborTerminalBenchModelId,
  serving: GymTerminalBenchComparisonServing,
  hydraliskRunRef: S.String,
  summarySchema: S.Literal(HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA),
  summaryArtifactRef: S.String,
  costBasisRef: S.String,
  throughputMeasurementRef: S.String,
  officialFullTaskSet: S.Boolean,
  totalTasks: S.Number,
  attemptedTasks: S.Number,
  properlyAttemptedTasks: S.Number,
  solvedTasks: S.Number,
  fullDenominatorSolveRate: S.NullOr(S.Number),
  attemptedSolveRate: S.NullOr(S.Number),
  properlyAttemptedSolveRate: S.NullOr(S.Number),
  gapToClaimBps: S.NullOr(S.Number),
  totalCostBasisMsat: S.Number,
  costPerAcceptedOutcomeMsat: S.NullOr(S.Number),
  ttftMs: ComparisonMeasuredNumber,
  totalWallClockMs: ComparisonMeasuredNumber,
  perceivedTps: ComparisonMeasuredNumber,
  interTokenLatencyMs: ComparisonMeasuredNumber,
  aggregateTps: ComparisonMeasuredNumber,
  decisionGrade: S.Boolean,
  replicationClaimSatisfied: S.Boolean,
  blockers: S.Array(S.String),
  caveats: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
})
export type GymTerminalBenchProfileComparisonRow =
  typeof GymTerminalBenchProfileComparisonRow.Type

export const GymTerminalBenchComparisonReport = S.Struct({
  schemaVersion: S.Literal(GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA),
  reportRef: S.String,
  generatedAt: S.String,
  datasetRef: S.Literal('terminal-bench@2.0'),
  externalClaim: GymTerminalBenchExternalClaimTarget,
  rows: S.Array(GymTerminalBenchProfileComparisonRow),
  decisionGrade: S.Boolean,
  replicationClaimSatisfied: S.Boolean,
  bestDecisionGradeSolveRateProfileRef: S.NullOr(GymTerminalBenchProfileRef),
  bestDecisionGradeCostProfileRef: S.NullOr(GymTerminalBenchProfileRef),
  publicSafe: S.Literal(true),
  rawArtifactsIncluded: S.Literal(false),
  blockers: S.Array(S.String),
  caveats: S.Array(S.String),
})
export type GymTerminalBenchComparisonReport =
  typeof GymTerminalBenchComparisonReport.Type

export type GymTerminalBenchComparisonRowInput = Readonly<{
  run: GymHarborTerminalBenchRun
  rewardReport: GymHarborTerminalBenchRewardReportType
  throughput: GymTerminalBenchThroughputMeasurement
  evidenceRefs?: ReadonlyArray<string> | undefined
}>

export type GymTerminalBenchComparisonProfile = Readonly<{
  profileRef: GymTerminalBenchProfileRefType
  lane: BenchmarkLane
  model: GymHarborTerminalBenchModelIdType
}>

export class GymTerminalBenchComparisonError extends S.TaggedErrorClass<GymTerminalBenchComparisonError>()(
  'GymTerminalBenchComparisonError',
  {
    reason: S.Literals(['profile_mismatch']),
    message: S.String,
  },
) {}

const decodeRewardReport = S.decodeUnknownSync(
  GymHarborTerminalBenchRewardReport,
)
const decodeThroughput = S.decodeUnknownSync(
  GymTerminalBenchThroughputMeasurement,
)
const decodeRow = S.decodeUnknownSync(GymTerminalBenchProfileComparisonRow)
const decodeReport = S.decodeUnknownSync(GymTerminalBenchComparisonReport)
const decodeClaimTarget = S.decodeUnknownSync(GymTerminalBenchExternalClaimTarget)

const safeRefSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, '_')
    .replace(/^_+|_+$/g, '') || 'empty'

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  Array.from(new Set(values.map(value => value.trim()).filter(Boolean))).sort()

const fnv1a32 = (value: string): string => {
  let hash = 0x811c9dc5
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash.toString(16).padStart(8, '0')
}

const stableJson = (value: unknown): string => {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value) ?? 'null'
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`
  }
  const entries = Object.entries(value as Record<string, unknown>).sort(
    ([a], [b]) => a.localeCompare(b),
  )
  return `{${entries
    .map(([key, entry]) => `${JSON.stringify(key)}:${stableJson(entry)}`)
    .join(',')}}`
}

const reportRefForRows = (input: {
  claim: GymTerminalBenchExternalClaimTarget
  rows: ReadonlyArray<GymTerminalBenchProfileComparisonRow>
}): string =>
  `report.gym.terminal_bench_comparison.${safeRefSegment(
    input.claim.targetRef,
  )}.${fnv1a32(
    stableJson({
      claim: input.claim.targetRef,
      rows: input.rows.map(row => ({
        costBasisRef: row.costBasisRef,
        profileRef: row.profileRef,
        solvedTasks: row.solvedTasks,
        summaryArtifactRef: row.summaryArtifactRef,
        totalTasks: row.totalTasks,
      })),
    }),
  )}`

const rate = (numerator: number, denominator: number): number | null =>
  denominator <= 0 ? null : numerator / denominator

const gapBps = (rateValue: number | null, target: number): number | null =>
  rateValue === null ? null : Math.round((rateValue - target) * 10_000)

const measuredValue = (value: MeasuredNumber): MeasuredNumber =>
  value === NOT_MEASURED ? NOT_MEASURED : value

const throughputCaveats = (
  throughput: GymTerminalBenchThroughputMeasurement,
): ReadonlyArray<string> => {
  const caveats: Array<string> = []
  if (
    throughput.ttftMs === NOT_MEASURED ||
    throughput.totalWallClockMs === NOT_MEASURED ||
    throughput.perceivedTps === NOT_MEASURED ||
    throughput.interTokenLatencyMs === NOT_MEASURED ||
    throughput.aggregateTps === NOT_MEASURED
  ) {
    caveats.push('caveat.gym.terminal_bench.throughput_not_fully_measured')
  }
  return caveats
}

const rowBlockers = (input: {
  run: GymHarborTerminalBenchRun
  rewardReport: GymHarborTerminalBenchRewardReportType
  officialFullTaskSet: boolean
}): ReadonlyArray<string> => {
  const blockers: Array<string> = []
  if (input.run.job.ownerApprovalRef === null) {
    blockers.push('blocker.gym.terminal_bench.owner_approval_missing')
  }
  if (!input.officialFullTaskSet) {
    blockers.push('blocker.gym.terminal_bench.official_full_task_set_required')
  }
  if (!input.run.publicSafety.safe) {
    blockers.push('blocker.gym.terminal_bench.public_safe_summary_required')
  }
  if (
    !input.run.ingest.verifierPlacementVerified ||
    !input.rewardReport.verifierPlacementVerified
  ) {
    blockers.push('blocker.gym.terminal_bench.verifier_placement_required')
  }
  if (input.rewardReport.costBasisRef.trim() === '') {
    blockers.push('blocker.gym.terminal_bench.cost_basis_missing')
  }
  if (!input.rewardReport.gpuContentionCleared) {
    blockers.push('blocker.gym.terminal_bench.gpu_contention_not_cleared')
  }
  if (!input.rewardReport.decisionGrade) {
    blockers.push('blocker.gym.terminal_bench.reward_report_not_decision_grade')
  }
  blockers.push(...input.rewardReport.blockers)
  return uniqueSorted(blockers)
}

const comparisonRow = (
  input: GymTerminalBenchComparisonRowInput,
  claim: GymTerminalBenchExternalClaimTarget,
): GymTerminalBenchProfileComparisonRow => {
  const rewardReport = decodeRewardReport(input.rewardReport)
  const throughput = decodeThroughput(input.throughput)
  if (throughput.profileRef !== input.run.job.profileRef) {
    throw new GymTerminalBenchComparisonError({
      reason: 'profile_mismatch',
      message:
        'Terminal-Bench throughput measurement profile must match the Harbor run profile.',
    })
  }
  if (rewardReport.profileRef !== input.run.job.profileRef) {
    throw new GymTerminalBenchComparisonError({
      reason: 'profile_mismatch',
      message:
        'Terminal-Bench reward report profile must match the Harbor run profile.',
    })
  }

  const summary = input.run.summary
  const profile = input.run.job.servingProfile
  const officialFullTaskSet =
    summary.benchmark.datasetRef === 'terminal-bench@2.0' &&
    summary.counts.total === OFFICIAL_TERMINAL_BENCH_2_TOTAL_TASKS
  const blockers = rowBlockers({ run: input.run, rewardReport, officialFullTaskSet })
  const fullDenominatorSolveRate = rate(
    summary.counts.solved,
    summary.counts.total,
  )
  const attemptedSolveRate = rate(
    summary.counts.solved,
    summary.counts.attempted,
  )
  const properlyAttemptedSolveRate = rate(
    summary.counts.solved,
    summary.counts.properlyAttempted,
  )
  const decisionGrade = blockers.length === 0
  const claimGapBps = gapBps(
    fullDenominatorSolveRate,
    claim.claimedFullDenominatorSolveRate,
  )

  return decodeRow({
    profileRef: input.run.job.profileRef,
    lane: profile.lane,
    model: input.run.job.model,
    serving: {
      publicLabel: profile.publicLabel,
      sourceModelRef: profile.sourceModelRef,
      attribution: profile.attribution,
      hardwareProfile: profile.hardwareProfile,
      tensorParallelism: profile.tensorParallelism,
      replicaTopology: profile.replicaTopology,
      contextWindowTokens: profile.contextWindowTokens,
      quantization: profile.quantization,
      speculationMode: profile.speculationMode,
      sampler: profile.sampler,
    },
    hydraliskRunRef: input.run.dispatch.hydraliskRunRef,
    summarySchema: HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
    summaryArtifactRef: input.run.dispatch.summaryArtifactRef,
    costBasisRef: rewardReport.costBasisRef,
    throughputMeasurementRef: throughput.measurementRef,
    officialFullTaskSet,
    totalTasks: summary.counts.total,
    attemptedTasks: summary.counts.attempted,
    properlyAttemptedTasks: summary.counts.properlyAttempted,
    solvedTasks: summary.counts.solved,
    fullDenominatorSolveRate,
    attemptedSolveRate,
    properlyAttemptedSolveRate,
    gapToClaimBps: claimGapBps,
    totalCostBasisMsat: rewardReport.totalCostBasisMsat,
    costPerAcceptedOutcomeMsat: rewardReport.costPerAcceptedOutcomeMsat,
    ttftMs: measuredValue(throughput.ttftMs),
    totalWallClockMs: measuredValue(throughput.totalWallClockMs),
    perceivedTps: measuredValue(throughput.perceivedTps),
    interTokenLatencyMs: measuredValue(throughput.interTokenLatencyMs),
    aggregateTps: measuredValue(throughput.aggregateTps),
    decisionGrade,
    replicationClaimSatisfied:
      decisionGrade && claimGapBps !== null && claimGapBps >= 0,
    blockers,
    caveats: uniqueSorted([
      ...rewardReport.caveats,
      ...profile.caveatRefs,
      ...throughputCaveats(throughput),
    ]),
    evidenceRefs: uniqueSorted([
      input.run.job.jobRef,
      input.run.dispatch.hydraliskRunRef,
      input.run.dispatch.summaryArtifactRef,
      input.run.dispatch.verifierPlacement.rewardArtifactRef,
      rewardReport.costBasisRef,
      throughput.measurementRef,
      ...(input.evidenceRefs ?? []),
    ]),
  })
}

const bestBySolveRate = (
  rows: ReadonlyArray<GymTerminalBenchProfileComparisonRow>,
): GymTerminalBenchProfileRefType | null => {
  const candidates = rows
    .filter(row => row.decisionGrade && row.fullDenominatorSolveRate !== null)
    .sort((left, right) => {
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
      const costDelta =
        leftCost - rightCost
      if (costDelta !== 0) {
        return costDelta
      }
      return left.profileRef.localeCompare(right.profileRef)
    })
  return candidates[0]?.profileRef ?? null
}

const bestByCost = (
  rows: ReadonlyArray<GymTerminalBenchProfileComparisonRow>,
): GymTerminalBenchProfileRefType | null => {
  const candidates = rows
    .filter(
      row => row.decisionGrade && row.costPerAcceptedOutcomeMsat !== null,
    )
    .sort((left, right) => {
      const leftCost =
        left.costPerAcceptedOutcomeMsat ?? Number.MAX_SAFE_INTEGER
      const rightCost =
        right.costPerAcceptedOutcomeMsat ?? Number.MAX_SAFE_INTEGER
      const costDelta =
        leftCost - rightCost
      if (costDelta !== 0) {
        return costDelta
      }
      const solveDelta =
        (right.fullDenominatorSolveRate ?? -1) -
        (left.fullDenominatorSolveRate ?? -1)
      if (solveDelta !== 0) {
        return solveDelta
      }
      return left.profileRef.localeCompare(right.profileRef)
    })
  return candidates[0]?.profileRef ?? null
}

export const buildGymTerminalBenchComparisonReport = (input: {
  generatedAt: string
  rows: ReadonlyArray<GymTerminalBenchComparisonRowInput>
  externalClaim?: GymTerminalBenchExternalClaimTarget | undefined
}): GymTerminalBenchComparisonReport => {
  const externalClaim = decodeClaimTarget(
    input.externalClaim ?? GLM_REAP_TERMINAL_BENCH_691_TARGET,
  )
  const rows = input.rows.map(row => comparisonRow(row, externalClaim))
  const reportBlockers = uniqueSorted(rows.flatMap(row => row.blockers))
  const reportCaveats = uniqueSorted([
    ...externalClaim.caveatRefs,
    ...rows.flatMap(row => row.caveats),
    'caveat.gym.terminal_bench.comparison_public_safe_projection_only',
    'caveat.gym.terminal_bench.external_claim_not_openagents_result',
  ])

  return decodeReport({
    schemaVersion: GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA,
    reportRef: reportRefForRows({ claim: externalClaim, rows }),
    generatedAt: input.generatedAt,
    datasetRef: 'terminal-bench@2.0',
    externalClaim,
    rows,
    decisionGrade: rows.length > 0 && rows.every(row => row.decisionGrade),
    replicationClaimSatisfied: rows.some(row => row.replicationClaimSatisfied),
    bestDecisionGradeSolveRateProfileRef: bestBySolveRate(rows),
    bestDecisionGradeCostProfileRef: bestByCost(rows),
    publicSafe: true,
    rawArtifactsIncluded: false,
    blockers: reportBlockers,
    caveats: reportCaveats,
  })
}
