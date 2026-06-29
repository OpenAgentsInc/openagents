import { Schema as S } from 'effect'

import {
  type BlueprintContinuationDecision,
  BlueprintContinuationDecisionKind,
  blueprintContinuationDecisionIsEvidenceOnly,
} from './blueprint/schemas/continuation-decision'
import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  CodingAutopilotDecisionActionKind,
} from './coding-autopilot-decision-actions'

export const CodingAutopilotContinuationGuardrailState = S.Literals([
  'blocked',
  'failed',
  'needs_action_submission',
  'passed',
])
export type CodingAutopilotContinuationGuardrailState =
  typeof CodingAutopilotContinuationGuardrailState.Type

export class CodingAutopilotContinuationDecisionRecord extends S.Class<CodingAutopilotContinuationDecisionRecord>(
  'CodingAutopilotContinuationDecisionRecord',
)({
  actionSubmissionRequiredForDirectEffects: S.Boolean,
  confidence: S.Number,
  constraintRefs: S.Array(S.String),
  customerExplanationRef: S.String,
  decisionRef: S.String,
  directEffectPermitted: S.Literal(false),
  evidenceOnly: S.Literal(true),
  evidenceRefs: S.Array(S.String),
  guardrailState: CodingAutopilotContinuationGuardrailState,
  id: S.String,
  missionRef: S.String,
  moduleVersionId: S.NullOr(S.String),
  programRunRef: S.NullOr(S.String),
  programSignatureId: S.String,
  programTypeId: S.String,
  queuedActionKind: CodingAutopilotDecisionActionKind,
  receiptRefs: S.Array(S.String),
  rejectedAlternativeRefs: S.Array(S.String),
  riskRefs: S.Array(S.String),
  selectedContinuationAction: BlueprintContinuationDecisionKind,
  sourceAuthorityRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workRef: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotContinuationDecisionProjection extends S.Class<CodingAutopilotContinuationDecisionProjection>(
  'CodingAutopilotContinuationDecisionProjection',
)({
  actionSubmissionRequiredForDirectEffects: S.Boolean,
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  confidence: S.Number,
  confidenceBucket: S.String,
  constraintRefs: S.Array(S.String),
  customerExplanationRef: S.String,
  decisionRef: S.String,
  directEffectPermitted: S.Literal(false),
  evidenceOnly: S.Literal(true),
  evidenceRefs: S.Array(S.String),
  guardrailState: CodingAutopilotContinuationGuardrailState,
  id: S.String,
  missionRef: S.String,
  moduleVersionId: S.NullOr(S.String),
  programRunRef: S.NullOr(S.String),
  programSignatureId: S.String,
  programTypeId: S.String,
  queuedActionKind: CodingAutopilotDecisionActionKind,
  receiptRefs: S.Array(S.String),
  rejectedAlternativeRefs: S.Array(S.String),
  riskRefs: S.Array(S.String),
  selectedContinuationAction: BlueprintContinuationDecisionKind,
  sourceAuthorityRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  workRef: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotContinuationDecisionUnsafe extends S.TaggedErrorClass<CodingAutopilotContinuationDecisionUnsafe>()(
  'CodingAutopilotContinuationDecisionUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i
const publicUnsafeRefPattern =
  /(program_run|source[_-]?authority|workroom\.)/i
const customerUnsafeRefPattern =
  /(source[_-]?authority|workroom\.private)/i
const teamUnsafeRefPattern =
  /(source[_-]?authority)/i

const queuedActionForContinuationAction:
  Record<BlueprintContinuationDecisionKind, CodingAutopilotDecisionActionKind> =
  {
    continue: 'continue',
    escalate: 'request_customer_input',
    fix: 'steer',
    prepare_review: 'approve_pr_draft',
    request_context: 'provide_context',
    retry_account: 'retry_account',
    stop: 'stop',
    summarize: 'continue',
    test: 'rerun_tests',
  }

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const confidenceBucket = (confidence: number): string => {
  if (confidence >= 0.85) {
    return 'high'
  }

  if (confidence >= 0.55) {
    return 'medium'
  }

  return 'low'
}

const assertNoUniversalPrivateMaterial = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new CodingAutopilotContinuationDecisionUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw artifact material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertNoUniversalPrivateMaterial(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeNullableRefForAudience = (
  label: string,
  ref: string | null,
  audience: BlueprintMissionBriefingAudience,
): string | null => {
  if (ref === null) {
    return null
  }

  return safeRefsForAudience(label, [ref], audience)[0] ?? null
}

const assertRecordSafe = (
  record: CodingAutopilotContinuationDecisionRecord,
): void => {
  assertNoUniversalPrivateMaterial('continuation decision identity refs', [
    record.id,
    record.decisionRef,
    record.missionRef,
    record.moduleVersionId ?? '',
    record.programRunRef ?? '',
    record.programSignatureId,
    record.programTypeId,
    record.customerExplanationRef,
    record.workRef,
  ])
  assertNoUniversalPrivateMaterial('continuation decision workroom refs', record.workroomRefs)
  assertNoUniversalPrivateMaterial('continuation decision constraint refs', record.constraintRefs)
  assertNoUniversalPrivateMaterial('continuation decision evidence refs', record.evidenceRefs)
  assertNoUniversalPrivateMaterial('continuation decision receipt refs', record.receiptRefs)
  assertNoUniversalPrivateMaterial('continuation decision source refs', record.sourceAuthorityRefs)
  assertNoUniversalPrivateMaterial('continuation decision rejected refs', record.rejectedAlternativeRefs)
  assertNoUniversalPrivateMaterial('continuation decision risk refs', record.riskRefs)

  if (
    !record.evidenceOnly ||
    record.directEffectPermitted ||
    !record.actionSubmissionRequiredForDirectEffects
  ) {
    throw new CodingAutopilotContinuationDecisionUnsafe({
      reason:
        'Continuation decision records must be evidence-only and require Action Submissions for direct effects.',
    })
  }
}

export const codingAutopilotContinuationDecisionProjectionHasPrivateMaterial =
  (
    projection: CodingAutopilotContinuationDecisionProjection,
  ): boolean =>
    universallyUnsafeRefPattern.test(JSON.stringify(projection)) ||
    /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(JSON.stringify(projection))

export const codingAutopilotContinuationDecisionRecordFromBlueprint = (
  input: Readonly<{
    customerExplanationRef: string
    decision: BlueprintContinuationDecision
    guardrailState: CodingAutopilotContinuationGuardrailState
    id: string
    missionRef: string
    programRunRef: string | null
    rejectedAlternativeRefs?: ReadonlyArray<string> | undefined
    riskRefs?: ReadonlyArray<string> | undefined
    updatedAtIso: string
    workroomRefs?: ReadonlyArray<string> | undefined
  }>,
): CodingAutopilotContinuationDecisionRecord => {
  if (!blueprintContinuationDecisionIsEvidenceOnly(input.decision)) {
    throw new CodingAutopilotContinuationDecisionUnsafe({
      reason:
        'Blueprint continuation decision is not evidence-only and cannot become a mission decision record.',
    })
  }

  return {
    actionSubmissionRequiredForDirectEffects:
      input.decision.actionSubmissionRequiredForDirectEffects,
    confidence: input.decision.confidence,
    constraintRefs: input.decision.constraintRefs,
    customerExplanationRef: input.customerExplanationRef,
    decisionRef: input.decision.decisionRef,
    directEffectPermitted: false,
    evidenceOnly: true,
    evidenceRefs: input.decision.evidenceRefs,
    guardrailState: input.guardrailState,
    id: input.id,
    missionRef: input.missionRef,
    moduleVersionId: input.decision.moduleVersionId,
    programRunRef: input.programRunRef,
    programSignatureId: input.decision.programSignatureId,
    programTypeId: input.decision.programTypeId,
    queuedActionKind: queuedActionForContinuationAction[input.decision.action],
    receiptRefs: input.decision.receiptRefs,
    rejectedAlternativeRefs: [...(input.rejectedAlternativeRefs ?? [])],
    riskRefs: [...(input.riskRefs ?? [])],
    selectedContinuationAction: input.decision.action,
    sourceAuthorityRefs: input.decision.sourceAuthorityRefs,
    updatedAtIso: input.updatedAtIso,
    workRef: input.decision.workRef,
    workroomRefs: [...(input.workroomRefs ?? [])],
  }
}

export const projectCodingAutopilotContinuationDecisionRecord = (
  record: CodingAutopilotContinuationDecisionRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): CodingAutopilotContinuationDecisionProjection => {
  assertRecordSafe(record)

  const projection: CodingAutopilotContinuationDecisionProjection = {
    actionSubmissionRequiredForDirectEffects:
      record.actionSubmissionRequiredForDirectEffects,
    audience,
    confidence: record.confidence,
    confidenceBucket: confidenceBucket(record.confidence),
    constraintRefs: safeRefsForAudience(
      'continuation decision constraint refs',
      record.constraintRefs,
      audience,
    ),
    customerExplanationRef: safeRefsForAudience(
      'continuation decision explanation ref',
      [record.customerExplanationRef],
      audience,
    )[0] ?? 'explanation.redacted',
    decisionRef: record.decisionRef,
    directEffectPermitted: false,
    evidenceOnly: true,
    evidenceRefs: safeRefsForAudience(
      'continuation decision evidence refs',
      record.evidenceRefs,
      audience,
    ),
    guardrailState: record.guardrailState,
    id: record.id,
    missionRef: record.missionRef,
    moduleVersionId: safeNullableRefForAudience(
      'continuation decision module version ref',
      record.moduleVersionId,
      audience,
    ),
    programRunRef: safeNullableRefForAudience(
      'continuation decision Program Run ref',
      record.programRunRef,
      audience,
    ),
    programSignatureId: record.programSignatureId,
    programTypeId: record.programTypeId,
    queuedActionKind: record.queuedActionKind,
    receiptRefs: safeRefsForAudience(
      'continuation decision receipt refs',
      record.receiptRefs,
      audience,
    ),
    rejectedAlternativeRefs: safeRefsForAudience(
      'continuation decision rejected refs',
      record.rejectedAlternativeRefs,
      audience,
    ),
    riskRefs: safeRefsForAudience(
      'continuation decision risk refs',
      record.riskRefs,
      audience,
    ),
    selectedContinuationAction: record.selectedContinuationAction,
    sourceAuthorityRefs: audience === 'operator'
      ? safeRefsForAudience('continuation decision source refs', record.sourceAuthorityRefs, audience)
      : [],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workRef: record.workRef,
    workroomRefs: audience === 'public'
      ? []
      : safeRefsForAudience('continuation decision workroom refs', record.workroomRefs, audience),
  }

  if (codingAutopilotContinuationDecisionProjectionHasPrivateMaterial(projection)) {
    throw new CodingAutopilotContinuationDecisionUnsafe({
      reason:
        'Continuation decision projection contains private material or raw timestamps.',
    })
  }

  return projection
}

export const exampleCodingAutopilotContinuationDecisionRecord =
  (): CodingAutopilotContinuationDecisionRecord =>
    codingAutopilotContinuationDecisionRecordFromBlueprint({
      customerExplanationRef: 'explanation.continuation.retry_account',
      decision: {
        action: 'retry_account',
        actionSubmissionRequiredForDirectEffects: true,
        authorityBoundary: 'evidence_only',
        confidence: 0.82,
        constraintRefs: ['constraint.use_connected_account_fleet'],
        decisionRef: 'continuation_decision.otec_revision_4.retry_account',
        directMutationDisabled: true,
        evidenceRefs: ['evidence.account_rate_limit'],
        forbiddenDirectEffects: [
          'create_pull_request',
          'deploy',
          'mutate_source_fact',
          'send_email',
          'spend_money',
          'upgrade_public_claim',
        ],
        moduleVersionId: 'module_version.autopilot.retry_account.candidate_1',
        noDeploy: true,
        noEmail: true,
        noPublicClaimUpgrade: true,
        noSourceMutation: true,
        noSpend: true,
        programSignatureId: 'program_signature.autopilot.retry_account.v1',
        programTypeId: 'program_type.autopilot.continuation.v1',
        reason:
          'Provider account capacity or auth failed and should be retried or rotated.',
        receiptRefs: ['receipt.account_failover.otec_revision_4'],
        sourceAuthorityRefs: ['source_authority.account_fleet_health'],
        turnResultRef: 'turn_result.otec_revision_4.interrupted',
        workRef: 'mission.otec_revision_4',
      },
      guardrailState: 'needs_action_submission',
      id: 'continuation_decision_record_otec_revision_4_retry_account',
      missionRef: 'mission.otec_revision_4',
      programRunRef: 'program_run.continuation.otec_revision_4',
      rejectedAlternativeRefs: [
        'rejected.continue_without_account_retry',
        'rejected.stop_without_accepted_outcome',
      ],
      riskRefs: ['risk.account_rotation_needed'],
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      workroomRefs: ['workroom.otec_site_revision_4'],
    })
