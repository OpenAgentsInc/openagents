import { Schema as S } from 'effect'

import {
  type BenchmarkReport,
  checkReportPublicSafety,
} from '../benchmark'
import {
  assertPylonGepaMetricCallPublicRefs,
} from '../../pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from '../../public-ref-format'
import type { CompiledGymExperiment } from './experiment'
import { summarizeGymCostPerAcceptedOutcome } from './flywheel'

export const GymLeaderboardProjectionSchemaVersion =
  'openagents.gym.leaderboard_projection.v1'
export const GymModuleAuthorSplitProjectionSchemaVersion =
  'openagents.gym.module_author_split_projection.v1'
export const AgentClEvalSchemaVersion = 'agentcl_eval.v0'

export const GymModuleAuthorSplitState = S.Literals(['blocked', 'modeled'])
export type GymModuleAuthorSplitState =
  typeof GymModuleAuthorSplitState.Type

export type GymLeaderboardReportInput = Readonly<{
  compiled: CompiledGymExperiment
  report: BenchmarkReport
  reportRef: string
  receiptRef: string
  candidateRef: string
  learningClaim?: GymLearningClaim | undefined
  agentClEval?: AgentClEval | undefined
}>

export type GymLeaderboardRow = Readonly<{
  rank: number
  reportRef: string
  receiptRef: string
  configId: string
  environmentRef: string
  candidateRef: string
  coordinatorRef: string
  acceptedOutcomes: number
  attemptedVerifications: number
  verificationRateBps: number | null
  costPerAcceptedOutcomeMsat: number
  totalCostBasisMsat: number
  cellsExecuted: number
}>

export type GymLeaderboardExcludedReport = Readonly<{
  reportRef: string
  reason:
    | 'not_decision_grade'
    | 'no_accepted_outcomes'
    | 'public_safety_violation'
    | 'agentcl_evidence_missing'
}>

export type GymLeaderboardProjection = Readonly<{
  schemaVersion: typeof GymLeaderboardProjectionSchemaVersion
  projectionRef: string
  rowCount: number
  rows: ReadonlyArray<GymLeaderboardRow>
  excludedReports: ReadonlyArray<GymLeaderboardExcludedReport>
  caveatRefs: ReadonlyArray<string>
}>

export type GymModuleAuthorContribution = Readonly<{
  moduleRef: string
  authorRef: string
  programSignatureRef: string
  evidenceRef: string
  weightBps: number
}>

export type GymModuleAuthorShare = Readonly<
  GymModuleAuthorContribution & {
    shareMsat: number
  }
>

export type GymModuleAuthorSplitProjection = Readonly<{
  schemaVersion: typeof GymModuleAuthorSplitProjectionSchemaVersion
  splitRef: string
  reportRef: string
  ownerArmed: boolean
  state: GymModuleAuthorSplitState
  grossRevenueMsat: number
  contributorShareMsat: number
  shares: ReadonlyArray<GymModuleAuthorShare>
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  authorPayoutClaimAllowed: boolean
  marketplaceListingAllowed: boolean
  settlementMutationAllowed: boolean
}>

export class GymLeaderboardUnsafe extends S.TaggedErrorClass<GymLeaderboardUnsafe>()(
  'GymLeaderboardUnsafe',
  {
    reason: S.String,
  },
) {}

export const GymLearningClaimKind = S.Literals([
  'continually_learns',
  'memory_improves',
])
export type GymLearningClaimKind = typeof GymLearningClaimKind.Type

export const GymLearningClaim = S.Struct({
  claimRef: S.String,
  kind: GymLearningClaimKind,
})
export type GymLearningClaim = typeof GymLearningClaim.Type

export const AgentClEvalMetricSet = S.Struct({
  evalRef: S.String,
  accuracyBps: S.Number,
  sampleCount: S.Number,
})
export type AgentClEvalMetricSet = typeof AgentClEvalMetricSet.Type

export const AgentClGain = S.Struct({
  gainBps: S.Number,
  evidenceRef: S.String,
})
export type AgentClGain = typeof AgentClGain.Type

export const AgentClEval = S.Struct({
  schemaVersion: S.Literal(AgentClEvalSchemaVersion),
  evalRef: S.String,
  baseline: AgentClEvalMetricSet,
  firstPass: AgentClEvalMetricSet,
  frozenSecondPass: AgentClEvalMetricSet,
  heldOut: AgentClEvalMetricSet,
  plasticityGain: AgentClGain,
  stabilityGain: AgentClGain,
  generalizationGain: AgentClGain,
})
export type AgentClEval = typeof AgentClEval.Type

const LEADERBOARD_CAVEATS = [
  'caveat.public.gym.leaderboard.decision_grade_only',
  'caveat.public.gym.leaderboard.public_safe_fields_only',
  'caveat.public.gym.leaderboard.no_fixture_or_synthetic_ranking',
] as const

const MODULE_SPLIT_CAVEATS = [
  'caveat.public.gym.author_split.modeled_not_settled',
  'caveat.public.gym.author_split.no_public_marketplace_authority',
  'caveat.public.gym.author_split.owner_armed_evidence_required',
] as const

const agentClPublicRefs = (agentClEval: AgentClEval): ReadonlyArray<string> => [
  agentClEval.evalRef,
  agentClEval.baseline.evalRef,
  agentClEval.firstPass.evalRef,
  agentClEval.frozenSecondPass.evalRef,
  agentClEval.heldOut.evalRef,
  agentClEval.plasticityGain.evidenceRef,
  agentClEval.stabilityGain.evidenceRef,
  agentClEval.generalizationGain.evidenceRef,
]

const assertPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  try {
    assertPylonGepaMetricCallPublicRefs(label, normalized)
  } catch (error) {
    throw new GymLeaderboardUnsafe({
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

const assertAgentClEvalReady = (
  input: GymLeaderboardReportInput,
): boolean => {
  if (input.learningClaim === undefined) {
    return true
  }

  assertPublicRefs('Gym learning claim refs', [input.learningClaim.claimRef])
  const agentClEval = input.agentClEval
  if (agentClEval === undefined) {
    return false
  }

  assertPublicRefs('Gym AgentCL eval refs', agentClPublicRefs(agentClEval))
  const metrics = [
    agentClEval.baseline,
    agentClEval.firstPass,
    agentClEval.frozenSecondPass,
    agentClEval.heldOut,
  ]
  const gains = [
    agentClEval.plasticityGain,
    agentClEval.stabilityGain,
    agentClEval.generalizationGain,
  ]

  return (
    metrics.every(
      metric =>
        Number.isFinite(metric.accuracyBps) &&
        metric.accuracyBps >= 0 &&
        metric.accuracyBps <= 10_000 &&
        Number.isInteger(metric.sampleCount) &&
        metric.sampleCount > 0,
    ) &&
    gains.every(gain => Number.isFinite(gain.gainBps))
  )
}

const leaderboardCandidateRow = (
  input: GymLeaderboardReportInput,
): GymLeaderboardRow | GymLeaderboardExcludedReport => {
  assertPublicRefs('Gym leaderboard report refs', [
    input.reportRef,
    input.receiptRef,
    input.candidateRef,
    input.compiled.policySelection.environment.ref,
    input.compiled.policySelection.coordinator,
  ])

  const publicSafety = checkReportPublicSafety(input.report)
  if (!publicSafety.safe) {
    return {
      reportRef: input.reportRef,
      reason: 'public_safety_violation',
    }
  }

  if (!input.report.decisionGrade) {
    return {
      reportRef: input.reportRef,
      reason: 'not_decision_grade',
    }
  }

  if (!assertAgentClEvalReady(input)) {
    return {
      reportRef: input.reportRef,
      reason: 'agentcl_evidence_missing',
    }
  }

  const costSummary = summarizeGymCostPerAcceptedOutcome(input.report)
  if (
    costSummary.acceptedOutcomes <= 0 ||
    costSummary.costPerAcceptedOutcomeMsat === null
  ) {
    return {
      reportRef: input.reportRef,
      reason: 'no_accepted_outcomes',
    }
  }

  const attemptedVerifications = input.report.groups.reduce(
    (sum, group) => sum + group.attemptedVerifications,
    0,
  )
  const verificationRateBps =
    attemptedVerifications === 0
      ? null
      : Math.round(
          (costSummary.acceptedOutcomes / attemptedVerifications) * 10_000,
        )

  return {
    rank: 0,
    reportRef: input.reportRef,
    receiptRef: input.receiptRef,
    configId: input.report.configId,
    environmentRef: input.compiled.policySelection.environment.ref,
    candidateRef: input.candidateRef,
    coordinatorRef: input.compiled.policySelection.coordinator,
    acceptedOutcomes: costSummary.acceptedOutcomes,
    attemptedVerifications,
    verificationRateBps,
    costPerAcceptedOutcomeMsat: costSummary.costPerAcceptedOutcomeMsat,
    totalCostBasisMsat: costSummary.totalCostBasisMsat,
    cellsExecuted: input.report.cellsExecuted,
  }
}

const isLeaderboardRow = (
  value: GymLeaderboardRow | GymLeaderboardExcludedReport,
): value is GymLeaderboardRow => 'rank' in value

export const buildGymLeaderboardProjection = (
  inputs: ReadonlyArray<GymLeaderboardReportInput>,
): GymLeaderboardProjection => {
  const candidates = inputs.map(leaderboardCandidateRow)
  const excludedReports = candidates.filter(
    (candidate): candidate is GymLeaderboardExcludedReport =>
      !isLeaderboardRow(candidate),
  )
  const sortedRows = candidates
    .filter(isLeaderboardRow)
    .sort((left, right) => {
      const costDelta =
        left.costPerAcceptedOutcomeMsat - right.costPerAcceptedOutcomeMsat
      if (costDelta !== 0) {
        return costDelta
      }
      const verificationDelta =
        (right.verificationRateBps ?? -1) - (left.verificationRateBps ?? -1)
      if (verificationDelta !== 0) {
        return verificationDelta
      }
      const acceptedDelta = right.acceptedOutcomes - left.acceptedOutcomes
      if (acceptedDelta !== 0) {
        return acceptedDelta
      }
      return left.reportRef.localeCompare(right.reportRef)
    })
    .map((row, index) => ({ ...row, rank: index + 1 }))

  return {
    schemaVersion: GymLeaderboardProjectionSchemaVersion,
    projectionRef: refFor(
      'projection.gym.leaderboard',
      sortedRows.map(row => row.reportRef),
      'empty',
    ),
    rowCount: sortedRows.length,
    rows: sortedRows,
    excludedReports,
    caveatRefs: [...LEADERBOARD_CAVEATS],
  }
}

const contributionBlockers = (
  input: Readonly<{
    ownerArmed: boolean
    report: BenchmarkReport
    grossRevenueMsat: number
    contributorShareMsat: number
    contributions: ReadonlyArray<GymModuleAuthorContribution>
  }>,
): ReadonlyArray<string> => {
  const weightSum = input.contributions.reduce(
    (sum, contribution) => sum + contribution.weightBps,
    0,
  )

  return uniqueRefs([
    ...(!input.ownerArmed
      ? ['blocker.gym.author_split.owner_arming_missing']
      : []),
    ...(!input.report.decisionGrade
      ? ['blocker.gym.author_split.report_not_decision_grade']
      : []),
    ...(input.grossRevenueMsat <= 0
      ? ['blocker.gym.author_split.gross_revenue_missing']
      : []),
    ...(input.contributorShareMsat <= 0
      ? ['blocker.gym.author_split.contributor_share_missing']
      : []),
    ...(input.contributorShareMsat > input.grossRevenueMsat
      ? ['blocker.gym.author_split.contributor_share_exceeds_gross']
      : []),
    ...(input.contributions.length === 0
      ? ['blocker.gym.author_split.composition_evidence_missing']
      : []),
    ...(input.contributions.some(
      contribution =>
        !Number.isInteger(contribution.weightBps) ||
        contribution.weightBps <= 0,
    )
      ? ['blocker.gym.author_split.invalid_contribution_weight']
      : []),
    ...(input.contributions.length > 0 && weightSum !== 10_000
      ? ['blocker.gym.author_split.contribution_weights_must_sum_to_10000']
      : []),
  ])
}

export const modelGymModuleAuthorSplit = (
  input: Readonly<{
    report: BenchmarkReport
    reportRef: string
    ownerArmed: boolean
    grossRevenueMsat: number
    contributorShareMsat: number
    contributions: ReadonlyArray<GymModuleAuthorContribution>
  }>,
): GymModuleAuthorSplitProjection => {
  assertPublicRefs('Gym module author split refs', [
    input.reportRef,
    ...input.contributions.flatMap(contribution => [
      contribution.moduleRef,
      contribution.authorRef,
      contribution.programSignatureRef,
      contribution.evidenceRef,
    ]),
  ])

  if (
    !Number.isInteger(input.grossRevenueMsat) ||
    !Number.isInteger(input.contributorShareMsat) ||
    input.grossRevenueMsat < 0 ||
    input.contributorShareMsat < 0
  ) {
    throw new GymLeaderboardUnsafe({
      reason:
        'Gym module author split revenue amounts must be non-negative integer msat.',
    })
  }

  const blockerRefs = contributionBlockers(input)
  const state: GymModuleAuthorSplitState =
    blockerRefs.length === 0 ? 'modeled' : 'blocked'
  const shares =
    state === 'modeled'
      ? input.contributions.map(contribution => ({
          ...contribution,
          shareMsat: Math.floor(
            (input.contributorShareMsat * contribution.weightBps) / 10_000,
          ),
        }))
      : []

  return {
    schemaVersion: GymModuleAuthorSplitProjectionSchemaVersion,
    splitRef: refFor(
      'split.gym.module_author',
      [input.reportRef, ...input.contributions.map(c => c.moduleRef)],
      'split',
    ),
    reportRef: input.reportRef,
    ownerArmed: input.ownerArmed,
    state,
    grossRevenueMsat: input.grossRevenueMsat,
    contributorShareMsat: input.contributorShareMsat,
    shares,
    evidenceRefs: uniqueRefs(input.contributions.map(c => c.evidenceRef)),
    blockerRefs,
    caveatRefs: [...MODULE_SPLIT_CAVEATS],
    authorPayoutClaimAllowed: false,
    marketplaceListingAllowed: false,
    settlementMutationAllowed: false,
  }
}
