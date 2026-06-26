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
