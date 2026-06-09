import {
  blueprintProgramRegistryProjection,
  type BlueprintProgramRegistryApiSeed,
} from '../schemas/program-registry'
import type {
  BlueprintProgramFamily,
  BlueprintProgramRiskClass,
  BlueprintProgramType,
} from '../schemas/program'
import {
  type AutopilotContinuationAction,
  AUTOPILOT_CONTINUATION_ACTIONS,
  AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
  AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES,
  AUTOPILOT_CONTINUATION_RELEASE_GATES,
} from './autopilot-continuation-signatures'

const familyForAction = (
  action: AutopilotContinuationAction,
): BlueprintProgramFamily => {
  switch (action) {
    case 'email_decisioning':
      return 'email_decisioning'
    case 'prepare_review':
      return 'review'
    case 'proof_projection':
      return 'proof_projection'
    case 'request_context':
      return 'context'
    case 'research_policy':
      return 'research_policy'
    case 'route_selection':
      return 'routing'
    case 'continue':
    case 'escalate':
    case 'fix':
    case 'retry_account':
    case 'stop':
    case 'summarize':
    case 'test':
      return 'continuation'
  }
}

const riskClassForAction = (
  action: AutopilotContinuationAction,
): BlueprintProgramRiskClass => {
  switch (action) {
    case 'email_decisioning':
    case 'escalate':
    case 'fix':
    case 'prepare_review':
    case 'retry_account':
      return 'medium'
    case 'continue':
    case 'proof_projection':
    case 'research_policy':
    case 'route_selection':
      return 'low'
    case 'request_context':
    case 'stop':
    case 'summarize':
    case 'test':
      return 'low'
  }
}

const signatureForAction = (action: AutopilotContinuationAction) => {
  const signature = AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES.find(
    candidate => candidate.programTypeId === `program_type.autopilot.${action}`,
  )

  return signature!
}

const programTypeForAction = (
  action: AutopilotContinuationAction,
): BlueprintProgramType => {
  const signature = signatureForAction(action)

  return {
    allowedStrategyRefs: [`strategy.autopilot.${action}.evidence_only`],
    directMutationAllowed: false,
    evidenceRequirements: signature.evidenceRequirements,
    family: familyForAction(action),
    id: `program_type.autopilot.${action}`,
    instructionRefs: [`instruction.autopilot.${action}.v1`],
    instructionsVersionRef: `instruction.autopilot.${action}.v1`,
    purposeRef: `purpose.autopilot.${action}`,
    receiptRequirements: signature.receiptRequirements,
    releaseGates: [
      {
        evidenceRefs: [`fixture.autopilot.${action}.v1`],
        gateKind: 'operator_review',
        gateRef: `release_gate.autopilot.${action}.v1`,
        required: true,
      },
    ],
    riskClass: riskClassForAction(action),
    status: 'draft',
    toolScopes: signature.toolScopes,
  }
}

export const AUTOPILOT_CONTINUATION_PROGRAM_TYPES =
  AUTOPILOT_CONTINUATION_ACTIONS.map(programTypeForAction)

export const AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY =
  blueprintProgramRegistryProjection({
    moduleVersions: AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
    programSignatures: AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES,
    programTypes: AUTOPILOT_CONTINUATION_PROGRAM_TYPES,
    releaseGates: AUTOPILOT_CONTINUATION_RELEASE_GATES,
  })

export const AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF =
  'blueprint_registry.autopilot_continuation.seed.v1'

export const AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_API_SEED: BlueprintProgramRegistryApiSeed =
  {
    audience: 'operator',
    method: 'GET',
    path: '/api/blueprint/program-registry',
    projection: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
    safeProjectionPolicyRef:
      'policy.blueprint.operator_safe_registry_projection.v1',
  }
