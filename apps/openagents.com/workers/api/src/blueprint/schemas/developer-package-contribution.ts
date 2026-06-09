import { Schema as S } from 'effect'

import { BlueprintProgramFamily, BlueprintProgramRiskClass } from './program'

export const BlueprintDeveloperPackageContributionStatus = S.Literals([
  'approved_for_release_gate',
  'archived',
  'draft',
  'in_review',
  'needs_changes',
  'promoted',
  'rejected',
  'submitted',
])
export type BlueprintDeveloperPackageContributionStatus =
  typeof BlueprintDeveloperPackageContributionStatus.Type

export const BlueprintDeveloperPackageContributionReviewStatus = S.Literals([
  'approved',
  'changes_requested',
  'not_requested',
  'pending',
  'rejected',
])
export type BlueprintDeveloperPackageContributionReviewStatus =
  typeof BlueprintDeveloperPackageContributionReviewStatus.Type

export const BlueprintDeveloperPackageContributionCapabilityFamily = S.Literals(
  [
    'agent_tool',
    'backend_projection_adapter',
    'context_package',
    'outcome_template',
    'program_signature',
    'retrieval_package',
    'route_policy',
    'tool_package',
    'ui_binding',
    'workroom_template',
  ],
)
export type BlueprintDeveloperPackageContributionCapabilityFamily =
  typeof BlueprintDeveloperPackageContributionCapabilityFamily.Type

export const BlueprintDeveloperPackageContributionAuthority = S.Struct({
  canChangePublicClaims: S.Boolean,
  canCreateSite: S.Boolean,
  canDeploy: S.Boolean,
  canDispatchRuntime: S.Boolean,
  canExecute: S.Boolean,
  canMutateRepository: S.Boolean,
  canPostPublicly: S.Boolean,
  canSendEmail: S.Boolean,
  canSpend: S.Boolean,
  deniedEffectRefs: S.Array(S.String),
})
export type BlueprintDeveloperPackageContributionAuthority =
  typeof BlueprintDeveloperPackageContributionAuthority.Type

export const BlueprintDeveloperPackageContributionRecord = S.Struct({
  authority: BlueprintDeveloperPackageContributionAuthority,
  backendProjectionAdapterRefs: S.Array(S.String),
  capabilityFamily: BlueprintDeveloperPackageContributionCapabilityFamily,
  capabilitySummaryRef: S.String,
  contextPackageRefs: S.Array(S.String),
  contributorRefs: S.Array(S.String),
  createdAt: S.String,
  dogfoodScopeRef: S.NullOr(S.String),
  id: S.String,
  intendedProgramFamily: BlueprintProgramFamily,
  noProductionRuntimeAuthority: S.Boolean,
  outcomeTemplateRefs: S.Array(S.String),
  paymentAttributionRefs: S.Array(S.String),
  promotionRef: S.NullOr(S.String),
  proposedModuleVersionRefs: S.Array(S.String),
  proposedProgramSignatureRefs: S.Array(S.String),
  proposedProgramTypeRefs: S.Array(S.String),
  rejectionRef: S.NullOr(S.String),
  releaseGateRefs: S.Array(S.String),
  requiredFixtureRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  reviewStatus: BlueprintDeveloperPackageContributionReviewStatus,
  riskClass: BlueprintProgramRiskClass,
  selfPromotionAttempt: S.Boolean,
  sourceRefs: S.Array(S.String),
  status: BlueprintDeveloperPackageContributionStatus,
  toolPackageRefs: S.Array(S.String),
  uiBindingRefs: S.Array(S.String),
  updatedAt: S.String,
})
export type BlueprintDeveloperPackageContributionRecord =
  typeof BlueprintDeveloperPackageContributionRecord.Type

export const BlueprintDeveloperPackageContributionProjection = S.Struct({
  authority: BlueprintDeveloperPackageContributionAuthority,
  backendProjectionAdapterRefs: S.Array(S.String),
  capabilityFamily: BlueprintDeveloperPackageContributionCapabilityFamily,
  capabilitySummaryRef: S.String,
  contextPackageRefs: S.Array(S.String),
  contributorRefs: S.Array(S.String),
  dogfoodScopeRef: S.NullOr(S.String),
  id: S.String,
  intendedProgramFamily: BlueprintProgramFamily,
  nonAuthoritative: S.Boolean,
  noProductionRuntimeAuthority: S.Boolean,
  outcomeTemplateRefs: S.Array(S.String),
  paymentAttributionRefs: S.Array(S.String),
  promotionRef: S.NullOr(S.String),
  proposedModuleVersionRefs: S.Array(S.String),
  proposedProgramSignatureRefs: S.Array(S.String),
  proposedProgramTypeRefs: S.Array(S.String),
  rejectionRef: S.NullOr(S.String),
  releaseGateReady: S.Boolean,
  releaseGateRefs: S.Array(S.String),
  requiredFixtureRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  reviewStatus: BlueprintDeveloperPackageContributionReviewStatus,
  riskClass: BlueprintProgramRiskClass,
  selfPromotionAttempt: S.Boolean,
  sourceRefs: S.Array(S.String),
  status: BlueprintDeveloperPackageContributionStatus,
  toolPackageRefs: S.Array(S.String),
  uiBindingRefs: S.Array(S.String),
})
export type BlueprintDeveloperPackageContributionProjection =
  typeof BlueprintDeveloperPackageContributionProjection.Type
