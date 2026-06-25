import { Schema as S } from 'effect'

import {
  GymHarborTerminalBenchModelId,
  type GymHarborTerminalBenchRun,
  GymTerminalBenchProfileRef,
  HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
  type GymHarborTerminalBenchModelId as GymHarborTerminalBenchModelIdType,
  type GymTerminalBenchProfileRef as GymTerminalBenchProfileRefType,
} from './harbor-dispatch'
import {
  BenchmarkLane as BenchmarkLaneSchema,
  type BenchmarkLane,
} from '../benchmark'

export const GYM_HARBOR_REAL_COST_BASIS_SCHEMA =
  'openagents.gym.harbor_real_cost_basis.v1'
export const GYM_HARBOR_REWARD_REPORT_SCHEMA =
  'openagents.gym.harbor_reward_report.v1'
export const GYM_HARBOR_TRAINING_TRAJECTORY_SCHEMA =
  'openagents.gym.harbor_training_trajectory.v1'

export const GymHarborGpuContentionGuard = S.Struct({
  state: S.Literals(['cleared', 'blocked']),
  schedulingMode: S.Literals([
    'benchmark_replica',
    'off_peak_exclusive_window',
    'unknown_live_lane',
  ]),
  liveServingLaneRefs: S.Array(S.String),
  benchmarkReplicaRef: S.NullOr(S.String),
  schedulingWindowRef: S.NullOr(S.String),
  blockers: S.Array(S.String),
})
export type GymHarborGpuContentionGuard =
  typeof GymHarborGpuContentionGuard.Type

export const GymHarborRealCostBasis = S.Struct({
  schemaVersion: S.Literal(GYM_HARBOR_REAL_COST_BASIS_SCHEMA),
  costBasisRef: S.String,
  source: S.Literal('served_tokens_recorder'),
  totalCostBasisMsat: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 1_000_000_000_000 }),
  ),
  tokenUsageEventRefs: S.Array(S.String),
  gpuContention: GymHarborGpuContentionGuard,
})
export type GymHarborRealCostBasis = typeof GymHarborRealCostBasis.Type

export const GymHarborTerminalBenchRewardReport = S.Struct({
  schemaVersion: S.Literal(GYM_HARBOR_REWARD_REPORT_SCHEMA),
  reportRef: S.String,
  configId: S.String,
  profileRef: GymTerminalBenchProfileRef,
  jobRef: S.String,
  hydraliskRunRef: S.String,
  summarySchema: S.Literal(HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA),
  summaryArtifactRef: S.String,
  costBasisRef: S.String,
  lane: BenchmarkLaneSchema,
  workload: S.Literal('verifier-run'),
  datasetRef: S.Literal('terminal-bench@2.0'),
  model: GymHarborTerminalBenchModelId,
  decisionGrade: S.Boolean,
  publicClaimEligible: S.Literal(false),
  verifierPlacementVerified: S.Literal(true),
  gpuContentionCleared: S.Boolean,
  acceptedOutcomes: S.Number,
  attemptedVerifications: S.Number,
  totalTasks: S.Number,
  scalarRewardMean: S.NullOr(S.Number),
  totalCostBasisMsat: S.Number,
  costPerAcceptedOutcomeMsat: S.NullOr(S.Number),
  blockers: S.Array(S.String),
  caveats: S.Array(S.String),
})
export type GymHarborTerminalBenchRewardReport =
  typeof GymHarborTerminalBenchRewardReport.Type

export type GymHarborTerminalBenchRewardReportProfile = Readonly<{
  profileRef: GymTerminalBenchProfileRefType
  lane: BenchmarkLane
  model: GymHarborTerminalBenchModelIdType
}>

export const GymHarborTrainingTrajectory = S.Struct({
  schemaVersion: S.Literal(GYM_HARBOR_TRAINING_TRAJECTORY_SCHEMA),
  trajectoryBundleRef: S.String,
  atifTraceRef: S.String,
  trajectoryFormat: S.Literal('atif_public_safe_subset'),
  trainingLoopRefs: S.Array(S.String),
  rewardKind: S.Literal('terminal_bench_executed_reward'),
  rewardSource: S.Literal('harbor_verifier_artifact'),
  rewardArtifactRef: S.String,
  acceptedOutcomes: S.Number,
  attemptedVerifications: S.Number,
  scalarRewardMean: S.NullOr(S.Number),
  publicSafe: S.Literal(true),
  rawTraceIncluded: S.Literal(false),
  readyForTraining: S.Boolean,
  blockers: S.Array(S.String),
})
export type GymHarborTrainingTrajectory =
  typeof GymHarborTrainingTrajectory.Type

export class GymHarborRewardError extends S.TaggedErrorClass<GymHarborRewardError>()(
  'GymHarborRewardError',
  {
    reason: S.Literals(['missing_atif_trace', 'invalid_cost_basis']),
    message: S.String,
  },
) {}

const decodeCostBasis = S.decodeUnknownSync(GymHarborRealCostBasis)
const decodeRewardReport = S.decodeUnknownSync(
  GymHarborTerminalBenchRewardReport,
)
const decodeTrainingTrajectory = S.decodeUnknownSync(GymHarborTrainingTrajectory)

const safeRefSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, '_')
    .replace(/^_+|_+$/g, '')

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

const reportRefForRun = (
  run: GymHarborTerminalBenchRun,
  costBasis: GymHarborRealCostBasis,
): string =>
  `report.gym.harbor_terminal_bench.${safeRefSegment(run.job.jobRef)}.${fnv1a32(
    stableJson({
      accepted: run.summary.counts.solved,
      attempted: run.summary.counts.properlyAttempted,
      costBasisRef: costBasis.costBasisRef,
      totalCostBasisMsat: costBasis.totalCostBasisMsat,
    }),
  )}`

const trajectoryBundleRefForRun = (
  run: GymHarborTerminalBenchRun,
  costBasis: GymHarborRealCostBasis,
): string =>
  `trajectory.gym.harbor_terminal_bench.${safeRefSegment(
    run.job.jobRef,
  )}.${fnv1a32(
    stableJson({
      atifTraceRef: run.dispatch.atifTraceRef,
      rewardArtifactRef: run.dispatch.verifierPlacement.rewardArtifactRef,
      costBasisRef: costBasis.costBasisRef,
    }),
  )}`

const scalarRewardMean = (
  acceptedOutcomes: number,
  attemptedVerifications: number,
): number | null =>
  attemptedVerifications <= 0 ? null : acceptedOutcomes / attemptedVerifications

const reportBlockers = (input: {
  costBasis: GymHarborRealCostBasis
  atifTraceRef: string | null
}): ReadonlyArray<string> => {
  const blockers: Array<string> = []
  if (input.costBasis.gpuContention.state !== 'cleared') {
    blockers.push('blocker.gym.harbor.gpu_contention_not_cleared')
  }
  if (input.costBasis.gpuContention.blockers.length > 0) {
    blockers.push(...input.costBasis.gpuContention.blockers)
  }
  if (input.atifTraceRef === null || input.atifTraceRef.trim() === '') {
    blockers.push('blocker.gym.harbor.atif_trace_missing')
  }
  return Array.from(new Set(blockers)).sort()
}

export const buildGymHarborTerminalBenchRewardArtifacts = (input: {
  run: GymHarborTerminalBenchRun
  costBasis: GymHarborRealCostBasis
}): Readonly<{
  report: GymHarborTerminalBenchRewardReport
  trainingTrajectory: GymHarborTrainingTrajectory
}> => {
  const costBasis = decodeCostBasis(input.costBasis)
  if (costBasis.totalCostBasisMsat < 0) {
    throw new GymHarborRewardError({
      reason: 'invalid_cost_basis',
      message: 'Harbor reward report cost basis must be non-negative.',
    })
  }
  const atifTraceRef = input.run.dispatch.atifTraceRef
  if (atifTraceRef === null || atifTraceRef.trim() === '') {
    throw new GymHarborRewardError({
      reason: 'missing_atif_trace',
      message:
        'Harbor training trajectory ingestion requires a public-safe ATIF trace ref.',
    })
  }

  const acceptedOutcomes = input.run.summary.counts.solved
  const attemptedVerifications = input.run.summary.counts.properlyAttempted
  const meanReward = scalarRewardMean(acceptedOutcomes, attemptedVerifications)
  const blockers = reportBlockers({ costBasis, atifTraceRef })
  const gpuContentionCleared = costBasis.gpuContention.state === 'cleared'
  const costPerAcceptedOutcomeMsat =
    acceptedOutcomes <= 0
      ? null
      : costBasis.totalCostBasisMsat / acceptedOutcomes
  const decisionGrade =
    input.run.job.ownerApprovalRef !== null &&
    input.run.ingest.verifierPlacementVerified &&
    gpuContentionCleared &&
    blockers.length === 0

  const report = decodeRewardReport({
    schemaVersion: GYM_HARBOR_REWARD_REPORT_SCHEMA,
    reportRef: reportRefForRun(input.run, costBasis),
    configId: input.run.job.configId,
    profileRef: input.run.job.profileRef,
    jobRef: input.run.job.jobRef,
    hydraliskRunRef: input.run.dispatch.hydraliskRunRef,
    summarySchema: HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
    summaryArtifactRef: input.run.dispatch.summaryArtifactRef,
    costBasisRef: costBasis.costBasisRef,
    lane: input.run.job.servingProfile.lane,
    workload: 'verifier-run',
    datasetRef: input.run.summary.benchmark.datasetRef,
    model: input.run.summary.runner.model,
    decisionGrade,
    publicClaimEligible: false,
    verifierPlacementVerified: true,
    gpuContentionCleared,
    acceptedOutcomes,
    attemptedVerifications,
    totalTasks: input.run.summary.counts.total,
    scalarRewardMean: meanReward,
    totalCostBasisMsat: costBasis.totalCostBasisMsat,
    costPerAcceptedOutcomeMsat,
    blockers,
    caveats: [
      ...input.run.job.servingProfile.caveatRefs,
      'public_claim_requires_product_promise_review',
      'raw_harbor_artifacts_excluded',
      'cost_basis_from_served_tokens_recorder',
    ],
  })

  const trainingTrajectory = decodeTrainingTrajectory({
    schemaVersion: GYM_HARBOR_TRAINING_TRAJECTORY_SCHEMA,
    trajectoryBundleRef: trajectoryBundleRefForRun(input.run, costBasis),
    atifTraceRef,
    trajectoryFormat: 'atif_public_safe_subset',
    trainingLoopRefs: [
      'psionic.gepa.reward_bundle.v1',
      'psionic.trinity.rollout_reward.v1',
      'psionic.conductor.runtime_candidate.v1',
    ],
    rewardKind: 'terminal_bench_executed_reward',
    rewardSource: 'harbor_verifier_artifact',
    rewardArtifactRef: input.run.dispatch.verifierPlacement.rewardArtifactRef,
    acceptedOutcomes,
    attemptedVerifications,
    scalarRewardMean: meanReward,
    publicSafe: true,
    rawTraceIncluded: false,
    readyForTraining: blockers.length === 0,
    blockers,
  })

  return { report, trainingTrajectory }
}
