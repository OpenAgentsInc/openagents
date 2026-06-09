import type { BlueprintModuleVersion } from '../schemas/module'
import type { BlueprintProgramSignature } from '../schemas/program'
import type { BlueprintReleaseGate } from '../schemas/release-gate'

export const AUTOPILOT_CONTINUATION_ACTIONS = [
  'continue',
  'email_decisioning',
  'escalate',
  'fix',
  'prepare_review',
  'proof_projection',
  'request_context',
  'research_policy',
  'retry_account',
  'route_selection',
  'stop',
  'summarize',
  'test',
] as const

export type AutopilotContinuationAction =
  (typeof AUTOPILOT_CONTINUATION_ACTIONS)[number]

const signatureForAction = (
  action: AutopilotContinuationAction,
): BlueprintProgramSignature => ({
  decodePolicy: {
    unknownFieldPolicy: 'reject',
    validationMode: 'strict',
    validationPolicyRef: 'policy.autopilot_continuation.strict_v1',
  },
  evidenceRequirements: [
    {
      descriptionRef: 'evidence.context_pack_required',
      kind: 'context_pack_ref',
      minimumCount: 1,
      required: true,
    },
  ],
  id: `program_signature.autopilot.${action}.v1`,
  inputSchema: {
    kind: 'input',
    schemaRef: `schema.autopilot.${action}.input`,
    versionRef: `schema.autopilot.${action}.input.v1`,
  },
  outputSchema: {
    kind: 'output',
    schemaRef: `schema.autopilot.${action}.output`,
    versionRef: `schema.autopilot.${action}.output.v1`,
  },
  programTypeId: `program_type.autopilot.${action}`,
  receiptRequirements: [
    {
      kind: 'program_run',
      receiptRef: 'receipt.program_run',
      required: true,
    },
  ],
  status: 'draft',
  supportsContext: action === 'request_context',
  supportsContinuation: [
    'continue',
    'escalate',
    'fix',
    'prepare_review',
    'retry_account',
    'stop',
    'summarize',
    'test',
  ].includes(action),
  supportsProofProjection: action === 'proof_projection',
  supportsReview: action === 'prepare_review',
  supportsRouting: action === 'route_selection',
  toolScopes: [
    {
      access: 'evidence',
      allowedSurfaces: ['agent_api', 'omni_workroom', 'operator_dashboard'],
      requiresApproval: false,
      toolRef: 'tool.context_pack.read',
    },
    {
      access: 'propose_action',
      allowedSurfaces: ['operator_dashboard'],
      requiresApproval: true,
      toolRef: 'tool.action_submission.propose',
    },
  ],
  versionRef: `program_signature.autopilot.${action}.v1`,
})

const moduleForAction = (
  action: AutopilotContinuationAction,
): BlueprintModuleVersion => ({
  artifactRefs: [`artifact.autopilot.${action}.candidate_prompt`],
  deprecatedAt: null,
  id: `module_version.autopilot.${action}.candidate_1`,
  implementationRef: `prompt.autopilot.${action}.candidate_1`,
  moduleKind: 'model_prompt',
  moduleRef: `module.autopilot.${action}`,
  programSignatureId: `program_signature.autopilot.${action}.v1`,
  programTypeId: `program_type.autopilot.${action}`,
  provenance: {
    createdByRef: 'omega.blueprint.seed',
    optimizerRunId: null,
    retainedFailureRefs: [],
    sourceModuleVersionId: null,
    trainingDataRefs: ['fixture.first_batch_autopilot_runs'],
  },
  releaseDecision: null,
  releaseState: 'release_candidate',
  rollbackOfModuleVersionId: null,
  scorecards: [],
  status: 'candidate',
  versionRef: `module_version.autopilot.${action}.candidate_1`,
})

const gateForAction = (
  action: AutopilotContinuationAction,
): BlueprintReleaseGate => ({
  decidedByRef: null,
  decision: null,
  decisionReasonRef: null,
  fixturePassState: 'draft',
  fixtureRefs: [`fixture.autopilot.${action}.v1`],
  id: `release_gate.autopilot.${action}.v1`,
  policyState: 'not_checked',
  receiptRefs: [],
  reviewState: 'not_requested',
  rollbackPosture: 'missing',
  scorecardRef: null,
  selfPromotionAttempt: false,
  targetKind: 'program_signature',
  targetRef: `program_signature.autopilot.${action}.v1`,
})

export const AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES =
  AUTOPILOT_CONTINUATION_ACTIONS.map(signatureForAction)

export const AUTOPILOT_CONTINUATION_MODULE_VERSIONS =
  AUTOPILOT_CONTINUATION_ACTIONS.map(moduleForAction)

export const AUTOPILOT_CONTINUATION_RELEASE_GATES =
  AUTOPILOT_CONTINUATION_ACTIONS.map(gateForAction)
