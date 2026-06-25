import { Schema as S } from 'effect'

import { isMeasured } from '../khala-telemetry'
import type { ServedTokensRecorderInput } from '../served-tokens-recorder'
import type { BenchmarkReport } from '../benchmark'
import type { BenchmarkRunSet } from '../benchmark'
import {
  assertPylonGepaMetricCallPublicRefs,
} from '../../pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from '../../public-ref-format'
import type { CompiledGymExperiment } from './experiment'

export const GymTrainingRewardBundleSchemaVersion =
  'openagents.gym.training_reward_bundle.v1'
export const GymFlywheelEvaluationSchemaVersion =
  'openagents.gym.training_flywheel_evaluation.v1'

export const GymTrainingConsumer = S.Literals([
  'conductor',
  'gepa',
  'trinity',
])
export type GymTrainingConsumer = typeof GymTrainingConsumer.Type

export const GymFlywheelRequestedPromotion = S.Literals([
  'runtime_promotion',
  'shadow',
])
export type GymFlywheelRequestedPromotion =
  typeof GymFlywheelRequestedPromotion.Type

export const GymFlywheelCandidateState = S.Literals([
  'blocked',
  'runtime_promotion_ready',
  'shadow',
])
export type GymFlywheelCandidateState =
  typeof GymFlywheelCandidateState.Type

export type GymTrainingRewardRow = Readonly<{
  rewardRef: string
  routeScorecardRef: string
  candidateRef: string
  environmentRef: string
  lane: BenchmarkReport['groups'][number]['lane']
  workload: BenchmarkReport['groups'][number]['workload']
  acceptedOutcomes: number
  attemptedVerifications: number
  verificationRateBps: number | null
  costPerAcceptedOutcomeMsat: number | null
  scalarReward: number
  consumers: ReadonlyArray<GymTrainingConsumer>
}>

export type GymTrainingRewardBundle = Readonly<{
  schemaVersion: typeof GymTrainingRewardBundleSchemaVersion
  bundleRef: string
  candidateRef: string
  candidateHash: string
  environmentRef: string
  reportRef: string
  decisionGrade: boolean
  consumers: ReadonlyArray<GymTrainingConsumer>
  psionicImportRef: string
  routeScorecardRefs: ReadonlyArray<string>
  rows: ReadonlyArray<GymTrainingRewardRow>
}>

export type GymCostPerAcceptedOutcomeSummary = Readonly<{
  acceptedOutcomes: number
  totalCostBasisMsat: number
  costPerAcceptedOutcomeMsat: number | null
}>

export type GymFlywheelEvaluation = Readonly<{
  schemaVersion: typeof GymFlywheelEvaluationSchemaVersion
  evaluationRef: string
  candidateRef: string
  candidateHash: string
  environmentRef: string
  requestedPromotion: GymFlywheelRequestedPromotion
  candidateState: GymFlywheelCandidateState
  shadowCandidateRef: string | null
  runtimePromotionRef: string | null
  runtimePromotionAllowed: boolean
  reentryExperimentRef: string | null
  blockers: ReadonlyArray<string>
  baseline: GymCostPerAcceptedOutcomeSummary
  candidate: GymCostPerAcceptedOutcomeSummary
  costImprovementBps: number | null
  rewardBundleRefs: ReadonlyArray<string>
  psionicFrontierRefs: ReadonlyArray<string>
  dogfoodEventRefs: ReadonlyArray<string>
}>

export class GymFlywheelUnsafe extends S.TaggedErrorClass<GymFlywheelUnsafe>()(
  'GymFlywheelUnsafe',
  {
    reason: S.String,
  },
) {}

const TRAINING_CONSUMERS: ReadonlyArray<GymTrainingConsumer> = [
  'gepa',
  'trinity',
  'conductor',
]

const assertPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  try {
    assertPylonGepaMetricCallPublicRefs(label, normalized)
  } catch (error) {
    throw new GymFlywheelUnsafe({
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

const scalarRewardFor = (
  acceptedOutcomes: number,
  costPerAcceptedOutcomeMsat: number | null,
): number => {
  if (acceptedOutcomes <= 0 || costPerAcceptedOutcomeMsat === null) {
    return 0
  }
  return Math.round(
    (acceptedOutcomes * 1_000_000) / (1 + costPerAcceptedOutcomeMsat),
  )
}

export const summarizeGymCostPerAcceptedOutcome = (
  report: BenchmarkReport,
): GymCostPerAcceptedOutcomeSummary => {
  const acceptedOutcomes = report.groups.reduce(
    (sum, group) => sum + group.acceptedOutcomes,
    0,
  )
  const totalCostBasisMsat = report.groups.reduce(
    (sum, group) => sum + group.totalCostBasisMsat,
    0,
  )

  return {
    acceptedOutcomes,
    totalCostBasisMsat,
    costPerAcceptedOutcomeMsat:
      acceptedOutcomes === 0 ? null : totalCostBasisMsat / acceptedOutcomes,
  }
}

export const buildGymTrainingRewardBundle = (
  input: Readonly<{
    compiled: CompiledGymExperiment
    report: BenchmarkReport
    reportRef: string
    candidateRef: string
    candidateHash: string
}>,
): GymTrainingRewardBundle => {
  const environmentRef = input.compiled.policySelection.environment.ref
  assertPublicRefs('Gym training reward refs', [
    input.candidateRef,
    input.candidateHash,
    input.reportRef,
    environmentRef,
  ])
  const rows = input.report.groups.map(group => {
    const routeScorecardRef = refFor(
      'route_scorecard.gym.reward',
      [input.reportRef, group.lane, group.workload],
      'group',
    )
    return {
      rewardRef: refFor(
        'reward.gym.training',
        [input.candidateRef, group.lane, group.workload],
        'row',
      ),
      routeScorecardRef,
      candidateRef: input.candidateRef,
      environmentRef,
      lane: group.lane,
      workload: group.workload,
      acceptedOutcomes: group.acceptedOutcomes,
      attemptedVerifications: group.attemptedVerifications,
      verificationRateBps:
        group.verificationRate === null
          ? null
          : Math.round(group.verificationRate * 10_000),
      costPerAcceptedOutcomeMsat: group.costPerAcceptedOutcomeMsat,
      scalarReward: scalarRewardFor(
        group.acceptedOutcomes,
        group.costPerAcceptedOutcomeMsat,
      ),
      consumers: TRAINING_CONSUMERS,
    } satisfies GymTrainingRewardRow
  })
  const routeScorecardRefs = uniqueRefs(rows.map(row => row.routeScorecardRef))
  const bundleRef = refFor(
    'bundle.gym.training_reward',
    [input.candidateRef, input.reportRef],
    'bundle',
  )

  return {
    schemaVersion: GymTrainingRewardBundleSchemaVersion,
    bundleRef,
    candidateRef: input.candidateRef,
    candidateHash: input.candidateHash,
    environmentRef,
    reportRef: input.reportRef,
    decisionGrade: input.report.decisionGrade,
    consumers: TRAINING_CONSUMERS,
    psionicImportRef: refFor('psionic.import.gym_reward', [bundleRef], 'bundle'),
    routeScorecardRefs,
    rows,
  }
}

export const buildGymDogfoodServedTokensInputs = (
  input: Readonly<{
    accountRef: string
    runSet: BenchmarkRunSet
    demandClient?: string | undefined
  }>,
): ReadonlyArray<ServedTokensRecorderInput> =>
  input.runSet.runs.flatMap(run => {
    const record = run.record
    if (
      record === null ||
      run.cell.lane !== 'khala' ||
      !isMeasured(record.promptTokens) ||
      !isMeasured(record.completionTokens) ||
      !isMeasured(record.totalTokens)
    ) {
      return []
    }

    return [
      {
        accountRef: input.accountRef,
        requestedModel: 'openagents/khala',
        servedModel: 'openagents/khala',
        adapterId: record.provider,
        usage: {
          promptTokens: record.promptTokens,
          completionTokens: record.completionTokens,
          totalTokens: record.totalTokens,
          ...(isMeasured(record.cachedInputTokens)
            ? { cachedPromptTokens: record.cachedInputTokens }
            : {}),
        },
        streamed: record.requestClass === 'interactive_stream',
        requestId: refFor('gym_dogfood', [record.requestId], 'run'),
        requestAttribution: {
          demandKind: 'internal',
          demandSource: 'openagents-gym',
          demandClient: input.demandClient ?? 'gym-runner-eval',
        },
      },
    ]
  })

const costImprovementBps = (
  baseline: GymCostPerAcceptedOutcomeSummary,
  candidate: GymCostPerAcceptedOutcomeSummary,
): number | null => {
  if (
    baseline.costPerAcceptedOutcomeMsat === null ||
    candidate.costPerAcceptedOutcomeMsat === null ||
    baseline.costPerAcceptedOutcomeMsat <= 0
  ) {
    return null
  }

  return Math.round(
    ((baseline.costPerAcceptedOutcomeMsat -
      candidate.costPerAcceptedOutcomeMsat) /
      baseline.costPerAcceptedOutcomeMsat) *
      10_000,
  )
}

export const evaluateGymFlywheelCandidate = (
  input: Readonly<{
    compiled: CompiledGymExperiment
    baselineReport: BenchmarkReport
    candidateReport: BenchmarkReport
    baselineReportRef: string
    candidateReportRef: string
    candidateRef: string
    candidateHash: string
    rewardBundleRefs: ReadonlyArray<string>
    psionicFrontierRefs: ReadonlyArray<string>
    dogfoodEventRefs: ReadonlyArray<string>
    requestedPromotion: GymFlywheelRequestedPromotion
    runtimePromotionApprovalRef?: string | undefined
  }>,
): GymFlywheelEvaluation => {
  const environmentRef = input.compiled.policySelection.environment.ref
  assertPublicRefs('Gym flywheel evaluation refs', [
    input.baselineReportRef,
    input.candidateReportRef,
    input.candidateRef,
    input.candidateHash,
    environmentRef,
    ...input.rewardBundleRefs,
    ...input.psionicFrontierRefs,
    ...input.dogfoodEventRefs,
    ...(input.runtimePromotionApprovalRef === undefined
      ? []
      : [input.runtimePromotionApprovalRef]),
  ])
  const rewardBundleRefs = assertPublicRefs(
    'Gym flywheel reward bundle refs',
    input.rewardBundleRefs,
  )
  const psionicFrontierRefs = assertPublicRefs(
    'Gym flywheel Psionic frontier refs',
    input.psionicFrontierRefs,
  )
  const dogfoodEventRefs = assertPublicRefs(
    'Gym flywheel dogfood event refs',
    input.dogfoodEventRefs,
  )
  const baseline = summarizeGymCostPerAcceptedOutcome(input.baselineReport)
  const candidate = summarizeGymCostPerAcceptedOutcome(input.candidateReport)
  const improvementBps = costImprovementBps(baseline, candidate)
  const coreBlockers = uniqueRefs([
    ...(!input.baselineReport.decisionGrade
      ? ['blocker.gym.flywheel.baseline_not_decision_grade']
      : []),
    ...(!input.candidateReport.decisionGrade
      ? ['blocker.gym.flywheel.candidate_not_decision_grade']
      : []),
    ...(candidate.acceptedOutcomes <= 0
      ? ['blocker.gym.flywheel.no_candidate_accepted_outcomes']
      : []),
    ...(improvementBps === null || improvementBps <= 0
      ? ['blocker.gym.flywheel.candidate_not_cheaper_than_heuristic']
      : []),
    ...(rewardBundleRefs.length === 0
      ? ['blocker.gym.flywheel.missing_reward_bundle']
      : []),
    ...(psionicFrontierRefs.length === 0
      ? ['blocker.gym.flywheel.missing_psionic_frontier']
      : []),
    ...(dogfoodEventRefs.length === 0
      ? ['blocker.gym.flywheel.missing_khala_dogfood_attribution']
      : []),
  ])
  const shadowReady = coreBlockers.length === 0
  const runtimeApprovalMissing =
    input.requestedPromotion === 'runtime_promotion' &&
    input.runtimePromotionApprovalRef === undefined
  const blockers = uniqueRefs([
    ...coreBlockers,
    ...(runtimeApprovalMissing
      ? ['blocker.gym.flywheel.runtime_promotion_requires_approval']
      : []),
  ])
  const runtimePromotionAllowed =
    shadowReady &&
    input.requestedPromotion === 'runtime_promotion' &&
    input.runtimePromotionApprovalRef !== undefined
  const candidateState: GymFlywheelCandidateState = !shadowReady
    ? 'blocked'
    : runtimePromotionAllowed
      ? 'runtime_promotion_ready'
      : 'shadow'
  const shadowCandidateRef = shadowReady
    ? refFor('candidate.gym.shadow', [input.candidateRef], 'candidate')
    : null
  const runtimePromotionRef = runtimePromotionAllowed
    ? refFor(
        'runtime_promotion.gym',
        [
          input.candidateRef,
          input.runtimePromotionApprovalRef ?? 'approval',
        ],
        'candidate',
      )
    : null

  return {
    schemaVersion: GymFlywheelEvaluationSchemaVersion,
    evaluationRef: refFor(
      'evaluation.gym.flywheel',
      [
        input.baselineReportRef,
        input.candidateReportRef,
        input.candidateRef,
      ],
      'evaluation',
    ),
    candidateRef: input.candidateRef,
    candidateHash: input.candidateHash,
    environmentRef,
    requestedPromotion: input.requestedPromotion,
    candidateState,
    shadowCandidateRef,
    runtimePromotionRef,
    runtimePromotionAllowed,
    reentryExperimentRef: shadowReady
      ? refFor(
          'experiment.gym.head_to_head_reentry',
          [environmentRef, input.candidateRef],
          'candidate',
        )
      : null,
    blockers,
    baseline,
    candidate,
    costImprovementBps: improvementBps,
    rewardBundleRefs,
    psionicFrontierRefs,
    dogfoodEventRefs,
  }
}
