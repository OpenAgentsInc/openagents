import { Schema as S } from 'effect'

export const ProbeGepaStandingOptimizationLoopSchemaVersion =
  'omega.probe_gepa_standing_optimization_loop.v1'

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
  const hasTraceOrEvalEvidence =
    input.sourceTraceRefs.length > 0 || input.evalResultRefs.length > 0
  const hasLowQualitySelection =
    input.lowQualityTurnRefs.length > 0 || input.failureFamilyRefs.length > 0
  const candidateRequested = input.requestedAction === 'emit_candidates'
  const candidateRefsExist =
    input.candidateArtifactRefs.length > 0 ||
    input.candidateManifestRefs.length > 0

  return uniqueRefs([
    ...(!hasTraceOrEvalEvidence
      ? ['blocker.probe_gepa_standing_loop.trace_or_eval_refs_missing']
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
