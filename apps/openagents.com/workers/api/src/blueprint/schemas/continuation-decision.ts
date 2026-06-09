import { Schema as S } from 'effect'

import {
  BlueprintProgramRunAuthorityBoundary,
} from './program-run'

export const BlueprintContinuationDirectEffectKind = S.Literals([
  'create_pull_request',
  'deploy',
  'mutate_source_fact',
  'send_email',
  'spend_money',
  'upgrade_public_claim',
])
export type BlueprintContinuationDirectEffectKind =
  typeof BlueprintContinuationDirectEffectKind.Type

export const BlueprintContinuationDecisionKind = S.Literals([
  'continue',
  'escalate',
  'fix',
  'prepare_review',
  'request_context',
  'retry_account',
  'stop',
  'summarize',
  'test',
])
export type BlueprintContinuationDecisionKind =
  typeof BlueprintContinuationDecisionKind.Type

export const BlueprintContinuationTurnState = S.Literals([
  'blocked',
  'completed',
  'failed',
  'interrupted',
])
export type BlueprintContinuationTurnState =
  typeof BlueprintContinuationTurnState.Type

export const BlueprintContinuationTurnResult = S.Struct({
  accountFailureRefs: S.Array(S.String),
  actorRef: S.String,
  blockerRefs: S.Array(S.String),
  buildFailureRefs: S.Array(S.String),
  classifierConfidence: S.Number,
  constraintRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  missingContextRefs: S.Array(S.String),
  readyArtifactRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  runtimeFailureRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  state: BlueprintContinuationTurnState,
  summaryNeeded: S.Boolean,
  testFailureRefs: S.Array(S.String),
  unverifiedChangeRefs: S.Array(S.String),
  updatedAt: S.String,
  userRequestedEscalation: S.Boolean,
  userRequestedStop: S.Boolean,
  workRef: S.String,
})
export type BlueprintContinuationTurnResult =
  typeof BlueprintContinuationTurnResult.Type

export const BlueprintContinuationDecision = S.Struct({
  action: BlueprintContinuationDecisionKind,
  actionSubmissionRequiredForDirectEffects: S.Boolean,
  authorityBoundary: BlueprintProgramRunAuthorityBoundary,
  confidence: S.Number,
  constraintRefs: S.Array(S.String),
  decisionRef: S.String,
  directMutationDisabled: S.Boolean,
  evidenceRefs: S.Array(S.String),
  forbiddenDirectEffects: S.Array(BlueprintContinuationDirectEffectKind),
  moduleVersionId: S.NullOr(S.String),
  noDeploy: S.Boolean,
  noEmail: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
  programSignatureId: S.String,
  programTypeId: S.String,
  reason: S.String,
  receiptRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  turnResultRef: S.String,
  workRef: S.String,
})
export type BlueprintContinuationDecision =
  typeof BlueprintContinuationDecision.Type

export const blueprintContinuationDecisionIsEvidenceOnly = (
  decision: BlueprintContinuationDecision,
): boolean =>
  decision.authorityBoundary === 'evidence_only' &&
  decision.directMutationDisabled &&
  decision.noDeploy &&
  decision.noEmail &&
  decision.noPublicClaimUpgrade &&
  decision.noSourceMutation &&
  decision.noSpend &&
  decision.actionSubmissionRequiredForDirectEffects
