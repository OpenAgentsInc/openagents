export const BLUEPRINT_REPLAY_PROGRAM_TYPE_ID =
  'program_type.blueprint.proof_replay.archived'
export const BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID =
  'program_signature.blueprint.proof_replay.archived.v1'
export const BLUEPRINT_REPLAY_TOOL_REF = 'tool.blueprint.proof_replay.archived'

export const BLUEPRINT_REPLAY_MODULE_VERSION: any = {
  id: 'module_version.blueprint.proof_replay.archived.v1',
  programSignatureIds: [BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID],
  programTypeId: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  releaseGateRefs: ['release_gate.blueprint.proof_replay.archived.v1'],
  sourceRefs: ['backroom:openagents-prune-20260708-tassadar-psionic'],
  status: 'retired',
  versionRef: 'module_version.blueprint.proof_replay.archived.v1',
} as const

export const BLUEPRINT_REPLAY_PROGRAM_SIGNATURE: any = {
  decodePolicy: {
    unknownFieldPolicy: 'reject',
    validationMode: 'strict',
    validationPolicyRef: 'policy.blueprint.proof_replay.archived',
  },
  evidenceRequirements: ['evidence.blueprint.proof_replay.archived'],
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
  receiptRequirements: ['receipt.blueprint.proof_replay.archived'],
  status: 'retired',
  supportsContext: false,
  supportsContinuation: false,
  supportsProofProjection: false,
  supportsReview: false,
  supportsRouting: false,
  toolScopes: [
    {
      access: 'read',
      allowedSurfaces: ['operator'],
      policy: 'approval_required',
      toolRef: BLUEPRINT_REPLAY_TOOL_REF,
    },
  ],
  versionRef: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
} as const

export const BLUEPRINT_REPLAY_PROGRAM_TYPE: any = {
  allowedStrategyRefs: ['strategy.blueprint.proof_replay.archived'],
  directMutationAllowed: false,
  evidenceRequirements: ['evidence.blueprint.proof_replay.archived'],
  family: 'proof_projection',
  id: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  instructionRefs: ['instruction.blueprint.proof_replay.archived'],
  instructionsVersionRef: 'instruction.blueprint.proof_replay.archived.v1',
  purposeRef: 'purpose.blueprint.proof_replay.archived',
  receiptRequirements: ['receipt.blueprint.proof_replay.archived'],
  releaseGates: [
    {
      evidenceRefs: ['backroom:openagents-prune-20260708-tassadar-psionic'],
      gateKind: 'operator_review',
      gateRef: 'release_gate.blueprint.proof_replay.archived.v1',
      required: true,
    },
  ],
  riskClass: 'low',
  status: 'retired',
  toolScopes: BLUEPRINT_REPLAY_PROGRAM_SIGNATURE.toolScopes,
} as const

export const BLUEPRINT_REPLAY_RELEASE_GATE: any =
  BLUEPRINT_REPLAY_PROGRAM_TYPE.releaseGates[0]

export const blueprintReplaySignatureFixtures = [
  BLUEPRINT_REPLAY_PROGRAM_TYPE,
  BLUEPRINT_REPLAY_PROGRAM_SIGNATURE,
  BLUEPRINT_REPLAY_MODULE_VERSION,
  BLUEPRINT_REPLAY_RELEASE_GATE,
] as const
