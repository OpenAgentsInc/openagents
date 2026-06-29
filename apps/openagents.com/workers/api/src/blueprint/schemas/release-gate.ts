import { Schema as S } from 'effect'

export const BlueprintEvalFixtureKind = S.Literals([
  'continuation_decision',
  'email_decision',
  'proof_projection',
  'research_policy',
  'route_selection',
  'source_selection',
])
export type BlueprintEvalFixtureKind =
  typeof BlueprintEvalFixtureKind.Type

export const BlueprintEvalFixtureResult = S.Literals([
  'failed',
  'not_run',
  'passed',
])
export type BlueprintEvalFixtureResult =
  typeof BlueprintEvalFixtureResult.Type

export const BlueprintReleaseTargetKind = S.Literals([
  'email_policy',
  'module_version',
  'program_signature',
  'proof_projection',
  'route_selector',
])
export type BlueprintReleaseTargetKind =
  typeof BlueprintReleaseTargetKind.Type

export const BlueprintReleaseGateDecision = S.Literals([
  'approved',
  'blocked',
  'rejected',
])
export type BlueprintReleaseGateDecision =
  typeof BlueprintReleaseGateDecision.Type

export const BlueprintReleaseGateState = S.Literals([
  'blocked',
  'draft',
  'failed',
  'passed',
])
export type BlueprintReleaseGateState =
  typeof BlueprintReleaseGateState.Type

export const BlueprintReleaseReviewState = S.Literals([
  'approved',
  'not_requested',
  'pending',
  'rejected',
])
export type BlueprintReleaseReviewState =
  typeof BlueprintReleaseReviewState.Type

export const BlueprintReleasePolicyState = S.Literals([
  'blocked',
  'compliant',
  'not_checked',
])
export type BlueprintReleasePolicyState =
  typeof BlueprintReleasePolicyState.Type

export const BlueprintRollbackPosture = S.Literals([
  'missing',
  'ready',
  'verified',
])
export type BlueprintRollbackPosture =
  typeof BlueprintRollbackPosture.Type

export const BlueprintEvalFixture = S.Struct({
  evidenceRefs: S.Array(S.String),
  expectedOutputRef: S.String,
  fixtureKind: BlueprintEvalFixtureKind,
  id: S.String,
  inputRef: S.String,
  result: BlueprintEvalFixtureResult,
  scorecardRefs: S.Array(S.String),
})
export type BlueprintEvalFixture = typeof BlueprintEvalFixture.Type

export const BlueprintReleaseGate = S.Struct({
  decidedByRef: S.NullOr(S.String),
  decision: S.NullOr(BlueprintReleaseGateDecision),
  decisionReasonRef: S.NullOr(S.String),
  fixturePassState: BlueprintReleaseGateState,
  fixtureRefs: S.Array(S.String),
  id: S.String,
  policyState: BlueprintReleasePolicyState,
  receiptRefs: S.Array(S.String),
  reviewState: BlueprintReleaseReviewState,
  rollbackPosture: BlueprintRollbackPosture,
  scorecardRef: S.NullOr(S.String),
  selfPromotionAttempt: S.Boolean,
  targetKind: BlueprintReleaseTargetKind,
  targetRef: S.String,
})
export type BlueprintReleaseGate = typeof BlueprintReleaseGate.Type

export const blueprintReleaseGateCanPromote = (
  gate: BlueprintReleaseGate,
): boolean =>
  gate.fixturePassState === 'passed' &&
  gate.reviewState === 'approved' &&
  gate.policyState === 'compliant' &&
  (gate.rollbackPosture === 'ready' || gate.rollbackPosture === 'verified') &&
  gate.scorecardRef !== null &&
  gate.receiptRefs.length > 0 &&
  gate.decision === 'approved' &&
  gate.decidedByRef !== null &&
  !gate.selfPromotionAttempt

export const blueprintReleaseGatePreservesRollbackEvidence = (
  gate: BlueprintReleaseGate,
): boolean =>
  gate.rollbackPosture !== 'missing' &&
  gate.receiptRefs.some(ref => ref.includes('rollback'))
