import {
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  LAUNCH_RECOGNITION_REPLAY_SLUG,
} from '@openagentsinc/proof-replay'

import type { BlueprintModuleVersion } from '../schemas/module'
import type {
  BlueprintProgramSignature,
  BlueprintProgramToolScope,
  BlueprintProgramType,
} from '../schemas/program'
import type { BlueprintReleaseGate } from '../schemas/release-gate'

export const BLUEPRINT_REPLAY_TOOL_REF = 'tool.proof_replay.bundle.show'
export const BLUEPRINT_REPLAY_MODULE_REF =
  'module.openagents.public_proof_replay_runtime'
export const BLUEPRINT_REPLAY_RUNTIME_REF =
  'runtime.openagents.public_proof_replay.v1'
export const BLUEPRINT_REPLAY_STEP_REF =
  'step.blueprint.proof_replay.show_replay.v1'
export const BLUEPRINT_REPLAY_PROGRAM_TYPE_ID =
  'program_type.blueprint.show_replay'
export const BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID =
  'program_signature.blueprint.show_replay.v1'
export const BLUEPRINT_REPLAY_MODULE_VERSION_ID =
  'module_version.blueprint.show_replay.public_proof_replay_runtime.v1'

export const BLUEPRINT_REPLAY_ALLOWED_SLUGS = [
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  LAUNCH_RECOGNITION_REPLAY_SLUG,
] as const

export const BLUEPRINT_REPLAY_TOOL_SCOPE: BlueprintProgramToolScope = {
  access: 'evidence',
  allowedSurfaces: ['agent_api', 'omni_workroom', 'operator_dashboard'],
  replayModule: {
    allowedReplaySlugs: [...BLUEPRINT_REPLAY_ALLOWED_SLUGS],
    defaultReplaySlug: FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
    expectedRuntimeRef: BLUEPRINT_REPLAY_RUNTIME_REF,
    kind: 'replay_module',
    moduleRef: BLUEPRINT_REPLAY_MODULE_REF,
    stepRef: BLUEPRINT_REPLAY_STEP_REF,
  },
  requiresApproval: false,
  toolRef: BLUEPRINT_REPLAY_TOOL_REF,
}

const BLUEPRINT_REPLAY_EVIDENCE_REQUIREMENTS = [
  {
    descriptionRef: 'evidence.public_proof_replay_source_required',
    kind: 'source_ref',
    minimumCount: 1,
    required: true,
  },
] as const

const BLUEPRINT_REPLAY_RECEIPT_REQUIREMENTS = [
  {
    kind: 'program_run',
    receiptRef: 'receipt.program_run',
    required: true,
  },
  {
    kind: 'proof_bundle',
    receiptRef: 'receipt.public_proof_replay_bundle',
    required: true,
  },
] as const

export const BLUEPRINT_REPLAY_PROGRAM_TYPE: BlueprintProgramType = {
  allowedStrategyRefs: ['strategy.blueprint.show_replay.evidence_only'],
  directMutationAllowed: false,
  evidenceRequirements: [...BLUEPRINT_REPLAY_EVIDENCE_REQUIREMENTS],
  family: 'proof_projection',
  id: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  instructionRefs: ['instruction.blueprint.show_replay.v1'],
  instructionsVersionRef: 'instruction.blueprint.show_replay.v1',
  purposeRef: 'purpose.blueprint.show_replay',
  receiptRequirements: [...BLUEPRINT_REPLAY_RECEIPT_REQUIREMENTS],
  releaseGates: [
    {
      evidenceRefs: ['fixture.blueprint.show_replay.public_runtime.v1'],
      gateKind: 'proof_bundle_ready',
      gateRef: 'release_gate.blueprint.show_replay.v1',
      required: true,
    },
  ],
  riskClass: 'low',
  status: 'draft',
  toolScopes: [BLUEPRINT_REPLAY_TOOL_SCOPE],
}

export const BLUEPRINT_REPLAY_PROGRAM_SIGNATURE: BlueprintProgramSignature = {
  decodePolicy: {
    unknownFieldPolicy: 'reject',
    validationMode: 'strict',
    validationPolicyRef: 'policy.blueprint.show_replay.strict_v1',
  },
  evidenceRequirements: [...BLUEPRINT_REPLAY_EVIDENCE_REQUIREMENTS],
  id: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
  inputSchema: {
    kind: 'input',
    schemaRef: 'schema.blueprint.ShowReplayInput.v1',
    versionRef: 'schema.blueprint.ShowReplayInput.v1',
  },
  outputSchema: {
    kind: 'output',
    schemaRef: 'schema.blueprint.ShowReplayOutput.v1',
    versionRef: 'schema.blueprint.ShowReplayOutput.v1',
  },
  programTypeId: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  receiptRequirements: [...BLUEPRINT_REPLAY_RECEIPT_REQUIREMENTS],
  status: 'draft',
  supportsContext: false,
  supportsContinuation: false,
  supportsProofProjection: true,
  supportsReview: false,
  supportsRouting: false,
  toolScopes: [BLUEPRINT_REPLAY_TOOL_SCOPE],
  versionRef: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
}

export const BLUEPRINT_REPLAY_MODULE_VERSION: BlueprintModuleVersion = {
  artifactRefs: [
    'artifact.openagents.public_proof_replay_runtime',
    'artifact.openagents.proof_replay_catalog',
  ],
  deprecatedAt: null,
  id: BLUEPRINT_REPLAY_MODULE_VERSION_ID,
  implementationRef: BLUEPRINT_REPLAY_RUNTIME_REF,
  moduleKind: 'runtime_adapter',
  moduleRef: BLUEPRINT_REPLAY_MODULE_REF,
  programSignatureId: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
  programTypeId: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  provenance: {
    createdByRef: 'omega.blueprint.seed',
    optimizerRunId: null,
    retainedFailureRefs: [],
    sourceModuleVersionId: null,
    trainingDataRefs: [
      'fixture.blueprint.show_replay.public_runtime.v1',
      ...BLUEPRINT_REPLAY_ALLOWED_SLUGS.map(
        slug => `proof_replay_catalog.${slug}`,
      ),
    ],
  },
  releaseDecision: null,
  releaseState: 'release_candidate',
  rollbackOfModuleVersionId: null,
  scorecards: [],
  status: 'candidate',
  versionRef: BLUEPRINT_REPLAY_MODULE_VERSION_ID,
}

export const BLUEPRINT_REPLAY_RELEASE_GATE: BlueprintReleaseGate = {
  decidedByRef: null,
  decision: null,
  decisionReasonRef: null,
  fixturePassState: 'draft',
  fixtureRefs: ['fixture.blueprint.show_replay.public_runtime.v1'],
  id: 'release_gate.blueprint.show_replay.v1',
  policyState: 'not_checked',
  receiptRefs: ['receipt.public_proof_replay_bundle'],
  reviewState: 'not_requested',
  rollbackPosture: 'missing',
  scorecardRef: null,
  selfPromotionAttempt: false,
  targetKind: 'program_signature',
  targetRef: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
}
