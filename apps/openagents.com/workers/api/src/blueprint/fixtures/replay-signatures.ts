import type { BlueprintModuleVersion } from '../schemas/module'
import type {
  BlueprintProgramSignature,
  BlueprintProgramType,
} from '../schemas/program'
import type { BlueprintReleaseGate } from '../schemas/release-gate'

// ---------------------------------------------------------------------------
// Archived ShowReplay/proof-replay Blueprint fixtures.
//
// The original `@openagentsinc/proof-replay`-backed ShowReplay module was
// retired in the Tassadar/Psionic prune (backroom:
// openagents-prune-20260708-tassadar-psionic). These fixtures keep the
// program-registry projection seeded with an inert, schema-conformant
// "archived" entry for that program type instead of restoring any retired
// runtime behavior. Every field below fully satisfies the
// `BlueprintModuleVersion` / `BlueprintProgramSignature` /
// `BlueprintProgramType` / `BlueprintReleaseGate` schemas (no shortcuts via
// `any`) so `blueprintProgramRegistryProjection` can compute a safe
// projection for it without dereferencing missing fields.
// ---------------------------------------------------------------------------

const BLUEPRINT_REPLAY_ARCHIVED_AT = '2026-07-08T00:00:00.000Z'
const BLUEPRINT_REPLAY_ARCHIVE_SOURCE_REF =
  'backroom:openagents-prune-20260708-tassadar-psionic'

export const BLUEPRINT_REPLAY_PROGRAM_TYPE_ID =
  'program_type.blueprint.proof_replay.archived'
export const BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID =
  'program_signature.blueprint.proof_replay.archived.v1'
export const BLUEPRINT_REPLAY_MODULE_VERSION_ID =
  'module_version.blueprint.proof_replay.archived.v1'
export const BLUEPRINT_REPLAY_RELEASE_GATE_ID =
  'release_gate.blueprint.proof_replay.archived.v1'
export const BLUEPRINT_REPLAY_TOOL_REF = 'tool.blueprint.proof_replay.archived'

export const BLUEPRINT_REPLAY_MODULE_VERSION: BlueprintModuleVersion = {
  artifactRefs: [],
  deprecatedAt: BLUEPRINT_REPLAY_ARCHIVED_AT,
  id: BLUEPRINT_REPLAY_MODULE_VERSION_ID,
  implementationRef: 'runtime.blueprint.proof_replay.archived.v1',
  moduleKind: 'runtime_adapter',
  moduleRef: 'module.blueprint.proof_replay.archived',
  programSignatureId: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
  programTypeId: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  provenance: {
    createdByRef: BLUEPRINT_REPLAY_ARCHIVE_SOURCE_REF,
    optimizerRunId: null,
    retainedFailureRefs: [],
    sourceModuleVersionId: null,
    trainingDataRefs: [],
  },
  releaseDecision: null,
  releaseState: 'deprecated',
  rollbackOfModuleVersionId: null,
  scorecards: [],
  status: 'archived',
  versionRef: BLUEPRINT_REPLAY_MODULE_VERSION_ID,
}

export const BLUEPRINT_REPLAY_PROGRAM_SIGNATURE: BlueprintProgramSignature = {
  decodePolicy: {
    unknownFieldPolicy: 'reject',
    validationMode: 'strict',
    validationPolicyRef: 'policy.blueprint.proof_replay.archived',
  },
  evidenceRequirements: [
    {
      descriptionRef: 'evidence.blueprint.proof_replay.archived',
      kind: 'source_ref',
      minimumCount: 0,
      required: false,
    },
  ],
  id: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
  inputSchema: {
    kind: 'input',
    schemaRef: 'schema.blueprint.proof_replay.archived.input',
    versionRef: 'schema.blueprint.proof_replay.archived.input.v1',
  },
  outputSchema: {
    kind: 'output',
    schemaRef: 'schema.blueprint.proof_replay.archived.output',
    versionRef: 'schema.blueprint.proof_replay.archived.output.v1',
  },
  programTypeId: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  receiptRequirements: [
    {
      kind: 'proof_bundle',
      receiptRef: 'receipt.blueprint.proof_replay.archived',
      required: false,
    },
  ],
  status: 'archived',
  supportsContext: false,
  supportsContinuation: false,
  supportsProofProjection: false,
  supportsReview: false,
  supportsRouting: false,
  toolScopes: [
    {
      access: 'read',
      allowedSurfaces: ['operator_dashboard'],
      requiresApproval: true,
      toolRef: BLUEPRINT_REPLAY_TOOL_REF,
    },
  ],
  versionRef: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
}

export const BLUEPRINT_REPLAY_PROGRAM_TYPE: BlueprintProgramType = {
  allowedStrategyRefs: ['strategy.blueprint.proof_replay.archived'],
  directMutationAllowed: false,
  evidenceRequirements: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE.evidenceRequirements,
  family: 'proof_projection',
  id: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  instructionRefs: ['instruction.blueprint.proof_replay.archived'],
  instructionsVersionRef: 'instruction.blueprint.proof_replay.archived.v1',
  purposeRef: 'purpose.blueprint.proof_replay.archived',
  receiptRequirements: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE.receiptRequirements,
  releaseGates: [
    {
      evidenceRefs: [BLUEPRINT_REPLAY_ARCHIVE_SOURCE_REF],
      gateKind: 'operator_review',
      gateRef: BLUEPRINT_REPLAY_RELEASE_GATE_ID,
      required: true,
    },
  ],
  riskClass: 'low',
  status: 'archived',
  toolScopes: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE.toolScopes,
}

export const BLUEPRINT_REPLAY_RELEASE_GATE: BlueprintReleaseGate = {
  decidedByRef: null,
  decision: null,
  decisionReasonRef: null,
  fixturePassState: 'blocked',
  fixtureRefs: [BLUEPRINT_REPLAY_ARCHIVE_SOURCE_REF],
  id: BLUEPRINT_REPLAY_RELEASE_GATE_ID,
  policyState: 'not_checked',
  receiptRefs: [],
  reviewState: 'not_requested',
  rollbackPosture: 'missing',
  scorecardRef: null,
  selfPromotionAttempt: false,
  targetKind: 'program_signature',
  targetRef: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
}

export const blueprintReplaySignatureFixtures = [
  BLUEPRINT_REPLAY_PROGRAM_TYPE,
  BLUEPRINT_REPLAY_PROGRAM_SIGNATURE,
  BLUEPRINT_REPLAY_MODULE_VERSION,
  BLUEPRINT_REPLAY_RELEASE_GATE,
] as const
