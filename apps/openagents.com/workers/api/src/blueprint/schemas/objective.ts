import { Schema as S } from 'effect'

import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeAcceptanceState,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from '../../omni-accepted-outcome-contracts'

export const BlueprintObjectiveRunStatus = S.Literals([
  'draft',
  'queued',
  'active',
  'blocked',
  'waiting_review',
  'accepted',
  'rejected',
  'superseded',
  'archived',
])
export type BlueprintObjectiveRunStatus =
  typeof BlueprintObjectiveRunStatus.Type

export const BlueprintObjectiveSurface = S.Literals([
  'agent_api',
  'customer_dashboard',
  'email',
  'github_pull_request',
  'omni_workroom',
  'operator_dashboard',
  'public_site',
  'pylon_desktop',
])
export type BlueprintObjectiveSurface = typeof BlueprintObjectiveSurface.Type

export const BlueprintPolicySeverity = S.Literals([
  'advisory',
  'required',
  'blocking',
])
export type BlueprintPolicySeverity = typeof BlueprintPolicySeverity.Type

export const BlueprintBudgetKind = S.Literals([
  'credits',
  'provider_tokens',
  'sats',
  'time_minutes',
  'usd',
  'human_review_count',
])
export type BlueprintBudgetKind = typeof BlueprintBudgetKind.Type

export const BlueprintBudgetEnforcement = S.Literals(['soft', 'hard'])
export type BlueprintBudgetEnforcement =
  typeof BlueprintBudgetEnforcement.Type

export const BlueprintRiskKind = S.Literals([
  'deployment',
  'legal',
  'payment',
  'private_data',
  'public_content',
  'source_control',
  'customer_reputation',
])
export type BlueprintRiskKind = typeof BlueprintRiskKind.Type

export const BlueprintReleaseGateKind = S.Literals([
  'build_passed',
  'customer_review',
  'deployment_live',
  'email_sent',
  'operator_review',
  'privacy_review',
  'proof_bundle_ready',
  'security_review',
  'source_exported',
  'tests_passed',
])
export type BlueprintReleaseGateKind = typeof BlueprintReleaseGateKind.Type

export const BlueprintObjectiveMetricRef = S.Struct({
  metricRef: S.String,
  required: S.Boolean,
  weight: S.Number,
})
export type BlueprintObjectiveMetricRef =
  typeof BlueprintObjectiveMetricRef.Type

export const BlueprintObjectiveGuardrailPolicy = S.Struct({
  evidenceRefs: S.Array(S.String),
  policyRef: S.String,
  severity: BlueprintPolicySeverity,
})
export type BlueprintObjectiveGuardrailPolicy =
  typeof BlueprintObjectiveGuardrailPolicy.Type

export const BlueprintObjectiveBudgetPolicy = S.Struct({
  budgetKind: BlueprintBudgetKind,
  budgetRef: S.String,
  enforcement: BlueprintBudgetEnforcement,
  limit: S.Number,
})
export type BlueprintObjectiveBudgetPolicy =
  typeof BlueprintObjectiveBudgetPolicy.Type

export const BlueprintObjectiveRiskPolicy = S.Struct({
  mitigationRefs: S.Array(S.String),
  riskKind: BlueprintRiskKind,
  riskRef: S.String,
  severity: BlueprintPolicySeverity,
})
export type BlueprintObjectiveRiskPolicy =
  typeof BlueprintObjectiveRiskPolicy.Type

export const BlueprintObjectiveReleaseGate = S.Struct({
  evidenceRefs: S.Array(S.String),
  gateKind: BlueprintReleaseGateKind,
  gateRef: S.String,
  required: S.Boolean,
})
export type BlueprintObjectiveReleaseGate =
  typeof BlueprintObjectiveReleaseGate.Type

export const BlueprintAcceptedOutcomeLink = S.Struct({
  acceptanceState: OmniAcceptedOutcomeAcceptanceState,
  acceptedOutcomeContractId: S.String,
  publicReceiptRef: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
})
export type BlueprintAcceptedOutcomeLink =
  typeof BlueprintAcceptedOutcomeLink.Type

export const BlueprintObjectiveType = S.Struct({
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  budgetPolicies: S.Array(BlueprintObjectiveBudgetPolicy),
  defaultWorkKind: OmniAcceptedOutcomeWorkKindSchema,
  descriptionRef: S.String,
  guardrailPolicies: S.Array(BlueprintObjectiveGuardrailPolicy),
  id: S.String,
  metricRefs: S.Array(BlueprintObjectiveMetricRef),
  releaseGates: S.Array(BlueprintObjectiveReleaseGate),
  rewardRef: S.String,
  riskPolicies: S.Array(BlueprintObjectiveRiskPolicy),
  titleRef: S.String,
  utilityRef: S.String,
})
export type BlueprintObjectiveType = typeof BlueprintObjectiveType.Type

export const BlueprintObjectiveRun = S.Struct({
  acceptedOutcomeLink: S.NullOr(BlueprintAcceptedOutcomeLink),
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  createdAt: S.String,
  id: S.String,
  objectiveTypeId: S.String,
  outcomeEvidenceRefs: S.Array(S.String),
  programRunId: S.NullOr(S.String),
  releaseGateRefs: S.Array(S.String),
  status: BlueprintObjectiveRunStatus,
  updatedAt: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.NullOr(S.String),
})
export type BlueprintObjectiveRun = typeof BlueprintObjectiveRun.Type

export const blueprintObjectiveTypeAllowsSurface = (
  objectiveType: BlueprintObjectiveType,
  surface: BlueprintObjectiveSurface,
): boolean => objectiveType.allowedSurfaces.includes(surface)

export const blueprintObjectiveRequiredReleaseGateRefs = (
  objectiveType: BlueprintObjectiveType,
): ReadonlyArray<string> =>
  objectiveType.releaseGates
    .filter(gate => gate.required)
    .map(gate => gate.gateRef)

export const blueprintObjectiveRunHasAcceptedOutcome = (
  objectiveRun: BlueprintObjectiveRun,
): boolean => objectiveRun.acceptedOutcomeLink !== null

export const blueprintAcceptedOutcomeWorkKindMatches = (
  objectiveRun: BlueprintObjectiveRun,
  workKind: OmniAcceptedOutcomeWorkKind,
): boolean => objectiveRun.acceptedOutcomeLink?.workKind === workKind
