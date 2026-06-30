import { Schema as S } from 'effect'

export const ProbeGepaStandingOptimizationLoopSchemaVersion =
  'omega.probe_gepa_standing_optimization_loop.v1'
export const KhalaFleetDelegationCandidateAdmissionSchemaVersion =
  'omega.probe_gepa_khala_fleet_delegation_candidate_admission.v1'
export const KhalaFleetDelegationCandidateIntentSchemaVersion =
  'openagents.khala.fleet_delegation_candidate_intent.v0'
export const ProbeGepaCandidateManifestSchemaVersion =
  'psionic.probe_gepa_candidate_manifest.v1'
export const KhalaFleetDelegationCandidateSignature = 'khala.fleet.delegation'
export const KhalaFleetDelegationProgramSignatureId =
  'program_signature.khala.fleet.delegation.v1'
export const KhalaFleetDelegationProgramTypeId =
  'program_type.khala.fleet.delegation_policy.v1'

export const ProbeGepaStandingOptimizationLoopRequestedAction = S.Literals([
  'observe',
  'emit_candidates',
  'promote_live',
])
export type ProbeGepaStandingOptimizationLoopRequestedAction =
  typeof ProbeGepaStandingOptimizationLoopRequestedAction.Type

export const ProbeGepaStandingOptimizationLoopDecision = S.Literals([
  'blocked',
  'candidate_artifacts_ready',
  'needs_more_evidence',
])
export type ProbeGepaStandingOptimizationLoopDecision =
  typeof ProbeGepaStandingOptimizationLoopDecision.Type

export const KhalaFleetDelegationCandidateAdmissionDecision = S.Literals([
  'blocked',
  'gated_proposal_ready',
])
export type KhalaFleetDelegationCandidateAdmissionDecision =
  typeof KhalaFleetDelegationCandidateAdmissionDecision.Type

const ProbeBlueprintActionSubmissionProposal = S.Struct({
  actionSubmissionRef: S.String,
  actorRef: S.String,
  approvalPolicyRef: S.String,
  approvalRequired: S.Literal(true),
  assignmentRef: S.optionalKey(S.String),
  contentRedacted: S.Literal(true),
  contextPackRefs: S.Array(S.String),
  directExecution: S.Literal(false),
  directProgramRunExecutionAllowed: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  effectKind: S.Literal('mutate_source_backed_business_fact'),
  inputSnapshotHash: S.String,
  kind: S.Literal('probe_blueprint_action_submission_proposal'),
  modelConfidenceBypassDisabled: S.Literal(true),
  moduleVersionId: S.optionalKey(S.String),
  observedAt: S.String,
  programRunAuthorityBoundary: S.Literal('evidence_only'),
  programRunRef: S.String,
  programSignatureId: S.optionalKey(S.String),
  programTypeId: S.optionalKey(S.String),
  proposalOnly: S.Literal(true),
  receiptRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  status: S.Literal('proposed'),
  summaryRef: S.String,
  toolRefs: S.Array(S.String),
  typedIntent: S.Record(S.String, S.Unknown),
})
export type ProbeBlueprintActionSubmissionProposal =
  typeof ProbeBlueprintActionSubmissionProposal.Type

export class KhalaFleetDelegationCandidateManifestSummary extends S.Class<KhalaFleetDelegationCandidateManifestSummary>(
  'KhalaFleetDelegationCandidateManifestSummary',
)({
  baseModuleRef: S.String,
  candidateManifestRef: S.String,
  candidateRef: S.String,
  evalEvidenceRefs: S.Array(S.String),
  metricName: S.Literal(KhalaFleetDelegationCandidateSignature),
  metricValueBps: S.Number,
  optimizedModuleRef: S.String,
  schemaVersion: S.Literal(ProbeGepaCandidateManifestSchemaVersion),
  signature: S.Literal(KhalaFleetDelegationCandidateSignature),
  traceProvenanceRefs: S.Array(S.String),
}) {}

export class KhalaFleetDelegationBlueprintSelection extends S.Class<KhalaFleetDelegationBlueprintSelection>(
  'KhalaFleetDelegationBlueprintSelection',
)({
  actionSubmissionRequiredForDirectEffects: S.Boolean,
  candidateEntryIds: S.Array(S.String),
  directMutationAllowed: S.Boolean,
  evidenceRequirementRefs: S.Array(S.String),
  lookupId: S.String,
  moduleVersionIds: S.Array(S.String),
  policyRef: S.String,
  programSignatureIds: S.Array(S.String),
  programTypeIds: S.Array(S.String),
  receiptRequirementRefs: S.Array(S.String),
  registryVersionRef: S.String,
  releaseGateRefs: S.Array(S.String),
  safeProjection: S.Boolean,
  toolScopes: S.Array(S.String),
}) {}

export class ProbeGepaStandingOptimizationLoopInput extends S.Class<ProbeGepaStandingOptimizationLoopInput>(
  'ProbeGepaStandingOptimizationLoopInput',
)({
  candidateArtifactRefs: S.Array(S.String),
  candidateManifestRefs: S.Array(S.String),
  dspyRlmAuditRefs: S.Array(S.String),
  effectAuthorityGateRefs: S.Array(S.String),
  evalResultRefs: S.Array(S.String),
  failureFamilyRefs: S.Array(S.String),
  issueRefs: S.Array(S.String),
  loopRef: S.String,
  lowQualityTurnRefs: S.Array(S.String),
  metricCallCount: S.Number,
  mutaliskLaneRefs: S.Array(S.String),
  optimizerRunRefs: S.Array(S.String),
  releaseGateRefs: S.Array(S.String),
  requestedAction: ProbeGepaStandingOptimizationLoopRequestedAction,
  sourceTraceRefs: S.Array(S.String),
}) {}

export class ProbeGepaStandingOptimizationLoopProjection extends S.Class<ProbeGepaStandingOptimizationLoopProjection>(
  'ProbeGepaStandingOptimizationLoopProjection',
)({
  blockerRefs: S.Array(S.String),
  candidateArtifactRefs: S.Array(S.String),
  candidateArtifactsAdmissibleToAuthority: S.Boolean,
  candidateManifestRefs: S.Array(S.String),
  decision: ProbeGepaStandingOptimizationLoopDecision,
  dspyRlmAuditRefs: S.Array(S.String),
  effectAuthorityGateRefs: S.Array(S.String),
  evalResultRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  failureFamilyRefs: S.Array(S.String),
  issueRefs: S.Array(S.String),
  livePromotionAllowed: S.Boolean,
  loopRef: S.String,
  lowQualityTurnRefs: S.Array(S.String),
  metricCallCount: S.Number,
  mutaliskLaneRefs: S.Array(S.String),
  offlineOptimizationReady: S.Boolean,
  optimizerRunRefs: S.Array(S.String),
  releaseGateRefs: S.Array(S.String),
  requestedAction: ProbeGepaStandingOptimizationLoopRequestedAction,
  schemaVersion: S.Literal(ProbeGepaStandingOptimizationLoopSchemaVersion),
}) {}

export class KhalaFleetDelegationCandidateAdmissionInput extends S.Class<KhalaFleetDelegationCandidateAdmissionInput>(
  'KhalaFleetDelegationCandidateAdmissionInput',
)({
  actionSubmissionRef: S.optionalKey(S.String),
  actorRef: S.String,
  approvalPolicyRef: S.String,
  assignmentRef: S.optionalKey(S.String),
  blueprintSelection: KhalaFleetDelegationBlueprintSelection,
  candidate: KhalaFleetDelegationCandidateManifestSummary,
  contextPackRefs: S.Array(S.String),
  observedAt: S.String,
  programRunRef: S.String,
  standingLoop: ProbeGepaStandingOptimizationLoopInput,
  summaryRef: S.String,
}) {}

export class KhalaFleetDelegationCandidateAdmissionProjection extends S.Class<KhalaFleetDelegationCandidateAdmissionProjection>(
  'KhalaFleetDelegationCandidateAdmissionProjection',
)({
  actionSubmissionProposal: S.NullOr(ProbeBlueprintActionSubmissionProposal),
  actionSubmissionProposalRefs: S.Array(S.String),
  autoPromotionPathExists: S.Literal(false),
  blockerRefs: S.Array(S.String),
  blueprintLookupId: S.String,
  candidateManifestRef: S.String,
  candidateRef: S.String,
  candidateSignature: S.Literal(KhalaFleetDelegationCandidateSignature),
  decision: KhalaFleetDelegationCandidateAdmissionDecision,
  directExecutionAllowed: S.Literal(false),
  livePromotionAllowed: S.Literal(false),
  proposalRequired: S.Literal(true),
  runtimePromotionAllowed: S.Literal(false),
  schemaVersion: S.Literal(
    KhalaFleetDelegationCandidateAdmissionSchemaVersion,
  ),
  standingLoop: ProbeGepaStandingOptimizationLoopProjection,
}) {}

export class ProbeGepaStandingOptimizationLoopUnsafe extends S.TaggedErrorClass<ProbeGepaStandingOptimizationLoopUnsafe>()(
  'ProbeGepaStandingOptimizationLoopUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|fixture[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(auth|benchmark|email|fixture|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|trace|traces)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ProbeGepaStandingOptimizationLoopUnsafe({
      reason: `${label} contains raw traces, prompts, fixtures, provider credentials, account refs, wallet/payment material, private repo paths, local filesystem paths, or raw timestamps.`,
    })
  }
}

const collectStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }
  if (Array.isArray(value)) {
    return value.flatMap(item => collectStringValues(item))
  }
  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(item => collectStringValues(item))
  }
  return []
}

const publicRefSegment = (value: string, fallback: string): string => {
  const segment = value
    .trim()
    .replace(/[^A-Za-z0-9_.-]/g, '_')
    .replace(/_+/g, '_')
    .slice(0, 80)

  return segment === '' ? fallback : segment
}

const normalizeKhalaFleetDelegationCandidate = (
  candidate: KhalaFleetDelegationCandidateManifestSummary,
): KhalaFleetDelegationCandidateManifestSummary =>
  new KhalaFleetDelegationCandidateManifestSummary({
    ...candidate,
    evalEvidenceRefs: uniqueRefs(candidate.evalEvidenceRefs),
    traceProvenanceRefs: uniqueRefs(candidate.traceProvenanceRefs),
  })

const normalizeKhalaFleetDelegationBlueprintSelection = (
  selection: KhalaFleetDelegationBlueprintSelection,
): KhalaFleetDelegationBlueprintSelection =>
  new KhalaFleetDelegationBlueprintSelection({
    ...selection,
    candidateEntryIds: uniqueRefs(selection.candidateEntryIds),
    evidenceRequirementRefs: uniqueRefs(selection.evidenceRequirementRefs),
    moduleVersionIds: uniqueRefs(selection.moduleVersionIds),
    programSignatureIds: uniqueRefs(selection.programSignatureIds),
    programTypeIds: uniqueRefs(selection.programTypeIds),
    receiptRequirementRefs: uniqueRefs(selection.receiptRequirementRefs),
    releaseGateRefs: uniqueRefs(selection.releaseGateRefs),
    toolScopes: uniqueRefs(selection.toolScopes),
  })

const normalizeInput = (
  input: ProbeGepaStandingOptimizationLoopInput,
): ProbeGepaStandingOptimizationLoopInput =>
  new ProbeGepaStandingOptimizationLoopInput({
    ...input,
    candidateArtifactRefs: uniqueRefs(input.candidateArtifactRefs),
    candidateManifestRefs: uniqueRefs(input.candidateManifestRefs),
    dspyRlmAuditRefs: uniqueRefs(input.dspyRlmAuditRefs),
    effectAuthorityGateRefs: uniqueRefs(input.effectAuthorityGateRefs),
    evalResultRefs: uniqueRefs(input.evalResultRefs),
    failureFamilyRefs: uniqueRefs(input.failureFamilyRefs),
    issueRefs: uniqueRefs(input.issueRefs),
    lowQualityTurnRefs: uniqueRefs(input.lowQualityTurnRefs),
    mutaliskLaneRefs: uniqueRefs(input.mutaliskLaneRefs),
    optimizerRunRefs: uniqueRefs(input.optimizerRunRefs),
    releaseGateRefs: uniqueRefs(input.releaseGateRefs),
    sourceTraceRefs: uniqueRefs(input.sourceTraceRefs),
  })

const blockerRefsFor = (
  input: ProbeGepaStandingOptimizationLoopInput,
): ReadonlyArray<string> => {
  const hasLowQualitySelection =
    input.lowQualityTurnRefs.length > 0 || input.failureFamilyRefs.length > 0
  const candidateRequested = input.requestedAction === 'emit_candidates'
  const candidateRefsExist =
    input.candidateArtifactRefs.length > 0 ||
    input.candidateManifestRefs.length > 0

  return uniqueRefs([
    ...(input.sourceTraceRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.source_trace_refs_missing']
      : []),
    ...(input.evalResultRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.eval_result_refs_missing']
      : []),
    ...(!hasLowQualitySelection
      ? ['blocker.probe_gepa_standing_loop.low_quality_selection_missing']
      : []),
    ...(input.metricCallCount <= 0
      ? ['blocker.probe_gepa_standing_loop.metric_calls_missing']
      : []),
    ...(candidateRequested && input.mutaliskLaneRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.mutalisk_lane_missing']
      : []),
    ...(candidateRequested && input.dspyRlmAuditRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.dspy_rlm_audit_missing']
      : []),
    ...(candidateRequested && input.optimizerRunRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.optimizer_run_refs_missing']
      : []),
    ...(candidateRequested && input.candidateArtifactRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.candidate_artifacts_missing']
      : []),
    ...(candidateRequested && input.candidateManifestRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.candidate_manifests_missing']
      : []),
    ...(candidateRefsExist && input.effectAuthorityGateRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.effect_authority_gate_missing']
      : []),
    ...(candidateRefsExist && input.releaseGateRefs.length === 0
      ? ['blocker.probe_gepa_standing_loop.release_gate_missing']
      : []),
    ...(input.requestedAction === 'promote_live'
      ? ['blocker.probe_gepa_standing_loop.live_promotion_not_allowed']
      : []),
  ])
}

export const projectProbeGepaStandingOptimizationLoop = (
  input: ProbeGepaStandingOptimizationLoopInput,
): ProbeGepaStandingOptimizationLoopProjection => {
  const normalized = normalizeInput(
    S.decodeUnknownSync(ProbeGepaStandingOptimizationLoopInput)(input),
  )

  assertSafeRefs('Probe GEPA standing loop identity refs', [
    normalized.loopRef,
    ...normalized.issueRefs,
  ])
  assertSafeRefs(
    'Probe GEPA standing loop source trace refs',
    normalized.sourceTraceRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop eval result refs',
    normalized.evalResultRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop low-quality turn refs',
    normalized.lowQualityTurnRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop failure family refs',
    normalized.failureFamilyRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop Mutalisk lane refs',
    normalized.mutaliskLaneRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop optimizer run refs',
    normalized.optimizerRunRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop candidate artifact refs',
    normalized.candidateArtifactRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop candidate manifest refs',
    normalized.candidateManifestRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop DSPy/RLM audit refs',
    normalized.dspyRlmAuditRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop Effect authority gate refs',
    normalized.effectAuthorityGateRefs,
  )
  assertSafeRefs(
    'Probe GEPA standing loop release gate refs',
    normalized.releaseGateRefs,
  )

  if (!Number.isInteger(normalized.metricCallCount)) {
    throw new ProbeGepaStandingOptimizationLoopUnsafe({
      reason: 'Metric call count must be an integer.',
    })
  }

  const blockerRefs = blockerRefsFor(normalized)
  const offlineOptimizationReady =
    blockerRefs.length === 0 &&
    normalized.requestedAction === 'emit_candidates'
  const candidateArtifactsAdmissibleToAuthority =
    offlineOptimizationReady &&
    normalized.effectAuthorityGateRefs.length > 0 &&
    normalized.releaseGateRefs.length > 0
  const evidenceRefs = uniqueRefs([
    ...normalized.sourceTraceRefs,
    ...normalized.evalResultRefs,
    ...normalized.dspyRlmAuditRefs,
    ...normalized.optimizerRunRefs,
  ])
  const decision: ProbeGepaStandingOptimizationLoopDecision =
    blockerRefs.length > 0
      ? 'blocked'
      : normalized.requestedAction === 'emit_candidates'
        ? 'candidate_artifacts_ready'
        : 'needs_more_evidence'

  return new ProbeGepaStandingOptimizationLoopProjection({
    blockerRefs,
    candidateArtifactRefs: normalized.candidateArtifactRefs,
    candidateArtifactsAdmissibleToAuthority,
    candidateManifestRefs: normalized.candidateManifestRefs,
    decision,
    dspyRlmAuditRefs: normalized.dspyRlmAuditRefs,
    effectAuthorityGateRefs: normalized.effectAuthorityGateRefs,
    evalResultRefs: normalized.evalResultRefs,
    evidenceRefs,
    failureFamilyRefs: normalized.failureFamilyRefs,
    issueRefs: normalized.issueRefs,
    livePromotionAllowed: false,
    loopRef: normalized.loopRef,
    lowQualityTurnRefs: normalized.lowQualityTurnRefs,
    metricCallCount: normalized.metricCallCount,
    mutaliskLaneRefs: normalized.mutaliskLaneRefs,
    offlineOptimizationReady,
    optimizerRunRefs: normalized.optimizerRunRefs,
    releaseGateRefs: normalized.releaseGateRefs,
    requestedAction: normalized.requestedAction,
    schemaVersion: ProbeGepaStandingOptimizationLoopSchemaVersion,
  })
}

const blockerRefsForKhalaFleetDelegationAdmission = (
  standingLoop: ProbeGepaStandingOptimizationLoopProjection,
  candidate: KhalaFleetDelegationCandidateManifestSummary,
  blueprintSelection: KhalaFleetDelegationBlueprintSelection,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...standingLoop.blockerRefs,
    ...(!standingLoop.candidateArtifactsAdmissibleToAuthority ||
    standingLoop.decision !== 'candidate_artifacts_ready'
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_not_admissible',
        ]
      : []),
    ...(candidate.schemaVersion !== ProbeGepaCandidateManifestSchemaVersion
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_manifest_schema_mismatch',
        ]
      : []),
    ...(candidate.signature !== KhalaFleetDelegationCandidateSignature
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_signature_mismatch',
        ]
      : []),
    ...(candidate.metricName !== KhalaFleetDelegationCandidateSignature
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_metric_mismatch',
        ]
      : []),
    ...(!Number.isFinite(candidate.metricValueBps) ||
    !Number.isInteger(candidate.metricValueBps) ||
    candidate.metricValueBps < 0 ||
    candidate.metricValueBps > 10_000
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_metric_invalid',
        ]
      : []),
    ...(candidate.evalEvidenceRefs.length === 0
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_eval_evidence_missing',
        ]
      : []),
    ...(candidate.traceProvenanceRefs.length === 0
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_trace_provenance_missing',
        ]
      : []),
    ...(!standingLoop.candidateManifestRefs.includes(
      candidate.candidateManifestRef,
    )
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_candidate_manifest_not_in_standing_loop',
        ]
      : []),
    ...(!blueprintSelection.actionSubmissionRequiredForDirectEffects
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_action_submission_boundary_missing',
        ]
      : []),
    ...(blueprintSelection.directMutationAllowed
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_direct_mutation_not_allowed',
        ]
      : []),
    ...(!blueprintSelection.safeProjection
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_unsafe_projection',
        ]
      : []),
    ...(blueprintSelection.candidateEntryIds.length === 0
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_entry_missing',
        ]
      : []),
    ...(blueprintSelection.evidenceRequirementRefs.length === 0
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_evidence_missing',
        ]
      : []),
    ...(blueprintSelection.moduleVersionIds.length === 0
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_module_version_missing',
        ]
      : []),
    ...(!blueprintSelection.programSignatureIds.includes(
      KhalaFleetDelegationProgramSignatureId,
    )
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_missing',
        ]
      : []),
    ...(!blueprintSelection.programTypeIds.includes(
      KhalaFleetDelegationProgramTypeId,
    )
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_program_type_missing',
        ]
      : []),
    ...(blueprintSelection.releaseGateRefs.length === 0
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_release_gate_missing',
        ]
      : []),
    ...(blueprintSelection.toolScopes.length === 0
      ? [
          'blocker.probe_gepa_standing_loop.khala_delegation_signature_lookup_tool_scope_missing',
        ]
      : []),
  ])

export const projectKhalaFleetDelegationCandidateAdmission = (
  input: KhalaFleetDelegationCandidateAdmissionInput,
): KhalaFleetDelegationCandidateAdmissionProjection => {
  const decoded = S.decodeUnknownSync(KhalaFleetDelegationCandidateAdmissionInput)(
    input,
  )
  const candidate = normalizeKhalaFleetDelegationCandidate(decoded.candidate)
  const blueprintSelection = normalizeKhalaFleetDelegationBlueprintSelection(
    decoded.blueprintSelection,
  )
  const contextPackRefs = uniqueRefs(decoded.contextPackRefs)
  const standingLoop = projectProbeGepaStandingOptimizationLoop(
    decoded.standingLoop,
  )

  assertSafeRefs('Khala fleet delegation admission refs', [
    decoded.actionSubmissionRef ?? 'action_submission.khala_delegation.pending',
    decoded.actorRef,
    decoded.approvalPolicyRef,
    decoded.assignmentRef ?? 'assignment.khala_delegation.unassigned',
    decoded.programRunRef,
    decoded.summaryRef,
    ...contextPackRefs,
  ])
  assertSafeRefs('Khala fleet delegation candidate refs', [
    candidate.baseModuleRef,
    candidate.candidateManifestRef,
    candidate.candidateRef,
    candidate.metricName,
    candidate.optimizedModuleRef,
    candidate.schemaVersion,
    candidate.signature,
    ...candidate.evalEvidenceRefs,
    ...candidate.traceProvenanceRefs,
  ])
  assertSafeRefs('Khala fleet delegation Blueprint selection refs', [
    blueprintSelection.lookupId,
    blueprintSelection.policyRef,
    blueprintSelection.registryVersionRef,
    ...blueprintSelection.candidateEntryIds,
    ...blueprintSelection.evidenceRequirementRefs,
    ...blueprintSelection.moduleVersionIds,
    ...blueprintSelection.programSignatureIds,
    ...blueprintSelection.programTypeIds,
    ...blueprintSelection.receiptRequirementRefs,
    ...blueprintSelection.releaseGateRefs,
    ...blueprintSelection.toolScopes,
  ])

  const blockerRefs = blockerRefsForKhalaFleetDelegationAdmission(
    standingLoop,
    candidate,
    blueprintSelection,
  )
  const decision: KhalaFleetDelegationCandidateAdmissionDecision =
    blockerRefs.length === 0 ? 'gated_proposal_ready' : 'blocked'

  if (decision === 'blocked') {
    return new KhalaFleetDelegationCandidateAdmissionProjection({
      actionSubmissionProposal: null,
      actionSubmissionProposalRefs: [],
      autoPromotionPathExists: false,
      blockerRefs,
      blueprintLookupId: blueprintSelection.lookupId,
      candidateManifestRef: candidate.candidateManifestRef,
      candidateRef: candidate.candidateRef,
      candidateSignature: KhalaFleetDelegationCandidateSignature,
      decision,
      directExecutionAllowed: false,
      livePromotionAllowed: false,
      proposalRequired: true,
      runtimePromotionAllowed: false,
      schemaVersion: KhalaFleetDelegationCandidateAdmissionSchemaVersion,
      standingLoop,
    })
  }

  const candidateSegment = publicRefSegment(candidate.candidateRef, 'candidate')
  const actionSubmissionRef =
    decoded.actionSubmissionRef ??
    `action_submission.khala_fleet_delegation.${candidateSegment}`
  const typedIntent = {
    actionSubmissionRequiredForDirectEffects: true,
    approvalRequired: true,
    baseModuleRef: candidate.baseModuleRef,
    blueprintLookupId: blueprintSelection.lookupId,
    candidateManifestRef: candidate.candidateManifestRef,
    candidateRef: candidate.candidateRef,
    directExecutionAllowed: false,
    evalEvidenceRefs: candidate.evalEvidenceRefs,
    evidenceRequirementRefs: blueprintSelection.evidenceRequirementRefs,
    metricName: candidate.metricName,
    metricValueBps: candidate.metricValueBps,
    optimizedModuleRef: candidate.optimizedModuleRef,
    releaseGateRefs: blueprintSelection.releaseGateRefs,
    schemaVersion: KhalaFleetDelegationCandidateIntentSchemaVersion,
    signature: KhalaFleetDelegationCandidateSignature,
    traceProvenanceRefs: candidate.traceProvenanceRefs,
  }
  const proposal = S.decodeUnknownSync(ProbeBlueprintActionSubmissionProposal)({
    actionSubmissionRef,
    actorRef: decoded.actorRef,
    approvalPolicyRef: decoded.approvalPolicyRef,
    approvalRequired: true,
    ...(decoded.assignmentRef !== undefined
      ? { assignmentRef: decoded.assignmentRef }
      : {}),
    contentRedacted: true,
    contextPackRefs,
    directExecution: false,
    directProgramRunExecutionAllowed: false,
    evidenceRefs: uniqueRefs([
      ...standingLoop.evidenceRefs,
      ...candidate.evalEvidenceRefs,
      ...candidate.traceProvenanceRefs,
      ...blueprintSelection.evidenceRequirementRefs,
    ]),
    effectKind: 'mutate_source_backed_business_fact',
    inputSnapshotHash: `sha256:khala_fleet_delegation.${candidateSegment}.${candidate.metricValueBps}`,
    kind: 'probe_blueprint_action_submission_proposal',
    modelConfidenceBypassDisabled: true,
    moduleVersionId: blueprintSelection.moduleVersionIds[0],
    observedAt: decoded.observedAt,
    programRunAuthorityBoundary: 'evidence_only',
    programRunRef: decoded.programRunRef,
    programSignatureId: KhalaFleetDelegationProgramSignatureId,
    programTypeId: KhalaFleetDelegationProgramTypeId,
    proposalOnly: true,
    receiptRefs: uniqueRefs([
      'receipt.action_submission.khala_fleet_delegation.operator_review',
      ...blueprintSelection.receiptRequirementRefs,
    ]),
    sourceAuthorityRefs: uniqueRefs([
      blueprintSelection.policyRef,
      blueprintSelection.registryVersionRef,
      ...blueprintSelection.releaseGateRefs,
      ...standingLoop.effectAuthorityGateRefs,
      ...standingLoop.releaseGateRefs,
    ]),
    status: 'proposed',
    summaryRef: decoded.summaryRef,
    toolRefs: blueprintSelection.toolScopes,
    typedIntent,
  })

  const { observedAt: _observedAt, ...proposalPublicRefEnvelope } = proposal
  assertSafeRefs(
    'Khala fleet delegation Action Submission proposal refs',
    collectStringValues(proposalPublicRefEnvelope),
  )

  return new KhalaFleetDelegationCandidateAdmissionProjection({
    actionSubmissionProposal: proposal,
    actionSubmissionProposalRefs: [proposal.actionSubmissionRef],
    autoPromotionPathExists: false,
    blockerRefs,
    blueprintLookupId: blueprintSelection.lookupId,
    candidateManifestRef: candidate.candidateManifestRef,
    candidateRef: candidate.candidateRef,
    candidateSignature: KhalaFleetDelegationCandidateSignature,
    decision,
    directExecutionAllowed: false,
    livePromotionAllowed: false,
    proposalRequired: true,
    runtimePromotionAllowed: false,
    schemaVersion: KhalaFleetDelegationCandidateAdmissionSchemaVersion,
    standingLoop,
  })
}
