import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniModelLabAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniModelLabAudience = typeof OmniModelLabAudience.Type

export const OmniModelLabLoopState = S.Literals([
  'adapter_validated',
  'archived',
  'attributed',
  'blocked',
  'candidate_created',
  'eval_rerun',
  'gate_passed',
  'retained',
])
export type OmniModelLabLoopState = typeof OmniModelLabLoopState.Type

export const OmniModelLabFailureKind = S.Literals([
  'adapter_mismatch',
  'eval_regression',
  'generation',
  'provider_timeout',
  'route_selection',
  'safety_redaction',
  'tool_use',
  'validation',
])
export type OmniModelLabFailureKind = typeof OmniModelLabFailureKind.Type

export const OmniModelLabFailureState = S.Literals([
  'candidate_created',
  'retained',
  'triaged',
])
export type OmniModelLabFailureState = typeof OmniModelLabFailureState.Type

export const OmniModelLabCandidateKind = S.Literals([
  'eval_fixture',
  'model_adapter',
  'module_version',
  'program_signature',
  'prompt_policy',
])
export type OmniModelLabCandidateKind =
  typeof OmniModelLabCandidateKind.Type

export const OmniModelLabCandidateState = S.Literals([
  'draft',
  'eval_failed',
  'eval_passed',
  'eval_running',
  'needs_review',
  'proposed',
  'rejected',
  'retained',
])
export type OmniModelLabCandidateState =
  typeof OmniModelLabCandidateState.Type

export const OmniModelLabEvalState = S.Literals([
  'failed',
  'flaky',
  'not_run',
  'passed',
  'running',
])
export type OmniModelLabEvalState = typeof OmniModelLabEvalState.Type

export const OmniModelLabAdapterValidationState = S.Literals([
  'failed',
  'not_required',
  'passed',
  'pending',
])
export type OmniModelLabAdapterValidationState =
  typeof OmniModelLabAdapterValidationState.Type

export const OmniModelLabPromotionGateState = S.Literals([
  'blocked',
  'draft',
  'failed',
  'passed',
  'pending_review',
])
export type OmniModelLabPromotionGateState =
  typeof OmniModelLabPromotionGateState.Type

export const OmniModelLabRollbackPosture = S.Literals([
  'missing',
  'ready',
  'verified',
])
export type OmniModelLabRollbackPosture =
  typeof OmniModelLabRollbackPosture.Type

export const OmniModelLabAttributionState = S.Literals([
  'candidate',
  'disputed',
  'none',
  'recorded',
])
export type OmniModelLabAttributionState =
  typeof OmniModelLabAttributionState.Type

export const OmniModelLabAuthorityBoundary = S.Literals([
  'read_only_model_lab_retained_failure_loop',
])
export type OmniModelLabAuthorityBoundary =
  typeof OmniModelLabAuthorityBoundary.Type

export class OmniModelLabAuthority extends S.Class<OmniModelLabAuthority>(
  'OmniModelLabAuthority',
)({
  authorityBoundary: OmniModelLabAuthorityBoundary,
  noAdapterInstallation: S.Boolean,
  noEvalExecution: S.Boolean,
  noModelTrainingMutation: S.Boolean,
  noPayoutMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRoutingMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OmniModelLabRetainedFailureRecord extends S.Class<OmniModelLabRetainedFailureRecord>(
  'OmniModelLabRetainedFailureRecord',
)({
  evidenceRefs: S.Array(S.String),
  failureRef: S.String,
  kind: OmniModelLabFailureKind,
  redactionPolicyRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniModelLabFailureState,
  traceRefs: S.Array(S.String),
  workroomRefs: S.Array(S.String),
}) {}

export class OmniModelLabCandidateRecord extends S.Class<OmniModelLabCandidateRecord>(
  'OmniModelLabCandidateRecord',
)({
  candidateRef: S.String,
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  kind: OmniModelLabCandidateKind,
  modelRefs: S.Array(S.String),
  moduleVersionRefs: S.Array(S.String),
  signatureRefs: S.Array(S.String),
  sourceFailureRefs: S.Array(S.String),
  state: OmniModelLabCandidateState,
}) {}

export class OmniModelLabEvalRerunRecord extends S.Class<OmniModelLabEvalRerunRecord>(
  'OmniModelLabEvalRerunRecord',
)({
  candidateRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  evalRef: S.String,
  fixtureRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  scorecardRefs: S.Array(S.String),
  sourceFailureRefs: S.Array(S.String),
  state: OmniModelLabEvalState,
}) {}

export class OmniModelLabAdapterValidationRecord extends S.Class<OmniModelLabAdapterValidationRecord>(
  'OmniModelLabAdapterValidationRecord',
)({
  adapterRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  datasetRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  state: OmniModelLabAdapterValidationState,
  validationRef: S.String,
}) {}

export class OmniModelLabPromotionGateRecord extends S.Class<OmniModelLabPromotionGateRecord>(
  'OmniModelLabPromotionGateRecord',
)({
  adapterValidationRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  evalRefs: S.Array(S.String),
  gateRef: S.String,
  policyRefs: S.Array(S.String),
  reviewReceiptRefs: S.Array(S.String),
  rollbackPosture: OmniModelLabRollbackPosture,
  rollbackRefs: S.Array(S.String),
  selfPromotionAttempt: S.Boolean,
  state: OmniModelLabPromotionGateState,
  targetRef: S.String,
}) {}

export class OmniModelLabAttributionRecord extends S.Class<OmniModelLabAttributionRecord>(
  'OmniModelLabAttributionRecord',
)({
  acceptedOutcomeRefs: S.Array(S.String),
  attributionRef: S.String,
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  contributorRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  state: OmniModelLabAttributionState,
}) {}

export class OmniModelLabRetainedFailureLoopRecord extends S.Class<OmniModelLabRetainedFailureLoopRecord>(
  'OmniModelLabRetainedFailureLoopRecord',
)({
  adapterValidations: S.Array(OmniModelLabAdapterValidationRecord),
  attributions: S.Array(OmniModelLabAttributionRecord),
  authority: OmniModelLabAuthority,
  blockerRefs: S.Array(S.String),
  candidateRecords: S.Array(OmniModelLabCandidateRecord),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  evalReruns: S.Array(OmniModelLabEvalRerunRecord),
  id: S.String,
  loopRef: S.String,
  promotionGates: S.Array(OmniModelLabPromotionGateRecord),
  retainedFailures: S.Array(OmniModelLabRetainedFailureRecord),
  sourceRefs: S.Array(S.String),
  state: OmniModelLabLoopState,
  updatedAtIso: S.String,
}) {}

export class OmniModelLabRetainedFailureLoopProjection extends S.Class<OmniModelLabRetainedFailureLoopProjection>(
  'OmniModelLabRetainedFailureLoopProjection',
)({
  adapterInstallationAllowed: S.Boolean,
  adapterValidationPassedCount: S.Number,
  adapterValidations: S.Array(OmniModelLabAdapterValidationRecord),
  attributions: S.Array(OmniModelLabAttributionRecord),
  audience: OmniModelLabAudience,
  authority: OmniModelLabAuthority,
  blockerRefs: S.Array(S.String),
  candidateCount: S.Number,
  candidateRecords: S.Array(OmniModelLabCandidateRecord),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evalExecutionAllowed: S.Boolean,
  evalPassedCount: S.Number,
  evalReruns: S.Array(OmniModelLabEvalRerunRecord),
  id: S.String,
  loopRef: S.String,
  modelTrainingMutationAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  promotionGatePassedCount: S.Number,
  promotionGates: S.Array(OmniModelLabPromotionGateRecord),
  publicClaimUpgradeAllowed: S.Boolean,
  recordedAttributionCount: S.Number,
  retainedFailureCount: S.Number,
  retainedFailures: S.Array(OmniModelLabRetainedFailureRecord),
  rollbackPosture: OmniModelLabRollbackPosture,
  routingMutationAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  selfPromotionAttemptDetected: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: OmniModelLabLoopState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
}) {}

export class OmniModelLabRetainedFailureLoopUnsafe extends S.TaggedErrorClass<OmniModelLabRetainedFailureLoopUnsafe>()(
  'OmniModelLabRetainedFailureLoopUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_MODEL_LAB_READ_ONLY_AUTHORITY: OmniModelLabAuthority = {
  authorityBoundary: 'read_only_model_lab_retained_failure_loop',
  noAdapterInstallation: true,
  noEvalExecution: true,
  noModelTrainingMutation: true,
  noPayoutMutation: true,
  noPublicClaimUpgrade: true,
  noRoutingMutation: true,
  noRuntimePromotion: true,
  noSettlementMutation: true,
}

const stateRank: Readonly<Record<OmniModelLabLoopState, number>> = {
  archived: -1,
  blocked: -1,
  retained: 0,
  candidate_created: 1,
  eval_rerun: 2,
  adapter_validated: 3,
  gate_passed: 4,
  attributed: 5,
}

const stateLabelByState: Readonly<Record<OmniModelLabLoopState, string>> = {
  adapter_validated: 'Adapter validated',
  archived: 'Archived',
  attributed: 'Attributed',
  blocked: 'Blocked',
  candidate_created: 'Candidate created',
  eval_rerun: 'Eval rerun',
  gate_passed: 'Gate passed',
  retained: 'Retained',
}

const publicUnsafeRefPattern =
  /(adapter\.private|attribution\.private|candidate\.private|caveat\.private|dataset\.private|eval\.private|evidence\.private|failure\.private|gate\.private|model\.private|policy\.private|provider\.|receipt\.private|review\.private|rollback\.private|scorecard\.private|signature\.private|source\.|target\.private|trace\.private|workroom\.private)/i
const agentUnsafeRefPattern =
  /(adapter\.private|candidate\.private|dataset\.private|eval\.private|failure\.private|gate\.private|provider\.private|receipt\.private|review\.private|rollback\.private|source\.private|trace\.private)/i
const customerUnsafeRefPattern =
  /(adapter\.private|candidate\.private|dataset\.private|eval\.private|failure\.private|gate\.private|provider\.private|receipt\.private|review\.private|rollback\.private|source\.private|trace\.private)/i

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeModelLabRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|contact[_-]?(email|name|phone)|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.(raw|private)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(adapter|auth|connector|customer|dataset|email|failure|invoice|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const loopAtLeast = (
  state: OmniModelLabLoopState,
  threshold: OmniModelLabLoopState,
): boolean => stateRank[state] >= stateRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeModelLabRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: `${label} contains private prompts, source archives, provider payloads, customer data, secrets, payment/wallet material, private repos, raw logs, raw traces, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniModelLabAudience,
): RegExp | null => {
  switch (audience) {
    case 'agent':
      return agentUnsafeRefPattern
    case 'customer':
      return customerUnsafeRefPattern
    case 'public':
      return publicUnsafeRefPattern
    case 'operator':
    case 'team':
      return null
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniModelLabAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniModelLabAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (authority: OmniModelLabAuthority): void => {
  if (
    authority.noAdapterInstallation !== true ||
    authority.noEvalExecution !== true ||
    authority.noModelTrainingMutation !== true ||
    authority.noPayoutMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRoutingMutation !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Model Lab retained-failure loops are read-only and cannot run evals, train models, install adapters, promote runtime behavior, mutate routes, pay out, settle, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const hasAny = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertRetainedFailure = (
  failure: OmniModelLabRetainedFailureRecord,
): void => {
  assertSafeRefs('Model Lab retained failure refs', [
    failure.failureRef,
    ...failure.evidenceRefs,
    ...failure.redactionPolicyRefs,
    ...failure.sourceRefs,
    ...failure.traceRefs,
    ...failure.workroomRefs,
  ])

  if (
    !hasAny(failure.evidenceRefs) ||
    !hasAny(failure.redactionPolicyRefs) ||
    !hasAny(failure.sourceRefs)
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Retained failures require evidence, redaction policy, and source refs.',
    })
  }
}

const assertCandidate = (
  candidate: OmniModelLabCandidateRecord,
  failureRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Model Lab candidate refs', [
    candidate.candidateRef,
    ...candidate.caveatRefs,
    ...candidate.evidenceRefs,
    ...candidate.modelRefs,
    ...candidate.moduleVersionRefs,
    ...candidate.signatureRefs,
    ...candidate.sourceFailureRefs,
  ])

  const missingFailure = candidate.sourceFailureRefs.find(
    ref => !failureRefs.has(ref),
  )

  if (missingFailure !== undefined) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Model Lab candidates must link to retained failures in the same loop.',
    })
  }

  if (
    ['proposed', 'eval_running', 'eval_passed', 'needs_review', 'retained']
      .includes(candidate.state) &&
    (!hasAny(candidate.sourceFailureRefs) || !hasAny(candidate.evidenceRefs))
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Proposed or reviewed Model Lab candidates require source failure and evidence refs.',
    })
  }
}

const assertEval = (
  evalRerun: OmniModelLabEvalRerunRecord,
  failureRefs: ReadonlySet<string>,
  candidateRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Model Lab eval rerun refs', [
    evalRerun.evalRef,
    ...evalRerun.candidateRefs,
    ...evalRerun.evidenceRefs,
    ...evalRerun.fixtureRefs,
    ...evalRerun.receiptRefs,
    ...evalRerun.scorecardRefs,
    ...evalRerun.sourceFailureRefs,
  ])

  if (evalRerun.sourceFailureRefs.some(ref => !failureRefs.has(ref))) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Eval reruns must link to retained failures in the same loop.',
    })
  }

  if (evalRerun.candidateRefs.some(ref => !candidateRefs.has(ref))) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Eval reruns must link to candidates in the same loop.',
    })
  }

  if (
    evalRerun.state === 'passed' &&
    (!hasAny(evalRerun.receiptRefs) ||
      !hasAny(evalRerun.evidenceRefs) ||
      !hasAny(evalRerun.fixtureRefs) ||
      !hasAny(evalRerun.scorecardRefs))
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Passed eval reruns require receipt, evidence, fixture, and scorecard refs.',
    })
  }
}

const assertAdapterValidation = (
  validation: OmniModelLabAdapterValidationRecord,
  candidateRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Model Lab adapter validation refs', [
    validation.validationRef,
    ...validation.adapterRefs,
    ...validation.candidateRefs,
    ...validation.datasetRefs,
    ...validation.evidenceRefs,
    ...validation.providerRefs,
    ...validation.receiptRefs,
  ])

  if (validation.candidateRefs.some(ref => !candidateRefs.has(ref))) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Adapter validations must link to candidates in the same loop.',
    })
  }

  if (
    validation.state === 'passed' &&
    (!hasAny(validation.adapterRefs) ||
      !hasAny(validation.evidenceRefs) ||
      !hasAny(validation.providerRefs) ||
      !hasAny(validation.receiptRefs))
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Passed adapter validations require adapter, provider, evidence, and receipt refs.',
    })
  }
}

const assertPromotionGate = (
  gate: OmniModelLabPromotionGateRecord,
  candidateRefs: ReadonlySet<string>,
  passedEvalRefs: ReadonlySet<string>,
  passedAdapterRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Model Lab promotion gate refs', [
    gate.gateRef,
    gate.targetRef,
    ...gate.adapterValidationRefs,
    ...gate.candidateRefs,
    ...gate.evidenceRefs,
    ...gate.evalRefs,
    ...gate.policyRefs,
    ...gate.reviewReceiptRefs,
    ...gate.rollbackRefs,
  ])

  if (gate.candidateRefs.some(ref => !candidateRefs.has(ref))) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Promotion gates must link to candidates in the same loop.',
    })
  }

  if (
    gate.state === 'passed' &&
    (gate.selfPromotionAttempt ||
      !hasAny(gate.reviewReceiptRefs) ||
      !hasAny(gate.policyRefs) ||
      !hasAny(gate.evidenceRefs) ||
      !['ready', 'verified'].includes(gate.rollbackPosture) ||
      !hasAny(gate.rollbackRefs) ||
      gate.evalRefs.some(ref => !passedEvalRefs.has(ref)) ||
      gate.adapterValidationRefs.some(ref => !passedAdapterRefs.has(ref)))
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Passed promotion gates require passed evals, passed adapter validations, review receipts, policy/evidence refs, rollback posture, rollback refs, and no self-promotion attempt.',
    })
  }
}

const assertAttribution = (
  attribution: OmniModelLabAttributionRecord,
  candidateRefs: ReadonlySet<string>,
  passedGateCount: number,
): void => {
  assertSafeRefs('Model Lab attribution refs', [
    attribution.attributionRef,
    ...attribution.acceptedOutcomeRefs,
    ...attribution.candidateRefs,
    ...attribution.caveatRefs,
    ...attribution.contributorRefs,
    ...attribution.receiptRefs,
  ])

  if (attribution.candidateRefs.some(ref => !candidateRefs.has(ref))) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Model Lab attributions must link to candidates in the same loop.',
    })
  }

  if (
    attribution.state === 'recorded' &&
    (passedGateCount === 0 ||
      !hasAny(attribution.acceptedOutcomeRefs) ||
      !hasAny(attribution.contributorRefs) ||
      !hasAny(attribution.receiptRefs))
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Recorded Model Lab attribution requires a passed gate, accepted outcome refs, contributor refs, and receipt refs.',
    })
  }
}

const assertLoopRecord = (
  record: OmniModelLabRetainedFailureLoopRecord,
): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('Model Lab loop createdAtIso', record.createdAtIso)
  assertValidIso('Model Lab loop updatedAtIso', record.updatedAtIso)
  assertSafeRefs('Model Lab loop refs', [
    record.id,
    record.loopRef,
    ...record.blockerRefs,
    ...record.caveatRefs,
    ...record.sourceRefs,
  ])
  record.retainedFailures.forEach(assertRetainedFailure)

  const failureRefs = new Set(
    record.retainedFailures.map(failure => failure.failureRef),
  )
  record.candidateRecords.forEach(candidate =>
    assertCandidate(candidate, failureRefs),
  )

  const candidateRefs = new Set(
    record.candidateRecords.map(candidate => candidate.candidateRef),
  )
  record.evalReruns.forEach(evalRerun =>
    assertEval(evalRerun, failureRefs, candidateRefs),
  )

  const passedEvalRefs = new Set(
    record.evalReruns
      .filter(evalRerun => evalRerun.state === 'passed')
      .map(evalRerun => evalRerun.evalRef),
  )
  record.adapterValidations.forEach(validation =>
    assertAdapterValidation(validation, candidateRefs),
  )

  const passedAdapterRefs = new Set(
    record.adapterValidations
      .filter(validation => validation.state === 'passed')
      .map(validation => validation.validationRef),
  )
  record.promotionGates.forEach(gate =>
    assertPromotionGate(gate, candidateRefs, passedEvalRefs, passedAdapterRefs),
  )

  const passedGateCount = record.promotionGates.filter(
    gate => gate.state === 'passed',
  ).length
  record.attributions.forEach(attribution =>
    assertAttribution(attribution, candidateRefs, passedGateCount),
  )

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Blocked Model Lab loops require blocker refs.',
    })
  }

  if (
    loopAtLeast(record.state, 'retained') &&
    record.retainedFailures.length === 0
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Retained Model Lab loops require retained failures.',
    })
  }

  if (
    loopAtLeast(record.state, 'candidate_created') &&
    record.candidateRecords.length === 0
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Candidate-created Model Lab loops require candidate records.',
    })
  }

  if (loopAtLeast(record.state, 'eval_rerun') && passedEvalRefs.size === 0) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Eval-rerun Model Lab loops require passed eval reruns.',
    })
  }

  if (
    loopAtLeast(record.state, 'adapter_validated') &&
    passedAdapterRefs.size === 0
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Adapter-validated Model Lab loops require passed adapter validations.',
    })
  }

  if (loopAtLeast(record.state, 'gate_passed') && passedGateCount === 0) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Gate-passed Model Lab loops require passed promotion gates.',
    })
  }

  if (
    loopAtLeast(record.state, 'attributed') &&
    record.attributions.filter(item => item.state === 'recorded').length === 0
  ) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason: 'Attributed Model Lab loops require recorded attribution.',
    })
  }
}

const projectFailure = (
  failure: OmniModelLabRetainedFailureRecord,
  audience: OmniModelLabAudience,
): OmniModelLabRetainedFailureRecord | null => {
  const failureRef = refsForAudience(
    'Model Lab retained failure refs',
    [failure.failureRef],
    audience,
  )[0]

  if (failureRef === undefined) {
    return null
  }

  return {
    ...failure,
    evidenceRefs: refsForAudience(
      'Model Lab failure evidence refs',
      failure.evidenceRefs,
      audience,
    ),
    failureRef,
    redactionPolicyRefs: refsForAudience(
      'Model Lab failure redaction refs',
      failure.redactionPolicyRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Model Lab failure source refs',
      failure.sourceRefs,
      audience,
    ),
    traceRefs: refsForAudience(
      'Model Lab failure trace refs',
      failure.traceRefs,
      audience,
    ),
    workroomRefs: refsForAudience(
      'Model Lab failure workroom refs',
      failure.workroomRefs,
      audience,
    ),
  }
}

const projectCandidate = (
  candidate: OmniModelLabCandidateRecord,
  audience: OmniModelLabAudience,
): OmniModelLabCandidateRecord | null => {
  const candidateRef = refsForAudience(
    'Model Lab candidate refs',
    [candidate.candidateRef],
    audience,
  )[0]

  if (candidateRef === undefined) {
    return null
  }

  return {
    ...candidate,
    candidateRef,
    caveatRefs: refsForAudience(
      'Model Lab candidate caveat refs',
      candidate.caveatRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Model Lab candidate evidence refs',
      candidate.evidenceRefs,
      audience,
    ),
    modelRefs: refsForAudience(
      'Model Lab candidate model refs',
      candidate.modelRefs,
      audience,
    ),
    moduleVersionRefs: refsForAudience(
      'Model Lab candidate module refs',
      candidate.moduleVersionRefs,
      audience,
    ),
    signatureRefs: refsForAudience(
      'Model Lab candidate signature refs',
      candidate.signatureRefs,
      audience,
    ),
    sourceFailureRefs: refsForAudience(
      'Model Lab candidate source failure refs',
      candidate.sourceFailureRefs,
      audience,
    ),
  }
}

const projectEval = (
  evalRerun: OmniModelLabEvalRerunRecord,
  audience: OmniModelLabAudience,
): OmniModelLabEvalRerunRecord | null => {
  const evalRef = refsForAudience(
    'Model Lab eval refs',
    [evalRerun.evalRef],
    audience,
  )[0]

  if (evalRef === undefined) {
    return null
  }

  return {
    ...evalRerun,
    candidateRefs: refsForAudience(
      'Model Lab eval candidate refs',
      evalRerun.candidateRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Model Lab eval evidence refs',
      evalRerun.evidenceRefs,
      audience,
    ),
    evalRef,
    fixtureRefs: refsForAudience(
      'Model Lab eval fixture refs',
      evalRerun.fixtureRefs,
      audience,
    ),
    receiptRefs: refsForAudience(
      'Model Lab eval receipt refs',
      evalRerun.receiptRefs,
      audience,
    ),
    scorecardRefs: refsForAudience(
      'Model Lab eval scorecard refs',
      evalRerun.scorecardRefs,
      audience,
    ),
    sourceFailureRefs: refsForAudience(
      'Model Lab eval failure refs',
      evalRerun.sourceFailureRefs,
      audience,
    ),
  }
}

const projectAdapterValidation = (
  validation: OmniModelLabAdapterValidationRecord,
  audience: OmniModelLabAudience,
): OmniModelLabAdapterValidationRecord | null => {
  const validationRef = refsForAudience(
    'Model Lab adapter validation refs',
    [validation.validationRef],
    audience,
  )[0]

  if (validationRef === undefined) {
    return null
  }

  return {
    ...validation,
    adapterRefs: refsForAudience(
      'Model Lab adapter refs',
      validation.adapterRefs,
      audience,
    ),
    candidateRefs: refsForAudience(
      'Model Lab adapter candidate refs',
      validation.candidateRefs,
      audience,
    ),
    datasetRefs: refsForAudience(
      'Model Lab adapter dataset refs',
      validation.datasetRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Model Lab adapter evidence refs',
      validation.evidenceRefs,
      audience,
    ),
    providerRefs: refsForAudience(
      'Model Lab adapter provider refs',
      validation.providerRefs,
      audience,
    ),
    receiptRefs: refsForAudience(
      'Model Lab adapter receipt refs',
      validation.receiptRefs,
      audience,
    ),
    validationRef,
  }
}

const projectPromotionGate = (
  gate: OmniModelLabPromotionGateRecord,
  audience: OmniModelLabAudience,
): OmniModelLabPromotionGateRecord | null => {
  const gateRef = refsForAudience(
    'Model Lab promotion gate refs',
    [gate.gateRef],
    audience,
  )[0]

  if (gateRef === undefined) {
    return null
  }

  return {
    ...gate,
    adapterValidationRefs: refsForAudience(
      'Model Lab gate adapter refs',
      gate.adapterValidationRefs,
      audience,
    ),
    candidateRefs: refsForAudience(
      'Model Lab gate candidate refs',
      gate.candidateRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Model Lab gate evidence refs',
      gate.evidenceRefs,
      audience,
    ),
    evalRefs: refsForAudience('Model Lab gate eval refs', gate.evalRefs, audience),
    gateRef,
    policyRefs: refsForAudience(
      'Model Lab gate policy refs',
      gate.policyRefs,
      audience,
    ),
    reviewReceiptRefs: refsForAudience(
      'Model Lab gate review refs',
      gate.reviewReceiptRefs,
      audience,
    ),
    rollbackRefs: refsForAudience(
      'Model Lab gate rollback refs',
      gate.rollbackRefs,
      audience,
    ),
    targetRef: primaryRefForAudience(
      'Model Lab gate target refs',
      gate.targetRef,
      audience,
      'target.redacted',
    ),
  }
}

const projectAttribution = (
  attribution: OmniModelLabAttributionRecord,
  audience: OmniModelLabAudience,
): OmniModelLabAttributionRecord | null => {
  const attributionRef = refsForAudience(
    'Model Lab attribution refs',
    [attribution.attributionRef],
    audience,
  )[0]

  if (attributionRef === undefined) {
    return null
  }

  return {
    ...attribution,
    acceptedOutcomeRefs: refsForAudience(
      'Model Lab attribution accepted outcome refs',
      attribution.acceptedOutcomeRefs,
      audience,
    ),
    attributionRef,
    candidateRefs: refsForAudience(
      'Model Lab attribution candidate refs',
      attribution.candidateRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Model Lab attribution caveat refs',
      attribution.caveatRefs,
      audience,
    ),
    contributorRefs: refsForAudience(
      'Model Lab attribution contributor refs',
      attribution.contributorRefs,
      audience,
    ),
    receiptRefs: refsForAudience(
      'Model Lab attribution receipt refs',
      attribution.receiptRefs,
      audience,
    ),
  }
}

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(item => [...stringValues(item)])
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(item => [...stringValues(item)])
  }

  return []
}

export const omniModelLabProjectionHasPrivateMaterial = (
  projection: OmniModelLabRetainedFailureLoopProjection,
): boolean => {
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience)

  return (
    unsafeModelLabRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const projectOmniModelLabRetainedFailureLoop = (
  record: OmniModelLabRetainedFailureLoopRecord,
  audience: OmniModelLabAudience,
  nowIso: string,
): OmniModelLabRetainedFailureLoopProjection => {
  assertLoopRecord(record)

  const promotionGatePassedCount = record.promotionGates.filter(
    gate => gate.state === 'passed',
  ).length
  const latestGate = record.promotionGates.at(-1)

  const projection: OmniModelLabRetainedFailureLoopProjection = {
    adapterInstallationAllowed: false,
    adapterValidationPassedCount: record.adapterValidations.filter(
      validation => validation.state === 'passed',
    ).length,
    adapterValidations: record.adapterValidations
      .map(validation => projectAdapterValidation(validation, audience))
      .filter(
        (validation): validation is OmniModelLabAdapterValidationRecord =>
          validation !== null,
      ),
    attributions: record.attributions
      .map(attribution => projectAttribution(attribution, audience))
      .filter((attribution): attribution is OmniModelLabAttributionRecord =>
        attribution !== null,
      ),
    audience,
    authority: OMNI_MODEL_LAB_READ_ONLY_AUTHORITY,
    blockerRefs: refsForAudience(
      'Model Lab loop blocker refs',
      record.blockerRefs,
      audience,
    ),
    candidateCount: record.candidateRecords.length,
    candidateRecords: record.candidateRecords
      .map(candidate => projectCandidate(candidate, audience))
      .filter((candidate): candidate is OmniModelLabCandidateRecord =>
        candidate !== null,
      ),
    caveatRefs: refsForAudience(
      'Model Lab loop caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evalExecutionAllowed: false,
    evalPassedCount: record.evalReruns.filter(evalRerun =>
      evalRerun.state === 'passed'
    ).length,
    evalReruns: record.evalReruns
      .map(evalRerun => projectEval(evalRerun, audience))
      .filter((evalRerun): evalRerun is OmniModelLabEvalRerunRecord =>
        evalRerun !== null,
      ),
    id: primaryRefForAudience(
      'Model Lab loop id refs',
      record.id,
      audience,
      'model_lab_loop.redacted',
    ),
    loopRef: primaryRefForAudience(
      'Model Lab loop refs',
      record.loopRef,
      audience,
      'loop.redacted',
    ),
    modelTrainingMutationAllowed: false,
    payoutMutationAllowed: false,
    promotionGatePassedCount,
    promotionGates: record.promotionGates
      .map(gate => projectPromotionGate(gate, audience))
      .filter((gate): gate is OmniModelLabPromotionGateRecord =>
        gate !== null,
      ),
    publicClaimUpgradeAllowed: false,
    recordedAttributionCount: record.attributions.filter(
      attribution => attribution.state === 'recorded',
    ).length,
    retainedFailureCount: record.retainedFailures.length,
    retainedFailures: record.retainedFailures
      .map(failure => projectFailure(failure, audience))
      .filter((failure): failure is OmniModelLabRetainedFailureRecord =>
        failure !== null,
      ),
    rollbackPosture: latestGate?.rollbackPosture ?? 'missing',
    routingMutationAllowed: false,
    runtimePromotionAllowed: false,
    selfPromotionAttemptDetected: record.promotionGates.some(
      gate => gate.selfPromotionAttempt,
    ),
    settlementMutationAllowed: false,
    sourceRefs:
      audience === 'public' || audience === 'agent'
        ? []
        : refsForAudience('Model Lab loop source refs', record.sourceRefs, audience),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (omniModelLabProjectionHasPrivateMaterial(projection)) {
    throw new OmniModelLabRetainedFailureLoopUnsafe({
      reason:
        'Model Lab retained-failure loop projection contains private prompts, source archives, provider payloads, customer data, secrets, payment/wallet material, private repos, raw logs, raw traces, raw timestamps, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const exampleOmniModelLabRetainedFailureLoop =
  (): OmniModelLabRetainedFailureLoopRecord => ({
    adapterValidations: [
      {
        adapterRefs: ['adapter.public.site_revision_v2'],
        candidateRefs: ['candidate.public.site_revision_signature_v2'],
        datasetRefs: ['dataset.public.regression_fixture_refs'],
        evidenceRefs: ['evidence.public.adapter_validation_passed'],
        providerRefs: ['provider.public.local_model_lab'],
        receiptRefs: ['receipt.public.adapter_validation'],
        state: 'passed',
        validationRef: 'adapter.public.validation_site_revision_v2',
      },
    ],
    attributions: [
      {
        acceptedOutcomeRefs: ['outcome.public.site_revision_accepted'],
        attributionRef: 'attribution.public.site_revision_candidate',
        candidateRefs: ['candidate.public.site_revision_signature_v2'],
        caveatRefs: ['caveat.public.attribution_not_payout'],
        contributorRefs: ['contributor.public.model_lab_operator'],
        receiptRefs: ['receipt.public.model_lab_attribution'],
        state: 'recorded',
      },
    ],
    authority: OMNI_MODEL_LAB_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    candidateRecords: [
      {
        candidateRef: 'candidate.public.site_revision_signature_v2',
        caveatRefs: ['caveat.public.candidate_requires_release_gate'],
        evidenceRefs: ['evidence.public.retained_failure_replayed'],
        kind: 'program_signature',
        modelRefs: ['model.public.local_adapter_v2'],
        moduleVersionRefs: ['module.public.site_revision_v2'],
        signatureRefs: ['signature.public.site_revision_v2'],
        sourceFailureRefs: ['failure.public.site_revision_timeout'],
        state: 'needs_review',
      },
    ],
    caveatRefs: ['caveat.public.model_lab_evidence_only'],
    createdAtIso: '2026-06-06T22:00:00.000Z',
    evalReruns: [
      {
        candidateRefs: ['candidate.public.site_revision_signature_v2'],
        evidenceRefs: ['evidence.public.eval_rerun_passed'],
        evalRef: 'eval.public.site_revision_regression',
        fixtureRefs: ['fixture.public.site_revision_timeout'],
        receiptRefs: ['receipt.public.eval_rerun'],
        scorecardRefs: ['scorecard.public.eval_passed'],
        sourceFailureRefs: ['failure.public.site_revision_timeout'],
        state: 'passed',
      },
    ],
    id: 'model_lab_loop.public.site_revision_timeout',
    loopRef: 'loop.public.site_revision_timeout',
    promotionGates: [
      {
        adapterValidationRefs: ['adapter.public.validation_site_revision_v2'],
        candidateRefs: ['candidate.public.site_revision_signature_v2'],
        evidenceRefs: ['evidence.public.release_gate_passed'],
        evalRefs: ['eval.public.site_revision_regression'],
        gateRef: 'gate.public.site_revision_v2',
        policyRefs: ['policy.public.no_self_promotion'],
        reviewReceiptRefs: ['review.public.operator_approved'],
        rollbackPosture: 'ready',
        rollbackRefs: ['rollback.public.previous_signature_v1'],
        selfPromotionAttempt: false,
        state: 'passed',
        targetRef: 'signature.public.site_revision_v2',
      },
    ],
    retainedFailures: [
      {
        evidenceRefs: ['evidence.public.failure_replay'],
        failureRef: 'failure.public.site_revision_timeout',
        kind: 'provider_timeout',
        redactionPolicyRefs: ['redaction.public.refs_only'],
        sourceRefs: ['source.public.failure_summary'],
        state: 'triaged',
        traceRefs: ['trace.public.failure_summary'],
        workroomRefs: ['workroom.public.site_revision'],
      },
    ],
    sourceRefs: ['source.public.model_lab_loop_summary'],
    state: 'attributed',
    updatedAtIso: '2026-06-06T22:25:00.000Z',
  })
