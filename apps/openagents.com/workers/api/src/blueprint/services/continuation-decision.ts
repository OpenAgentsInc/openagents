import { Effect, Match as M, Schema as S } from 'effect'

import {
  AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
  AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES,
} from '../fixtures/autopilot-continuation-signatures'
import {
  type BlueprintContinuationDecision,
  BlueprintContinuationDecisionKind,
  type BlueprintContinuationDecisionKind as BlueprintContinuationDecisionKindType,
  BlueprintContinuationDirectEffectKind,
  type BlueprintContinuationDirectEffectKind as BlueprintContinuationDirectEffectKindType,
  type BlueprintContinuationTurnResult,
  blueprintContinuationDecisionIsEvidenceOnly,
} from '../schemas/continuation-decision'

export class BlueprintContinuationCatalogError extends S.TaggedErrorClass<BlueprintContinuationCatalogError>()(
  'BlueprintContinuationCatalogError',
  {
    action: BlueprintContinuationDecisionKind,
    reason: S.String,
  },
) {}

export class BlueprintContinuationDirectEffectDenied extends S.TaggedErrorClass<BlueprintContinuationDirectEffectDenied>()(
  'BlueprintContinuationDirectEffectDenied',
  {
    decisionRef: S.String,
    effectKind: BlueprintContinuationDirectEffectKind,
    reason: S.String,
  },
) {}

export type BlueprintContinuationDecisionError =
  | BlueprintContinuationCatalogError
  | BlueprintContinuationDirectEffectDenied

const forbiddenDirectEffects: ReadonlyArray<BlueprintContinuationDirectEffectKindType> =
  [
    'create_pull_request',
    'deploy',
    'mutate_source_fact',
    'send_email',
    'spend_money',
    'upgrade_public_claim',
  ]

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const clampConfidence = (confidence: number): number => {
  if (confidence < 0) {
    return 0
  }

  if (confidence > 1) {
    return 1
  }

  return confidence
}

export const classifyBlueprintContinuationTurn = (
  turn: BlueprintContinuationTurnResult,
): BlueprintContinuationDecisionKindType => {
  if (turn.userRequestedEscalation) {
    return 'escalate'
  }

  if (turn.userRequestedStop) {
    return 'stop'
  }

  if (hasRefs(turn.missingContextRefs)) {
    return 'request_context'
  }

  if (hasRefs(turn.accountFailureRefs)) {
    return 'retry_account'
  }

  if (
    hasRefs(turn.buildFailureRefs) ||
    hasRefs(turn.runtimeFailureRefs) ||
    hasRefs(turn.testFailureRefs)
  ) {
    return 'fix'
  }

  if (hasRefs(turn.unverifiedChangeRefs)) {
    return 'test'
  }

  if (hasRefs(turn.readyArtifactRefs)) {
    return 'prepare_review'
  }

  if (turn.summaryNeeded) {
    return 'summarize'
  }

  if (turn.state === 'blocked' || hasRefs(turn.blockerRefs)) {
    return 'escalate'
  }

  return 'continue'
}

const reasonForDecision = (
  action: BlueprintContinuationDecisionKindType,
): string =>
  M.value(action).pipe(
    M.when('continue', () => 'No blocker is visible; continue the current run.'),
    M.when('test', () => 'Generated or changed artifacts need verification.'),
    M.when('fix', () => 'Failure evidence requires a repair turn before review.'),
    M.when('summarize', () => 'A concise state summary is needed before more work.'),
    M.when(
      'request_context',
      () => 'Required customer, source, or workroom context is missing.',
    ),
    M.when(
      'retry_account',
      () => 'Provider account capacity or auth failed and should be retried or rotated.',
    ),
    M.when('stop', () => 'A stop condition was requested or detected.'),
    M.when(
      'escalate',
      () => 'The run needs operator attention before it can safely continue.',
    ),
    M.when(
      'prepare_review',
      () => 'Reviewable artifacts are ready for customer or operator inspection.',
    ),
    M.exhaustive,
  )

const findProgramSignature = (
  action: BlueprintContinuationDecisionKindType,
) =>
  AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES.find(
    signature => signature.id === `program_signature.autopilot.${action}.v1`,
  )

const findModuleVersion = (
  action: BlueprintContinuationDecisionKindType,
) =>
  AUTOPILOT_CONTINUATION_MODULE_VERSIONS.find(
    moduleVersion =>
      moduleVersion.id === `module_version.autopilot.${action}.candidate_1`,
  )

export const decideBlueprintContinuation = (
  turn: BlueprintContinuationTurnResult,
): Effect.Effect<BlueprintContinuationDecision, BlueprintContinuationCatalogError> =>
  Effect.gen(function* () {
    const action = classifyBlueprintContinuationTurn(turn)
    const signature = findProgramSignature(action)
    const moduleVersion = findModuleVersion(action)

    if (signature === undefined) {
      return yield* new BlueprintContinuationCatalogError({
        action,
        reason:
          'No Blueprint Program Signature exists for the selected continuation decision.',
      })
    }

    return {
      action,
      actionSubmissionRequiredForDirectEffects: true,
      authorityBoundary: 'evidence_only',
      confidence: clampConfidence(turn.classifierConfidence),
      constraintRefs: turn.constraintRefs,
      decisionRef: `continuation_decision.${turn.id}.${action}`,
      directMutationDisabled: true,
      evidenceRefs: turn.evidenceRefs,
      forbiddenDirectEffects,
      moduleVersionId: moduleVersion?.id ?? null,
      noDeploy: true,
      noEmail: true,
      noPublicClaimUpgrade: true,
      noSourceMutation: true,
      noSpend: true,
      programSignatureId: signature.id,
      programTypeId: signature.programTypeId,
      reason: reasonForDecision(action),
      receiptRefs: turn.receiptRefs,
      sourceAuthorityRefs: turn.sourceAuthorityRefs,
      turnResultRef: turn.id,
      workRef: turn.workRef,
    }
  })

export const denyBlueprintContinuationDirectEffect = (
  decision: BlueprintContinuationDecision,
  effectKind: BlueprintContinuationDirectEffectKindType,
): Effect.Effect<never, BlueprintContinuationDirectEffectDenied> =>
  Effect.fail(
    new BlueprintContinuationDirectEffectDenied({
      decisionRef: decision.decisionRef,
      effectKind,
      reason:
        'Blueprint continuation decisions are evidence-only. Direct effects must be represented as approval-gated Action Submissions.',
    }),
  )

export const assertBlueprintContinuationDecisionEvidenceOnly = (
  decision: BlueprintContinuationDecision,
): Effect.Effect<BlueprintContinuationDecision, BlueprintContinuationDirectEffectDenied> =>
  blueprintContinuationDecisionIsEvidenceOnly(decision)
    ? Effect.succeed(decision)
    : Effect.fail(
        new BlueprintContinuationDirectEffectDenied({
          decisionRef: decision.decisionRef,
          effectKind: 'mutate_source_fact',
          reason:
            'Blueprint continuation decision contains write-authority flags and cannot be accepted as evidence-only.',
        }),
      )
