import { Schema as S } from 'effect'

import {
  BlueprintProgramFamily,
  BlueprintProgramRiskClass,
} from './program'

export const BlueprintSignatureContributionStatus = S.Literals([
  'approved_for_release_gate',
  'archived',
  'draft',
  'in_review',
  'needs_changes',
  'promoted',
  'rejected',
  'submitted',
])
export type BlueprintSignatureContributionStatus =
  typeof BlueprintSignatureContributionStatus.Type

export const BlueprintSignatureContributionReviewStatus = S.Literals([
  'approved',
  'changes_requested',
  'not_requested',
  'pending',
  'rejected',
])
export type BlueprintSignatureContributionReviewStatus =
  typeof BlueprintSignatureContributionReviewStatus.Type

export const BlueprintSignatureContributionAuthority = S.Struct({
  canChangePublicClaims: S.Boolean,
  canDeploy: S.Boolean,
  canExecute: S.Boolean,
  canMutate: S.Boolean,
  canSendEmail: S.Boolean,
  canSpend: S.Boolean,
  deniedEffectRefs: S.Array(S.String),
})
export type BlueprintSignatureContributionAuthority =
  typeof BlueprintSignatureContributionAuthority.Type

export const BlueprintSignatureContributionDraft = S.Struct({
  authority: BlueprintSignatureContributionAuthority,
  capabilitySummaryRef: S.String,
  contributorRefs: S.Array(S.String),
  createdAt: S.String,
  id: S.String,
  intendedFamily: BlueprintProgramFamily,
  promotionRef: S.NullOr(S.String),
  proposedModuleVersionRef: S.NullOr(S.String),
  proposedProgramSignatureRef: S.NullOr(S.String),
  proposedProgramTypeRef: S.NullOr(S.String),
  rejectionRef: S.NullOr(S.String),
  releaseGateRefs: S.Array(S.String),
  requiredFixtureRefs: S.Array(S.String),
  reviewStatus: BlueprintSignatureContributionReviewStatus,
  riskClass: BlueprintProgramRiskClass,
  sourceRefs: S.Array(S.String),
  status: BlueprintSignatureContributionStatus,
  updatedAt: S.String,
})
export type BlueprintSignatureContributionDraft =
  typeof BlueprintSignatureContributionDraft.Type

export const BlueprintSignatureContributionProjection = S.Struct({
  authority: BlueprintSignatureContributionAuthority,
  capabilitySummaryRef: S.String,
  contributorRefs: S.Array(S.String),
  id: S.String,
  intendedFamily: BlueprintProgramFamily,
  nonAuthoritative: S.Boolean,
  promotionRef: S.NullOr(S.String),
  proposedModuleVersionRef: S.NullOr(S.String),
  proposedProgramSignatureRef: S.NullOr(S.String),
  proposedProgramTypeRef: S.NullOr(S.String),
  rejectionRef: S.NullOr(S.String),
  releaseGateRefs: S.Array(S.String),
  requiredFixtureRefs: S.Array(S.String),
  reviewStatus: BlueprintSignatureContributionReviewStatus,
  riskClass: BlueprintProgramRiskClass,
  sourceRefs: S.Array(S.String),
  status: BlueprintSignatureContributionStatus,
})
export type BlueprintSignatureContributionProjection =
  typeof BlueprintSignatureContributionProjection.Type
