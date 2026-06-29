import { Schema as S } from 'effect'

import {
  BlueprintObjectiveReleaseGate,
  BlueprintObjectiveSurface,
} from './objective'

export const BlueprintProgramFamily = S.Literals([
  'action_planning',
  'artifact_review',
  'context',
  'continuation',
  'email_decisioning',
  'proof_projection',
  'research_policy',
  'review',
  'routing',
  'source_selection',
])
export type BlueprintProgramFamily = typeof BlueprintProgramFamily.Type

export const BlueprintProgramStatus = S.Literals([
  'draft',
  'active',
  'suspended',
  'deprecated',
  'archived',
])
export type BlueprintProgramStatus = typeof BlueprintProgramStatus.Type

export const BlueprintProgramRiskClass = S.Literals([
  'low',
  'medium',
  'high',
  'legal_sensitive',
  'payment_sensitive',
])
export type BlueprintProgramRiskClass = typeof BlueprintProgramRiskClass.Type

export const BlueprintSchemaRefKind = S.Literals([
  'input',
  'output',
  'context',
  'receipt',
])
export type BlueprintSchemaRefKind = typeof BlueprintSchemaRefKind.Type

export const BlueprintUnknownFieldPolicy = S.Literals([
  'reject',
  'strip',
  'preserve',
])
export type BlueprintUnknownFieldPolicy = typeof BlueprintUnknownFieldPolicy.Type

export const BlueprintValidationMode = S.Literals([
  'strict',
  'compatible',
  'advisory',
])
export type BlueprintValidationMode = typeof BlueprintValidationMode.Type

export const BlueprintEvidenceRequirementKind = S.Literals([
  'artifact_ref',
  'context_pack_ref',
  'customer_feedback_ref',
  'human_review_ref',
  'refusal_reason_ref',
  'source_ref',
  'test_result_ref',
])
export type BlueprintEvidenceRequirementKind =
  typeof BlueprintEvidenceRequirementKind.Type

export const BlueprintReceiptRequirementKind = S.Literals([
  'action_submission',
  'deployment',
  'email',
  'program_run',
  'proof_bundle',
  'pull_request',
  'review',
])
export type BlueprintReceiptRequirementKind =
  typeof BlueprintReceiptRequirementKind.Type

export const BlueprintToolAccess = S.Literals([
  'read',
  'evidence',
  'propose_action',
])
export type BlueprintToolAccess = typeof BlueprintToolAccess.Type

export const BlueprintTassadarModuleStepKind = S.Literals([
  'dense_weight_module',
  'linked_dense_module',
])
export type BlueprintTassadarModuleStepKind =
  typeof BlueprintTassadarModuleStepKind.Type

export const BlueprintTassadarModuleStepExecutionMode = S.Literals([
  'fixture_bound',
  'registry_resolved',
])
export type BlueprintTassadarModuleStepExecutionMode =
  typeof BlueprintTassadarModuleStepExecutionMode.Type

export const BlueprintTassadarModuleStepBinding = S.Struct({
  executionMode: BlueprintTassadarModuleStepExecutionMode,
  expectedCapabilityRef: S.String,
  expectedClaimClass: S.String,
  expectedModuleDigest: S.String,
  expectedTraceDigest: S.String,
  expectedTrustPosture: S.String,
  kind: S.Literal('tassadar_module_step'),
  moduleKind: BlueprintTassadarModuleStepKind,
  moduleRef: S.String,
  registryRef: S.String,
  stepRef: S.String,
})
export type BlueprintTassadarModuleStepBinding =
  typeof BlueprintTassadarModuleStepBinding.Type

export const BlueprintReplayModuleBinding = S.Struct({
  allowedReplaySlugs: S.Array(S.String),
  defaultReplaySlug: S.String,
  expectedRuntimeRef: S.String,
  kind: S.Literal('replay_module'),
  moduleRef: S.String,
  stepRef: S.String,
})
export type BlueprintReplayModuleBinding =
  typeof BlueprintReplayModuleBinding.Type

export const BlueprintProgramSchemaRef = S.Struct({
  kind: BlueprintSchemaRefKind,
  schemaRef: S.String,
  versionRef: S.String,
})
export type BlueprintProgramSchemaRef =
  typeof BlueprintProgramSchemaRef.Type

export const BlueprintProgramDecodePolicy = S.Struct({
  validationMode: BlueprintValidationMode,
  validationPolicyRef: S.String,
  unknownFieldPolicy: BlueprintUnknownFieldPolicy,
})
export type BlueprintProgramDecodePolicy =
  typeof BlueprintProgramDecodePolicy.Type

export const BlueprintProgramEvidenceRequirement = S.Struct({
  descriptionRef: S.String,
  kind: BlueprintEvidenceRequirementKind,
  minimumCount: S.Number,
  required: S.Boolean,
})
export type BlueprintProgramEvidenceRequirement =
  typeof BlueprintProgramEvidenceRequirement.Type

export const BlueprintProgramReceiptRequirement = S.Struct({
  kind: BlueprintReceiptRequirementKind,
  receiptRef: S.String,
  required: S.Boolean,
})
export type BlueprintProgramReceiptRequirement =
  typeof BlueprintProgramReceiptRequirement.Type

export const BlueprintProgramToolScope = S.Struct({
  access: BlueprintToolAccess,
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  requiresApproval: S.Boolean,
  replayModule: S.optional(BlueprintReplayModuleBinding),
  tassadarModuleStep: S.optional(BlueprintTassadarModuleStepBinding),
  toolRef: S.String,
})
export type BlueprintProgramToolScope =
  typeof BlueprintProgramToolScope.Type

export const BlueprintProgramType = S.Struct({
  allowedStrategyRefs: S.Array(S.String),
  directMutationAllowed: S.Boolean,
  evidenceRequirements: S.Array(BlueprintProgramEvidenceRequirement),
  family: BlueprintProgramFamily,
  id: S.String,
  instructionRefs: S.Array(S.String),
  instructionsVersionRef: S.String,
  purposeRef: S.String,
  receiptRequirements: S.Array(BlueprintProgramReceiptRequirement),
  releaseGates: S.Array(BlueprintObjectiveReleaseGate),
  riskClass: BlueprintProgramRiskClass,
  status: BlueprintProgramStatus,
  toolScopes: S.Array(BlueprintProgramToolScope),
})
export type BlueprintProgramType = typeof BlueprintProgramType.Type

export const BlueprintProgramSignature = S.Struct({
  decodePolicy: BlueprintProgramDecodePolicy,
  evidenceRequirements: S.Array(BlueprintProgramEvidenceRequirement),
  id: S.String,
  inputSchema: BlueprintProgramSchemaRef,
  outputSchema: BlueprintProgramSchemaRef,
  programTypeId: S.String,
  receiptRequirements: S.Array(BlueprintProgramReceiptRequirement),
  status: BlueprintProgramStatus,
  supportsContext: S.Boolean,
  supportsContinuation: S.Boolean,
  supportsProofProjection: S.Boolean,
  supportsReview: S.Boolean,
  supportsRouting: S.Boolean,
  toolScopes: S.Array(BlueprintProgramToolScope),
  versionRef: S.String,
})
export type BlueprintProgramSignature =
  typeof BlueprintProgramSignature.Type

export const blueprintProgramTypeRequiresApproval = (
  programType: BlueprintProgramType,
): boolean =>
  programType.toolScopes.some(
    scope => scope.access === 'propose_action' || scope.requiresApproval,
  )

export const blueprintProgramTypeRequiredReceiptRefs = (
  programType: BlueprintProgramType,
): ReadonlyArray<string> =>
  programType.receiptRequirements
    .filter(requirement => requirement.required)
    .map(requirement => requirement.receiptRef)

export const blueprintProgramToolScopeIsTassadarModuleStep = (
  scope: BlueprintProgramToolScope,
): boolean => scope.tassadarModuleStep !== undefined

export const blueprintProgramSignatureSupportsFamily = (
  signature: BlueprintProgramSignature,
  family: BlueprintProgramFamily,
): boolean => {
  switch (family) {
    case 'context':
      return signature.supportsContext
    case 'continuation':
      return signature.supportsContinuation
    case 'proof_projection':
      return signature.supportsProofProjection
    case 'review':
      return signature.supportsReview
    case 'routing':
      return signature.supportsRouting
    case 'action_planning':
    case 'artifact_review':
    case 'email_decisioning':
    case 'research_policy':
    case 'source_selection':
      return false
  }
}
