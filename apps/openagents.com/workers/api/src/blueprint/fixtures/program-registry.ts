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
import type { BlueprintProgramSignature } from '../schemas/program'
import {
  DELIVERY_PIPELINE_PROGRAM_TYPES,
  DELIVERY_PIPELINE_PROGRAMS,
} from '../delivery-pipeline-programs'
import {
  BLUEPRINT_REPLAY_MODULE_VERSION,
  BLUEPRINT_REPLAY_PROGRAM_SIGNATURE,
  BLUEPRINT_REPLAY_PROGRAM_TYPE,
  BLUEPRINT_REPLAY_RELEASE_GATE,
} from './replay-signatures'

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

// ---------------------------------------------------------------------------
// Delivery-pipeline programs (WS-B, OpenAgents #4980).
//
// The delivery-pipeline stage programs (declared on main in
// `../delivery-pipeline-programs`) are evidence-only Blueprint programs
// (`directMutationAllowed: false`, status `draft`). They are folded into the
// registry projection so they appear alongside the Autopilot continuation
// programs. Each stage mints one `BlueprintProgramSignature` whose
// `programTypeId` equals the program type id and whose `outputSchema.schemaRef`
// equals the program's `outputSchemaRef`.
//
// These programs ship no module versions or release gates in the seeded
// registry yet, so their registry entries carry zero module/gate ids until a
// candidate module is promoted through the per-stage `operator_review` gate.
// ---------------------------------------------------------------------------

const deliveryPipelineProgramSignatureFor = (
  program: (typeof DELIVERY_PIPELINE_PROGRAMS)[number],
): BlueprintProgramSignature => ({
  decodePolicy: {
    unknownFieldPolicy: 'reject',
    validationMode: 'strict',
    validationPolicyRef: 'policy.delivery_pipeline.strict_v1',
  },
  evidenceRequirements: program.programType.evidenceRequirements,
  id: `${program.programType.id}.signature.v1`,
  inputSchema: {
    kind: 'input',
    schemaRef: `schema.delivery_pipeline.${program.stage.replace(/-/g, '_')}.input`,
    versionRef: `schema.delivery_pipeline.${program.stage.replace(/-/g, '_')}.input.v1`,
  },
  outputSchema: {
    kind: 'output',
    schemaRef: program.outputSchemaRef,
    versionRef: program.outputSchemaRef,
  },
  programTypeId: program.programType.id,
  receiptRequirements: program.programType.receiptRequirements,
  status: program.programType.status,
  supportsContext: false,
  supportsContinuation: false,
  supportsProofProjection: false,
  supportsReview: true,
  supportsRouting: false,
  toolScopes: program.programType.toolScopes,
  versionRef: `${program.programType.id}.signature.v1`,
})

export const DELIVERY_PIPELINE_PROGRAM_SIGNATURES: ReadonlyArray<BlueprintProgramSignature> =
  DELIVERY_PIPELINE_PROGRAMS.map(deliveryPipelineProgramSignatureFor)

export const AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY =
  blueprintProgramRegistryProjection({
    moduleVersions: [
      ...AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
      BLUEPRINT_REPLAY_MODULE_VERSION,
    ],
    programSignatures: [
      ...AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES,
      ...DELIVERY_PIPELINE_PROGRAM_SIGNATURES,
      BLUEPRINT_REPLAY_PROGRAM_SIGNATURE,
    ],
    programTypes: [
      ...AUTOPILOT_CONTINUATION_PROGRAM_TYPES,
      ...DELIVERY_PIPELINE_PROGRAM_TYPES,
      BLUEPRINT_REPLAY_PROGRAM_TYPE,
    ],
    releaseGates: [
      ...AUTOPILOT_CONTINUATION_RELEASE_GATES,
      BLUEPRINT_REPLAY_RELEASE_GATE,
    ],
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
