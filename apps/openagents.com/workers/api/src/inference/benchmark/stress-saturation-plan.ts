import { Schema as S } from 'effect'

import type {
  BenchmarkCell,
  BenchmarkMatrixConfig,
  BenchmarkTarget,
  BenchmarkWorkload,
  SequenceShape,
} from './matrix'
import { expandMatrix } from './matrix'
import { mean, percentile } from './report'

export const GLM_CONTINUOUS_STRESS_PLAN_SCHEMA =
  'openagents.khala.glm_continuous_stress_plan.v0_1' as const

export const GLM_CONTINUOUS_STRESS_BLOCKER_SCHEMA =
  'openagents.khala.glm_continuous_stress_blocker.v0_1' as const

export const GLM_CONTINUOUS_STRESS_RUNNER_PLAN_SCHEMA =
  'openagents.khala.glm_continuous_stress_runner_plan.v0_1' as const

export const GLM_CONTINUOUS_STRESS_REPORT_SCHEMA =
  'openagents.khala.glm_continuous_stress_report.v0_1' as const

export const GLM_EXTERNAL_WINS_PROOF_SCHEMA =
  'openagents.khala.glm_external_wins_proof.v0_1' as const

export const GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA =
  'openagents.khala.telemetry.v1' as const

export const GLM_STRESS_DEMAND_KIND = 'internal_stress' as const

export const GLM_STRESS_DEMAND_SOURCE = 'glm-saturation' as const

export const GLM_STRESS_DEMAND_CLIENT = 'stress-harness' as const

export const GLM_STRESS_EXTERNAL_WINS_POLICY =
  'external_wins_reserved_headroom_and_preemption_required' as const

export const GLM_EXTERNAL_WINS_PRIMARY_SERVED_MODEL =
  'openagents/glm-5.2-reap-504b' as const

export const GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD = 0.02

export const GLM_STRESS_BACKOFF_STEP_FRACTION = 0.25

export const GLM_STRESS_ROLLOUT_STATE = S.Literals([
  'blocked_missing_live_scheduler_evidence',
  'armed',
])
export type GlmStressRolloutState = typeof GLM_STRESS_ROLLOUT_STATE.Type

export const GlmStressBlockerReason = S.Literals([
  'missing_live_headroom_evidence',
  'missing_external_wins_preemption_evidence',
  'missing_rollout_guard_evidence',
  'external_wins_proof_not_accepted',
  'throughput_rollout_not_accepted_for_stress',
  'external_headroom_unavailable',
  'no_glm_stress_cells',
])
export type GlmStressBlockerReason = typeof GlmStressBlockerReason.Type

export type GlmStressExternalHeadroomSnapshot = Readonly<{
  healthyReplicaCount: number
  aggregateAvailableSlots: number
  reservedExternalSlots: number
  externalDemandActive: boolean
}>

export type GlmStressEvidence = Readonly<{
  liveHeadroomEvidenceRef?: string | undefined
  externalWinsPreemptionEvidenceRef?: string | undefined
  externalWinsProofStatus?: GlmExternalWinsProofStatus | undefined
  rolloutGuardEvidenceRef?: string | undefined
  throughputRolloutCanStartIssue6317Stress?: boolean | undefined
}>

export type GlmStressPlanInput = Readonly<{
  matrixConfig: BenchmarkMatrixConfig
  target: BenchmarkTarget
  shapes: ReadonlyArray<SequenceShape>
  workloads: ReadonlyArray<BenchmarkWorkload>
  headroom?: GlmStressExternalHeadroomSnapshot | undefined
  evidence?: GlmStressEvidence | undefined
}>

export type GlmStressPlanBlocker = Readonly<{
  schema: typeof GLM_CONTINUOUS_STRESS_BLOCKER_SCHEMA
  state: 'blocked_missing_live_scheduler_evidence'
  blockerRefs: ReadonlyArray<string>
  reasons: ReadonlyArray<GlmStressBlockerReason>
  demandKind: typeof GLM_STRESS_DEMAND_KIND
  demandSource: typeof GLM_STRESS_DEMAND_SOURCE
  demandClient: typeof GLM_STRESS_DEMAND_CLIENT
  externalWinsPolicy: typeof GLM_STRESS_EXTERNAL_WINS_POLICY
}>

export type GlmStressRunnablePlan = Readonly<{
  schema: typeof GLM_CONTINUOUS_STRESS_PLAN_SCHEMA
  state: 'armed'
  demandKind: typeof GLM_STRESS_DEMAND_KIND
  demandSource: typeof GLM_STRESS_DEMAND_SOURCE
  demandClient: typeof GLM_STRESS_DEMAND_CLIENT
  externalWinsPolicy: typeof GLM_STRESS_EXTERNAL_WINS_POLICY
  reservedExternalSlots: number
  maxStressConcurrency: number
  cells: ReadonlyArray<BenchmarkCell>
  evidenceRefs: ReadonlyArray<string>
}>

export type GlmStressPlan = GlmStressPlanBlocker | GlmStressRunnablePlan

export type GlmStressRunnerPlanInput = Readonly<{
  generatedAt: string
  tickRef: string
  plan: GlmStressPlan
}>

export type GlmStressRunnerDispatchCell = Readonly<{
  cellId: string
  lane: BenchmarkCell['lane']
  engine: BenchmarkCell['engine']
  workload: BenchmarkCell['workload']
  shapeId: string
  requestedShapeConcurrency: number
  globalMaxConcurrency: number
  transport: BenchmarkCell['transport']
  telemetrySchema: typeof GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA
  demandKind: typeof GLM_STRESS_DEMAND_KIND
  demandSource: typeof GLM_STRESS_DEMAND_SOURCE
  demandClient: typeof GLM_STRESS_DEMAND_CLIENT
  requestHeaders: Readonly<{
    'x-openagents-demand-kind': typeof GLM_STRESS_DEMAND_KIND
    'x-openagents-demand-source': typeof GLM_STRESS_DEMAND_SOURCE
    'x-openagents-client': typeof GLM_STRESS_DEMAND_CLIENT
  }>
}>

export type GlmStressRunnerPlan = Readonly<{
  schema: typeof GLM_CONTINUOUS_STRESS_RUNNER_PLAN_SCHEMA
  generatedAt: string
  tickRef: string
  state: 'blocked' | 'ready'
  publicSafe: true
  canDispatch: boolean
  externalWinsPolicy: typeof GLM_STRESS_EXTERNAL_WINS_POLICY
  blockerRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  demandKind: typeof GLM_STRESS_DEMAND_KIND
  demandSource: typeof GLM_STRESS_DEMAND_SOURCE
  demandClient: typeof GLM_STRESS_DEMAND_CLIENT
  globalMaxConcurrency: number
  reservedExternalSlots: number
  dispatchCells: ReadonlyArray<GlmStressRunnerDispatchCell>
}>

export type GlmStressObservationStatus =
  | 'ok'
  | 'deferred_no_headroom'
  | 'preempted_for_external'
  | 'failed'

export type GlmStressFailureKind =
  | 'gateway_overload'
  | 'provider_overload'
  | 'rate_limited'
  | 'timeout'
  | 'unknown'

export type GlmStressObservation = Readonly<{
  cellId: string
  replicaRef?: string | undefined
  status: GlmStressObservationStatus
  httpStatus?: number | undefined
  failureKind?: GlmStressFailureKind | undefined
  outputTokens: number
  wallClockMs: number
  ttftMs?: number | undefined
  interTokenLatencyP50Ms?: number | undefined
  interTokenLatencyP90Ms?: number | undefined
  interTokenLatencyP99Ms?: number | undefined
  goodputTokens?: number | undefined
}>

export type GlmStressLatencySummary = Readonly<{
  p50: number | null
  p90: number | null
  p99: number | null
  mean: number | null
  sampleCount: number
}>

export type GlmStressLatencyRollup = Readonly<{
  ttftMs: GlmStressLatencySummary
  interTokenLatencyP50Ms: GlmStressLatencySummary
  interTokenLatencyP90Ms: GlmStressLatencySummary
  interTokenLatencyP99Ms: GlmStressLatencySummary
}>

export type GlmStressBackoffAction = 'hold' | 'decrease' | 'pause'

export type GlmStressBackoffReason =
  | 'none'
  | 'runner_not_ready'
  | 'all_dispatch_failed'
  | 'error_rate_over_budget'
  | 'overload_failures_observed'

export type GlmStressBackoffRecommendation = Readonly<{
  action: GlmStressBackoffAction
  currentConcurrency: number
  recommendedNextConcurrency: number
  maxStressConcurrency: number
  errorRateBackoffThreshold: number
  observedErrorRate: number | null
  overloadFailureCount: number
  reasonRefs: ReadonlyArray<string>
}>

export type GlmStressReplicaRollup = Readonly<{
  replicaRef: string
  aggregateTokPerSecond: number | null
  goodputTokPerSecond: number | null
  outputTokens: number
  goodputTokens: number
  okCount: number
  deferredCount: number
  preemptedCount: number
  failedCount: number
  latencyMs: GlmStressLatencyRollup
}>

export type GlmStressReportInput = Readonly<{
  generatedAt: string
  runnerPlan: GlmStressRunnerPlan
  throughputMeasurementWindowMs: number
  observations: ReadonlyArray<GlmStressObservation>
}>

export type GlmStressReport = Readonly<{
  schema: typeof GLM_CONTINUOUS_STRESS_REPORT_SCHEMA
  generatedAt: string
  tickRef: string
  publicSafe: true
  runnerState: GlmStressRunnerPlan['state']
  demandKind: typeof GLM_STRESS_DEMAND_KIND
  demandSource: typeof GLM_STRESS_DEMAND_SOURCE
  demandClient: typeof GLM_STRESS_DEMAND_CLIENT
  telemetrySchema: typeof GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA
  blockerRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  throughputMeasurementWindowMs: number | null
  aggregateTokPerSecond: number | null
  goodputTokPerSecond: number | null
  errorRate: number | null
  deferredCount: number
  preemptedCount: number
  failedCount: number
  overloadFailureCount: number
  okCount: number
  backoff: GlmStressBackoffRecommendation
  replicaRefs: ReadonlyArray<string>
  latencyMs: GlmStressLatencyRollup
  replicaRollups: ReadonlyArray<GlmStressReplicaRollup>
}>

export type GlmExternalWinsServedLane =
  | 'glm_primary'
  | 'unknown'
  | 'weaker_fallback'

export type GlmExternalWinsProofStatus = 'accepted' | 'blocked'

export type GlmExternalWinsProofBlockerReason =
  | 'empty_glm_content_after_preemption'
  | 'external_request_failed'
  | 'fallback_after_preemption'
  | 'missing_scheduler_preemption'
  | 'served_lane_not_glm_primary'

export type GlmExternalWinsProbeInput = Readonly<{
  externalHttpStatus: number
  fallbackReason: string | null
  schedulerPreemptionEvidenceRef?: string | undefined
  schedulerPreemptionTargetOutcome?: 'preempted_yielded' | undefined
  servedLane: GlmExternalWinsServedLane
  usageTotalTokens?: number | undefined
}>

export type GlmExternalWinsProofReadout = Readonly<{
  schema: typeof GLM_EXTERNAL_WINS_PROOF_SCHEMA
  status: GlmExternalWinsProofStatus
  publicSafe: true
  externalWinsPolicy: typeof GLM_STRESS_EXTERNAL_WINS_POLICY
  blockerRefs: ReadonlyArray<string>
  reasons: ReadonlyArray<GlmExternalWinsProofBlockerReason>
  evidenceRefs: ReadonlyArray<string>
  externalHttpStatus: number
  servedLane: GlmExternalWinsServedLane
  fallbackReason: string | null
  schedulerPreemptionTargetOutcome: 'preempted_yielded' | 'missing'
  usageTotalTokens: number | null
}>

export type GlmExternalWinsOpenAgentsResponseInput = Readonly<{
  body: unknown
  externalHttpStatus: number
}>

const safePositiveInteger = (value: number): number =>
  Number.isFinite(value) && value > 0 ? Math.floor(value) : 0

const isNonEmptyRef = (value: string | undefined): value is string =>
  value !== undefined && value.trim() !== ''

const missingEvidenceReasons = (
  evidence: GlmStressEvidence | undefined,
): Array<GlmStressBlockerReason> => [
  ...(!isNonEmptyRef(evidence?.liveHeadroomEvidenceRef)
    ? (['missing_live_headroom_evidence'] as const)
    : []),
  ...(!isNonEmptyRef(evidence?.externalWinsPreemptionEvidenceRef)
    ? (['missing_external_wins_preemption_evidence'] as const)
    : []),
  ...(!isNonEmptyRef(evidence?.rolloutGuardEvidenceRef)
    ? (['missing_rollout_guard_evidence'] as const)
    : []),
]

const acceptanceGateReasons = (
  evidence: GlmStressEvidence | undefined,
): Array<GlmStressBlockerReason> => {
  const hasExternalWinsEvidence = isNonEmptyRef(
    evidence?.externalWinsPreemptionEvidenceRef,
  )
  const hasRolloutGuardEvidence = isNonEmptyRef(
    evidence?.rolloutGuardEvidenceRef,
  )
  const externalWinsProofNotAccepted =
    hasExternalWinsEvidence && evidence?.externalWinsProofStatus !== 'accepted'
  const throughputRolloutNotAccepted =
    hasRolloutGuardEvidence &&
    evidence?.throughputRolloutCanStartIssue6317Stress !== true

  return [
    ...(externalWinsProofNotAccepted
      ? (['external_wins_proof_not_accepted'] as const)
      : []),
    ...(throughputRolloutNotAccepted
      ? (['throughput_rollout_not_accepted_for_stress'] as const)
      : []),
  ]
}

const blockerRefsFor = (
  reasons: ReadonlyArray<GlmStressBlockerReason>,
): ReadonlyArray<string> =>
  reasons.map(reason => `blocker.glm_continuous_stress.${reason}`)

const stressMatrixConfig = (
  input: GlmStressPlanInput,
): BenchmarkMatrixConfig => ({
  ...input.matrixConfig,
  id: `${input.matrixConfig.id}:continuous-stress-prep`,
  targets: [input.target],
  workloads: [...input.workloads],
  shapes: [...input.shapes],
  transports: ['streaming'],
  samplesPerCell: 1,
})

const onlyAvailableGlmCells = (
  cells: ReadonlyArray<BenchmarkCell>,
): ReadonlyArray<BenchmarkCell> =>
  cells.filter(
    cell => cell.lane === 'glm-52' && cell.laneAvailability === 'available',
  )

export const buildGlmContinuousStressPlan = (
  input: GlmStressPlanInput,
): GlmStressPlan => {
  const cells = onlyAvailableGlmCells(expandMatrix(stressMatrixConfig(input)))
  const evidenceReasons = missingEvidenceReasons(input.evidence)
  const headroom = input.headroom
  const availableSlots =
    headroom === undefined
      ? 0
      : safePositiveInteger(headroom.aggregateAvailableSlots)
  const reservedExternalSlots =
    headroom === undefined
      ? 0
      : safePositiveInteger(headroom.reservedExternalSlots)
  const maxStressConcurrency = Math.max(
    0,
    availableSlots - reservedExternalSlots,
  )
  const headroomUnavailable =
    headroom !== undefined &&
    (maxStressConcurrency === 0 || headroom.externalDemandActive)
  const reasons: Array<GlmStressBlockerReason> = [
    ...evidenceReasons,
    ...acceptanceGateReasons(input.evidence),
    ...(cells.length === 0 ? (['no_glm_stress_cells'] as const) : []),
    ...(headroom === undefined
      ? (['missing_live_headroom_evidence'] as const)
      : []),
    ...(headroomUnavailable
      ? (['external_headroom_unavailable'] as const)
      : []),
  ]
  const uniqueReasons = [...new Set(reasons)]

  if (uniqueReasons.length > 0) {
    return {
      schema: GLM_CONTINUOUS_STRESS_BLOCKER_SCHEMA,
      state: 'blocked_missing_live_scheduler_evidence',
      blockerRefs: blockerRefsFor(uniqueReasons),
      reasons: uniqueReasons,
      demandKind: GLM_STRESS_DEMAND_KIND,
      demandSource: GLM_STRESS_DEMAND_SOURCE,
      demandClient: GLM_STRESS_DEMAND_CLIENT,
      externalWinsPolicy: GLM_STRESS_EXTERNAL_WINS_POLICY,
    }
  }

  return {
    schema: GLM_CONTINUOUS_STRESS_PLAN_SCHEMA,
    state: 'armed',
    demandKind: GLM_STRESS_DEMAND_KIND,
    demandSource: GLM_STRESS_DEMAND_SOURCE,
    demandClient: GLM_STRESS_DEMAND_CLIENT,
    externalWinsPolicy: GLM_STRESS_EXTERNAL_WINS_POLICY,
    reservedExternalSlots,
    maxStressConcurrency,
    cells,
    evidenceRefs: [
      input.evidence!.liveHeadroomEvidenceRef!,
      input.evidence!.externalWinsPreemptionEvidenceRef!,
      input.evidence!.rolloutGuardEvidenceRef!,
    ],
  }
}

const runnerDispatchCell = (
  cell: BenchmarkCell,
  globalMaxConcurrency: number,
): GlmStressRunnerDispatchCell => ({
  cellId: cell.cellId,
  lane: cell.lane,
  engine: cell.engine,
  workload: cell.workload,
  shapeId: cell.shape.id,
  requestedShapeConcurrency: safePositiveInteger(cell.shape.concurrency),
  globalMaxConcurrency,
  transport: cell.transport,
  telemetrySchema: GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA,
  demandKind: GLM_STRESS_DEMAND_KIND,
  demandSource: GLM_STRESS_DEMAND_SOURCE,
  demandClient: GLM_STRESS_DEMAND_CLIENT,
  requestHeaders: {
    'x-openagents-demand-kind': GLM_STRESS_DEMAND_KIND,
    'x-openagents-demand-source': GLM_STRESS_DEMAND_SOURCE,
    'x-openagents-client': GLM_STRESS_DEMAND_CLIENT,
  },
})

export const buildGlmContinuousStressRunnerPlan = (
  input: GlmStressRunnerPlanInput,
): GlmStressRunnerPlan => {
  if (input.plan.state !== 'armed') {
    return {
      schema: GLM_CONTINUOUS_STRESS_RUNNER_PLAN_SCHEMA,
      generatedAt: input.generatedAt,
      tickRef: input.tickRef,
      state: 'blocked',
      publicSafe: true,
      canDispatch: false,
      externalWinsPolicy: GLM_STRESS_EXTERNAL_WINS_POLICY,
      blockerRefs: input.plan.blockerRefs,
      evidenceRefs: [],
      demandKind: GLM_STRESS_DEMAND_KIND,
      demandSource: GLM_STRESS_DEMAND_SOURCE,
      demandClient: GLM_STRESS_DEMAND_CLIENT,
      globalMaxConcurrency: 0,
      reservedExternalSlots: 0,
      dispatchCells: [],
    }
  }
  const plan = input.plan

  return {
    schema: GLM_CONTINUOUS_STRESS_RUNNER_PLAN_SCHEMA,
    generatedAt: input.generatedAt,
    tickRef: input.tickRef,
    state: 'ready',
    publicSafe: true,
    canDispatch: true,
    externalWinsPolicy: GLM_STRESS_EXTERNAL_WINS_POLICY,
    blockerRefs: [],
    evidenceRefs: plan.evidenceRefs,
    demandKind: GLM_STRESS_DEMAND_KIND,
    demandSource: GLM_STRESS_DEMAND_SOURCE,
    demandClient: GLM_STRESS_DEMAND_CLIENT,
    globalMaxConcurrency: plan.maxStressConcurrency,
    reservedExternalSlots: plan.reservedExternalSlots,
    dispatchCells: plan.cells.map(cell =>
      runnerDispatchCell(cell, plan.maxStressConcurrency),
    ),
  }
}

const measuredPositive = (value: number | undefined): number =>
  value === undefined || !Number.isFinite(value) || value <= 0 ? 0 : value

const rateOrNull = (numerator: number, denominator: number): number | null =>
  denominator <= 0 ? null : numerator / denominator

const latencySummary = (
  values: ReadonlyArray<number>,
): GlmStressLatencySummary => ({
  p50: percentile(values, 50),
  p90: percentile(values, 90),
  p99: percentile(values, 99),
  mean: mean(values),
  sampleCount: values.length,
})

const measuredLatencyValues = (
  observations: ReadonlyArray<GlmStressObservation>,
  pick: (observation: GlmStressObservation) => number | undefined,
): ReadonlyArray<number> =>
  observations.flatMap(observation => {
    const value = pick(observation)
    return value === undefined || !Number.isFinite(value) || value <= 0
      ? []
      : [value]
  })

const latencyRollup = (
  observations: ReadonlyArray<GlmStressObservation>,
): GlmStressLatencyRollup => ({
  ttftMs: latencySummary(measuredLatencyValues(observations, o => o.ttftMs)),
  interTokenLatencyP50Ms: latencySummary(
    measuredLatencyValues(observations, o => o.interTokenLatencyP50Ms),
  ),
  interTokenLatencyP90Ms: latencySummary(
    measuredLatencyValues(observations, o => o.interTokenLatencyP90Ms),
  ),
  interTokenLatencyP99Ms: latencySummary(
    measuredLatencyValues(observations, o => o.interTokenLatencyP99Ms),
  ),
})

const observationsWithStatus = (
  observations: ReadonlyArray<GlmStressObservation>,
  status: GlmStressObservationStatus,
): ReadonlyArray<GlmStressObservation> =>
  observations.filter(observation => observation.status === status)

const isOverloadFailure = (observation: GlmStressObservation): boolean => {
  if (observation.status !== 'failed') {
    return false
  }
  if (
    observation.failureKind === 'gateway_overload' ||
    observation.failureKind === 'provider_overload' ||
    observation.failureKind === 'rate_limited' ||
    observation.failureKind === 'timeout'
  ) {
    return true
  }
  const httpStatus = observation.httpStatus
  return (
    httpStatus !== undefined &&
    Number.isFinite(httpStatus) &&
    (httpStatus === 429 || (httpStatus >= 500 && httpStatus <= 599))
  )
}

const backoffReasonRefs = (
  reasons: ReadonlyArray<GlmStressBackoffReason>,
): ReadonlyArray<string> =>
  reasons.map(reason => `backoff.glm_continuous_stress.${reason}`)

const nextLowerConcurrency = (currentConcurrency: number): number => {
  const step = Math.max(
    1,
    Math.ceil(currentConcurrency * GLM_STRESS_BACKOFF_STEP_FRACTION),
  )
  return Math.max(0, currentConcurrency - step)
}

const buildBackoffRecommendation = (
  runnerPlan: GlmStressRunnerPlan,
  failedCount: number,
  observedCount: number,
  overloadFailureCount: number,
): GlmStressBackoffRecommendation => {
  const currentConcurrency = safePositiveInteger(runnerPlan.globalMaxConcurrency)
  const observedErrorRate = rateOrNull(failedCount, observedCount)

  if (runnerPlan.state !== 'ready' || currentConcurrency === 0) {
    return {
      action: 'pause',
      currentConcurrency,
      recommendedNextConcurrency: 0,
      maxStressConcurrency: currentConcurrency,
      errorRateBackoffThreshold: GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD,
      observedErrorRate,
      overloadFailureCount,
      reasonRefs: backoffReasonRefs(['runner_not_ready']),
    }
  }

  const reasons: Array<GlmStressBackoffReason> = [
    ...(observedCount > 0 && failedCount === observedCount
      ? (['all_dispatch_failed'] as const)
      : []),
    ...(observedErrorRate !== null &&
    observedErrorRate > GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD
      ? (['error_rate_over_budget'] as const)
      : []),
    ...(overloadFailureCount > 0
      ? (['overload_failures_observed'] as const)
      : []),
  ]
  const uniqueReasons = [...new Set(reasons)]

  if (uniqueReasons.length === 0) {
    return {
      action: 'hold',
      currentConcurrency,
      recommendedNextConcurrency: currentConcurrency,
      maxStressConcurrency: currentConcurrency,
      errorRateBackoffThreshold: GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD,
      observedErrorRate,
      overloadFailureCount,
      reasonRefs: backoffReasonRefs(['none']),
    }
  }

  return {
    action: currentConcurrency <= 1 ? 'pause' : 'decrease',
    currentConcurrency,
    recommendedNextConcurrency: nextLowerConcurrency(currentConcurrency),
    maxStressConcurrency: currentConcurrency,
    errorRateBackoffThreshold: GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD,
    observedErrorRate,
    overloadFailureCount,
    reasonRefs: backoffReasonRefs(uniqueReasons),
  }
}

const reportThroughput = (
  ok: ReadonlyArray<GlmStressObservation>,
  throughputMeasurementWindowMs: number,
): Readonly<{
  aggregateTokPerSecond: number | null
  goodputTokPerSecond: number | null
  outputTokens: number
  goodputTokens: number
}> => {
  const measurementWindowMs = measuredPositive(throughputMeasurementWindowMs)
  const outputTokens = ok.reduce(
    (sum, observation) => sum + measuredPositive(observation.outputTokens),
    0,
  )
  const goodputTokens = ok.reduce(
    (sum, observation) =>
      sum +
      measuredPositive(observation.goodputTokens ?? observation.outputTokens),
    0,
  )

  return {
    aggregateTokPerSecond:
      measurementWindowMs === 0 || ok.length === 0
        ? null
        : outputTokens / (measurementWindowMs / 1000),
    goodputTokPerSecond:
      measurementWindowMs === 0 || ok.length === 0
        ? null
        : goodputTokens / (measurementWindowMs / 1000),
    outputTokens,
    goodputTokens,
  }
}

const replicaRollup = (
  replicaRef: string,
  observations: ReadonlyArray<GlmStressObservation>,
  throughputMeasurementWindowMs: number,
): GlmStressReplicaRollup => {
  const scoped = observations.filter(
    observation => observation.replicaRef === replicaRef,
  )
  const ok = observationsWithStatus(scoped, 'ok')
  const throughput = reportThroughput(ok, throughputMeasurementWindowMs)

  return {
    replicaRef,
    aggregateTokPerSecond: throughput.aggregateTokPerSecond,
    goodputTokPerSecond: throughput.goodputTokPerSecond,
    outputTokens: throughput.outputTokens,
    goodputTokens: throughput.goodputTokens,
    okCount: ok.length,
    deferredCount: observationsWithStatus(
      scoped,
      'deferred_no_headroom',
    ).length,
    preemptedCount: observationsWithStatus(
      scoped,
      'preempted_for_external',
    ).length,
    failedCount: observationsWithStatus(scoped, 'failed').length,
    latencyMs: latencyRollup(ok),
  }
}

export const buildGlmContinuousStressReport = (
  input: GlmStressReportInput,
): GlmStressReport => {
  const ok = observationsWithStatus(input.observations, 'ok')
  const failed = observationsWithStatus(input.observations, 'failed')
  const deferred = observationsWithStatus(
    input.observations,
    'deferred_no_headroom',
  )
  const preempted = observationsWithStatus(
    input.observations,
    'preempted_for_external',
  )
  const throughputMeasurementWindowMs = measuredPositive(
    input.throughputMeasurementWindowMs,
  )
  const throughput = reportThroughput(ok, throughputMeasurementWindowMs)
  const overloadFailureCount = input.observations.filter(isOverloadFailure)
    .length
  const backoff = buildBackoffRecommendation(
    input.runnerPlan,
    failed.length,
    input.observations.length,
    overloadFailureCount,
  )
  const replicaRefs = [
    ...new Set(
      input.observations.flatMap(observation =>
        isNonEmptyRef(observation.replicaRef) ? [observation.replicaRef] : [],
      ),
    ),
  ].sort()

  return {
    schema: GLM_CONTINUOUS_STRESS_REPORT_SCHEMA,
    generatedAt: input.generatedAt,
    tickRef: input.runnerPlan.tickRef,
    publicSafe: true,
    runnerState: input.runnerPlan.state,
    demandKind: GLM_STRESS_DEMAND_KIND,
    demandSource: GLM_STRESS_DEMAND_SOURCE,
    demandClient: GLM_STRESS_DEMAND_CLIENT,
    telemetrySchema: GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA,
    blockerRefs: input.runnerPlan.blockerRefs,
    evidenceRefs: input.runnerPlan.evidenceRefs,
    throughputMeasurementWindowMs:
      throughputMeasurementWindowMs === 0 ? null : throughputMeasurementWindowMs,
    aggregateTokPerSecond: throughput.aggregateTokPerSecond,
    goodputTokPerSecond: throughput.goodputTokPerSecond,
    errorRate: rateOrNull(failed.length, input.observations.length),
    deferredCount: deferred.length,
    preemptedCount: preempted.length,
    failedCount: failed.length,
    overloadFailureCount,
    okCount: ok.length,
    backoff,
    replicaRefs,
    latencyMs: latencyRollup(ok),
    replicaRollups: replicaRefs.map(replicaRef =>
      replicaRollup(replicaRef, input.observations, throughputMeasurementWindowMs),
    ),
  }
}

export const evaluateGlmExternalWinsProbe = (
  input: GlmExternalWinsProbeInput,
): GlmExternalWinsProofReadout => {
  const schedulerPreemptionMissing =
    !isNonEmptyRef(input.schedulerPreemptionEvidenceRef) ||
    input.schedulerPreemptionTargetOutcome !== 'preempted_yielded'
  const externalRequestFailed =
    !Number.isFinite(input.externalHttpStatus) ||
    input.externalHttpStatus < 200 ||
    input.externalHttpStatus >= 300
  const fallbackAfterPreemption = input.fallbackReason !== null
  const servedLaneNotGlmPrimary = input.servedLane !== 'glm_primary'
  const emptyGlmContentAfterPreemption =
    input.fallbackReason === 'empty_assistant_content'

  const reasons: GlmExternalWinsProofBlockerReason[] = [
    ...(schedulerPreemptionMissing
      ? (['missing_scheduler_preemption'] as const)
      : []),
    ...(externalRequestFailed ? (['external_request_failed'] as const) : []),
    ...(fallbackAfterPreemption ? (['fallback_after_preemption'] as const) : []),
    ...(servedLaneNotGlmPrimary
      ? (['served_lane_not_glm_primary'] as const)
      : []),
    ...(emptyGlmContentAfterPreemption
      ? (['empty_glm_content_after_preemption'] as const)
      : []),
  ]

  return {
    schema: GLM_EXTERNAL_WINS_PROOF_SCHEMA,
    status: reasons.length === 0 ? 'accepted' : 'blocked',
    publicSafe: true,
    externalWinsPolicy: GLM_STRESS_EXTERNAL_WINS_POLICY,
    blockerRefs: reasons.map(reason => `blocker.glm_external_wins.${reason}`),
    reasons,
    evidenceRefs: isNonEmptyRef(input.schedulerPreemptionEvidenceRef)
      ? [input.schedulerPreemptionEvidenceRef]
      : [],
    externalHttpStatus: input.externalHttpStatus,
    servedLane: input.servedLane,
    fallbackReason: input.fallbackReason,
    schedulerPreemptionTargetOutcome:
      input.schedulerPreemptionTargetOutcome ?? 'missing',
    usageTotalTokens:
      input.usageTotalTokens === undefined ||
      !Number.isFinite(input.usageTotalTokens) ||
      input.usageTotalTokens < 0
        ? null
        : input.usageTotalTokens,
  }
}

const recordFromUnknown = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === 'object'
    ? (value as Record<string, unknown>)
    : {}

const nonEmptyStringFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const nullableStringFromUnknown = (value: unknown): string | null =>
  value === null ? null : nonEmptyStringFromUnknown(value) ?? null

const finiteNumberFromUnknown = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const schedulerPreemptionOutcomeFromUnknown = (
  value: unknown,
): 'preempted_yielded' | undefined =>
  value === 'preempted_yielded' ? 'preempted_yielded' : undefined

const servedLaneFromOpenAgentsResponse = (
  openagents: Record<string, unknown>,
): GlmExternalWinsServedLane => {
  const servedModel = nonEmptyStringFromUnknown(openagents.served_model)
  const supplyLane = nonEmptyStringFromUnknown(openagents.supply_lane)
  const worker = nonEmptyStringFromUnknown(openagents.worker)

  if (
    supplyLane === 'hydralisk' &&
    servedModel === GLM_EXTERNAL_WINS_PRIMARY_SERVED_MODEL
  ) {
    return 'glm_primary'
  }
  if (
    supplyLane === 'fireworks' ||
    supplyLane === 'openrouter' ||
    supplyLane === 'vertex-anthropic' ||
    supplyLane === 'vertex-gemini'
  ) {
    return 'weaker_fallback'
  }
  if (
    worker === 'hydralisk-vllm-glm-5p2-reap-504b' &&
    servedModel === GLM_EXTERNAL_WINS_PRIMARY_SERVED_MODEL
  ) {
    return 'glm_primary'
  }
  return 'unknown'
}

export const evaluateGlmExternalWinsOpenAgentsResponse = (
  input: GlmExternalWinsOpenAgentsResponseInput,
): GlmExternalWinsProofReadout => {
  const body = recordFromUnknown(input.body)
  const openagents = recordFromUnknown(body.openagents)
  const routing = recordFromUnknown(openagents.routing)
  const schedulerPreemption = recordFromUnknown(routing.scheduler_preemption)
  const usage = recordFromUnknown(body.usage)

  return evaluateGlmExternalWinsProbe({
    externalHttpStatus: input.externalHttpStatus,
    fallbackReason: nullableStringFromUnknown(routing.fallback_reason),
    schedulerPreemptionEvidenceRef: nonEmptyStringFromUnknown(
      schedulerPreemption.evidence_ref,
    ),
    schedulerPreemptionTargetOutcome: schedulerPreemptionOutcomeFromUnknown(
      schedulerPreemption.target_outcome,
    ),
    servedLane: servedLaneFromOpenAgentsResponse(openagents),
    usageTotalTokens: finiteNumberFromUnknown(usage.total_tokens),
  })
}
