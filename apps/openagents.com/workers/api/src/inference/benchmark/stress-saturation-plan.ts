import { Schema as S } from 'effect'

import type {
  BenchmarkCell,
  BenchmarkMatrixConfig,
  BenchmarkTarget,
  BenchmarkWorkload,
  SequenceShape,
} from './matrix'
import { expandMatrix } from './matrix'

export const GLM_CONTINUOUS_STRESS_PLAN_SCHEMA =
  'openagents.khala.glm_continuous_stress_plan.v0_1' as const

export const GLM_CONTINUOUS_STRESS_BLOCKER_SCHEMA =
  'openagents.khala.glm_continuous_stress_blocker.v0_1' as const

export const GLM_CONTINUOUS_STRESS_RUNNER_PLAN_SCHEMA =
  'openagents.khala.glm_continuous_stress_runner_plan.v0_1' as const

export const GLM_CONTINUOUS_STRESS_REPORT_SCHEMA =
  'openagents.khala.glm_continuous_stress_report.v0_1' as const

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
    'x-openagents-demand-client': typeof GLM_STRESS_DEMAND_CLIENT
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
    'x-openagents-demand-client': GLM_STRESS_DEMAND_CLIENT,
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

export const buildGlmContinuousStressReport = (
  input: GlmStressReportInput,
): GlmStressReport => {
  const ok = input.observations.filter(observation => observation.status === 'ok')
  const failed = input.observations.filter(
    observation => observation.status === 'failed',
  )
  const deferred = input.observations.filter(
    observation => observation.status === 'deferred_no_headroom',
  )
  const preempted = input.observations.filter(
    observation => observation.status === 'preempted_for_external',
  )
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
    aggregateTokPerSecond:
      wallClockMs === 0 ? null : outputTokens / (wallClockMs / 1000),
    goodputTokPerSecond:
      wallClockMs === 0 ? null : goodputTokens / (wallClockMs / 1000),
    errorRate: rateOrNull(failed.length, input.observations.length),
    deferredCount: deferred.length,
    preemptedCount: preempted.length,
    failedCount: failed.length,
    okCount: ok.length,
    replicaRefs,
  }
}
