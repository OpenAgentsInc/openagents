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

export const GLM_STRESS_ROLLOUT_STATE = S.Literals([
  'blocked_missing_live_scheduler_evidence',
  'armed',
])
export type GlmStressRolloutState = typeof GLM_STRESS_ROLLOUT_STATE.Type

export const GlmStressBlockerReason = S.Literals([
  'missing_live_headroom_evidence',
  'missing_external_wins_preemption_evidence',
  'missing_rollout_guard_evidence',
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
  rolloutGuardEvidenceRef?: string | undefined
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

export type GlmStressObservation = Readonly<{
  cellId: string
  replicaRef?: string | undefined
  status: GlmStressObservationStatus
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
  aggregateTokPerSecond: number | null
  goodputTokPerSecond: number | null
  errorRate: number | null
  deferredCount: number
  preemptedCount: number
  failedCount: number
  okCount: number
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

const reportThroughput = (
  ok: ReadonlyArray<GlmStressObservation>,
): Readonly<{
  aggregateTokPerSecond: number | null
  goodputTokPerSecond: number | null
  outputTokens: number
  goodputTokens: number
}> => {
  const wallClockMs = ok.reduce(
    (sum, observation) => sum + measuredPositive(observation.wallClockMs),
    0,
  )
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
      wallClockMs === 0 ? null : outputTokens / (wallClockMs / 1000),
    goodputTokPerSecond:
      wallClockMs === 0 ? null : goodputTokens / (wallClockMs / 1000),
    outputTokens,
    goodputTokens,
  }
}

const replicaRollup = (
  replicaRef: string,
  observations: ReadonlyArray<GlmStressObservation>,
): GlmStressReplicaRollup => {
  const scoped = observations.filter(
    observation => observation.replicaRef === replicaRef,
  )
  const ok = observationsWithStatus(scoped, 'ok')
  const throughput = reportThroughput(ok)

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
  const throughput = reportThroughput(ok)
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
    aggregateTokPerSecond: throughput.aggregateTokPerSecond,
    goodputTokPerSecond: throughput.goodputTokPerSecond,
    errorRate: rateOrNull(failed.length, input.observations.length),
    deferredCount: deferred.length,
    preemptedCount: preempted.length,
    failedCount: failed.length,
    okCount: ok.length,
    replicaRefs,
    latencyMs: latencyRollup(ok),
    replicaRollups: replicaRefs.map(replicaRef =>
      replicaRollup(replicaRef, input.observations),
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
