import { Schema as S } from 'effect'

export const BlueprintModuleKind = S.Literals([
  'deterministic_reducer',
  'effect_agent_module',
  'human_review_module',
  'model_prompt',
  'optimizer_candidate',
  'runtime_adapter',
])
export type BlueprintModuleKind = typeof BlueprintModuleKind.Type

export const BlueprintModuleLifecycleStatus = S.Literals([
  'draft',
  'candidate',
  'approved',
  'promoted',
  'rolled_back',
  'deprecated',
  'archived',
])
export type BlueprintModuleLifecycleStatus =
  typeof BlueprintModuleLifecycleStatus.Type

export const BlueprintModuleReleaseState = S.Literals([
  'unpromoted',
  'release_candidate',
  'production',
  'rolled_back',
  'deprecated',
])
export type BlueprintModuleReleaseState =
  typeof BlueprintModuleReleaseState.Type

export const BlueprintModuleScorecard = S.Struct({
  higherIsBetter: S.Boolean,
  metricRef: S.String,
  scoreRef: S.String,
  value: S.Number,
})
export type BlueprintModuleScorecard =
  typeof BlueprintModuleScorecard.Type

export const BlueprintModuleProvenance = S.Struct({
  createdByRef: S.String,
  optimizerRunId: S.NullOr(S.String),
  retainedFailureRefs: S.Array(S.String),
  sourceModuleVersionId: S.NullOr(S.String),
  trainingDataRefs: S.Array(S.String),
})
export type BlueprintModuleProvenance =
  typeof BlueprintModuleProvenance.Type

export const BlueprintModuleReleaseDecision = S.Struct({
  decidedAt: S.String,
  decidedByRef: S.String,
  decisionRef: S.String,
  reasonRef: S.String,
  releaseGateRef: S.String,
})
export type BlueprintModuleReleaseDecision =
  typeof BlueprintModuleReleaseDecision.Type

export const BlueprintModuleVersion = S.Struct({
  artifactRefs: S.Array(S.String),
  deprecatedAt: S.NullOr(S.String),
  id: S.String,
  implementationRef: S.String,
  moduleKind: BlueprintModuleKind,
  moduleRef: S.String,
  programSignatureId: S.NullOr(S.String),
  programTypeId: S.String,
  provenance: BlueprintModuleProvenance,
  releaseDecision: S.NullOr(BlueprintModuleReleaseDecision),
  releaseState: BlueprintModuleReleaseState,
  rollbackOfModuleVersionId: S.NullOr(S.String),
  scorecards: S.Array(BlueprintModuleScorecard),
  status: BlueprintModuleLifecycleStatus,
  versionRef: S.String,
})
export type BlueprintModuleVersion = typeof BlueprintModuleVersion.Type

export const blueprintModuleVersionCanSelfPromote = (
  _moduleVersion: BlueprintModuleVersion,
): boolean => false

export const blueprintModuleVersionIsProduction = (
  moduleVersion: BlueprintModuleVersion,
): boolean =>
  moduleVersion.status === 'promoted' &&
  moduleVersion.releaseState === 'production'

export const blueprintModuleVersionRequiresOperatorPromotion = (
  moduleVersion: BlueprintModuleVersion,
): boolean =>
  moduleVersion.moduleKind === 'optimizer_candidate' ||
  moduleVersion.status === 'draft' ||
  moduleVersion.status === 'candidate' ||
  moduleVersion.releaseState === 'unpromoted' ||
  moduleVersion.releaseState === 'release_candidate'

export const blueprintModuleVersionReleaseStateIsValid = (
  moduleVersion: BlueprintModuleVersion,
): boolean => {
  if (
    moduleVersion.releaseState === 'production' &&
    moduleVersion.releaseDecision === null
  ) {
    return false
  }

  if (
    moduleVersion.releaseState === 'rolled_back' &&
    moduleVersion.rollbackOfModuleVersionId === null
  ) {
    return false
  }

  if (
    moduleVersion.releaseState === 'deprecated' &&
    moduleVersion.deprecatedAt === null
  ) {
    return false
  }

  return true
}
