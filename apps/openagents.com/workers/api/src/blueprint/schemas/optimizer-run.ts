import { Schema as S } from 'effect'

import {
  type BlueprintModuleVersion,
  blueprintModuleVersionCanSelfPromote,
  blueprintModuleVersionIsProduction,
  blueprintModuleVersionRequiresOperatorPromotion,
} from './module'

export const BlueprintOptimizerKind = S.Literals([
  'ablation',
  'gepa_style_reflection',
  'human_curated',
  'retained_failure_replay',
  'scorecard_search',
])
export type BlueprintOptimizerKind = typeof BlueprintOptimizerKind.Type

export const BlueprintOptimizerRunStatus = S.Literals([
  'draft',
  'running',
  'completed',
  'failed',
  'archived',
])
export type BlueprintOptimizerRunStatus =
  typeof BlueprintOptimizerRunStatus.Type

export const BlueprintOptimizerCandidateState = S.Literals([
  'candidate',
  'discarded',
  'needs_review',
  'retained',
])
export type BlueprintOptimizerCandidateState =
  typeof BlueprintOptimizerCandidateState.Type

export const BlueprintOptimizerCandidateModule = S.Struct({
  candidateState: BlueprintOptimizerCandidateState,
  candidateSummaryRef: S.String,
  moduleVersionId: S.String,
  releaseGateRef: S.NullOr(S.String),
  scorecardRefs: S.Array(S.String),
})
export type BlueprintOptimizerCandidateModule =
  typeof BlueprintOptimizerCandidateModule.Type

export const BlueprintOptimizerRun = S.Struct({
  candidateModules: S.Array(BlueprintOptimizerCandidateModule),
  createdAt: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  optimizerKind: BlueprintOptimizerKind,
  retainedFailureRefs: S.Array(S.String),
  scorecardRefs: S.Array(S.String),
  status: BlueprintOptimizerRunStatus,
  updatedAt: S.String,
})
export type BlueprintOptimizerRun = typeof BlueprintOptimizerRun.Type

export const blueprintOptimizerRunHasCandidateModules = (
  optimizerRun: BlueprintOptimizerRun,
): boolean => optimizerRun.candidateModules.length > 0

export const blueprintOptimizerCandidateRequiresReleaseGate = (
  candidate: BlueprintOptimizerCandidateModule,
): boolean => candidate.releaseGateRef !== null

export const blueprintOptimizerOutputIsEvidenceOnly = (
  optimizerRun: BlueprintOptimizerRun,
  moduleVersions: ReadonlyArray<BlueprintModuleVersion>,
): boolean =>
  optimizerRun.candidateModules.every(candidate => {
    const moduleVersion = moduleVersions.find(
      item => item.id === candidate.moduleVersionId,
    )

    return (
      moduleVersion !== undefined &&
      !blueprintModuleVersionCanSelfPromote(moduleVersion) &&
      !blueprintModuleVersionIsProduction(moduleVersion) &&
      blueprintModuleVersionRequiresOperatorPromotion(moduleVersion)
    )
  })
