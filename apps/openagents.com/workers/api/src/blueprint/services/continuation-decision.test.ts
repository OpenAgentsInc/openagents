import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BlueprintContinuationDecision as BlueprintContinuationDecisionSchema,
  type BlueprintContinuationDecisionKind,
  type BlueprintContinuationDirectEffectKind,
  type BlueprintContinuationTurnResult,
  blueprintContinuationDecisionIsEvidenceOnly,
} from '../schemas/continuation-decision'
import {
  assertBlueprintContinuationDecisionEvidenceOnly,
  BlueprintContinuationDirectEffectDenied,
  classifyBlueprintContinuationTurn,
  decideBlueprintContinuation,
  denyBlueprintContinuationDirectEffect,
} from './continuation-decision'

const baseTurn: BlueprintContinuationTurnResult = {
  accountFailureRefs: [],
  actorRef: 'actor.operator.test',
  blockerRefs: [],
  buildFailureRefs: [],
  classifierConfidence: 0.82,
  constraintRefs: ['constraint.no_direct_mutation'],
  evidenceRefs: ['evidence.context_pack_1'],
  id: 'turn_result.base',
  missingContextRefs: [],
  readyArtifactRefs: [],
  receiptRefs: ['receipt.program_run_1'],
  runtimeFailureRefs: [],
  sourceAuthorityRefs: ['source.workroom_1'],
  state: 'completed',
  summaryNeeded: false,
  testFailureRefs: [],
  unverifiedChangeRefs: [],
  updatedAt: '2026-06-06T00:00:00.000Z',
  userRequestedEscalation: false,
  userRequestedStop: false,
  workRef: 'workroom.site_1',
}

const turnForDecision = (
  action: BlueprintContinuationDecisionKind,
): BlueprintContinuationTurnResult => {
  if (action === 'escalate') {
    return {
      ...baseTurn,
      id: 'turn_result.escalate',
      userRequestedEscalation: true,
    }
  }

  if (action === 'stop') {
    return {
      ...baseTurn,
      id: 'turn_result.stop',
      userRequestedStop: true,
    }
  }

  if (action === 'request_context') {
    return {
      ...baseTurn,
      id: 'turn_result.request_context',
      missingContextRefs: ['context.customer_asset_missing'],
    }
  }

  if (action === 'retry_account') {
    return {
      ...baseTurn,
      accountFailureRefs: ['provider_account.chatgpt_3.low_credit'],
      id: 'turn_result.retry_account',
    }
  }

  if (action === 'fix') {
    return {
      ...baseTurn,
      buildFailureRefs: ['failure.build.vite'],
      id: 'turn_result.fix',
    }
  }

  if (action === 'test') {
    return {
      ...baseTurn,
      id: 'turn_result.test',
      unverifiedChangeRefs: ['artifact.diff.needs_test'],
    }
  }

  if (action === 'prepare_review') {
    return {
      ...baseTurn,
      id: 'turn_result.prepare_review',
      readyArtifactRefs: ['site_version.review_ready'],
    }
  }

  if (action === 'summarize') {
    return {
      ...baseTurn,
      id: 'turn_result.summarize',
      summaryNeeded: true,
    }
  }

  return {
    ...baseTurn,
    id: 'turn_result.continue',
  }
}

describe('Blueprint continuation decision service', () => {
  test.each([
    'continue',
    'test',
    'fix',
    'summarize',
    'request_context',
    'retry_account',
    'stop',
    'escalate',
    'prepare_review',
  ] satisfies ReadonlyArray<BlueprintContinuationDecisionKind>)(
    'classifies %s with Blueprint catalog refs and evidence-only policy',
    async expectedAction => {
      const turn = turnForDecision(expectedAction)
      const decision = await Effect.runPromise(decideBlueprintContinuation(turn))

      expect(classifyBlueprintContinuationTurn(turn)).toBe(expectedAction)
      expect(decision.action).toBe(expectedAction)
      expect(decision.programSignatureId).toBe(
        `program_signature.autopilot.${expectedAction}.v1`,
      )
      expect(decision.moduleVersionId).toBe(
        `module_version.autopilot.${expectedAction}.candidate_1`,
      )
      expect(decision.evidenceRefs).toEqual(turn.evidenceRefs)
      expect(decision.receiptRefs).toEqual(turn.receiptRefs)
      expect(decision.sourceAuthorityRefs).toEqual(turn.sourceAuthorityRefs)
      expect(blueprintContinuationDecisionIsEvidenceOnly(decision)).toBe(true)
      expect(
        S.decodeUnknownSync(BlueprintContinuationDecisionSchema)(decision),
      ).toEqual(decision)
    },
  )

  test('clamps classifier confidence into the public decision envelope', async () => {
    const low = await Effect.runPromise(
      decideBlueprintContinuation({
        ...baseTurn,
        classifierConfidence: -1,
        id: 'turn_result.low_confidence',
      }),
    )
    const high = await Effect.runPromise(
      decideBlueprintContinuation({
        ...baseTurn,
        classifierConfidence: 2,
        id: 'turn_result.high_confidence',
      }),
    )

    expect(low.confidence).toBe(0)
    expect(high.confidence).toBe(1)
  })

  test.each([
    'create_pull_request',
    'deploy',
    'mutate_source_fact',
    'send_email',
    'spend_money',
    'upgrade_public_claim',
  ] satisfies ReadonlyArray<BlueprintContinuationDirectEffectKind>)(
    'denies %s as a direct continuation effect',
    async effectKind => {
      const decision = await Effect.runPromise(
        decideBlueprintContinuation(baseTurn),
      )

      await expect(
        Effect.runPromise(
          denyBlueprintContinuationDirectEffect(decision, effectKind),
        ),
      ).rejects.toBeInstanceOf(BlueprintContinuationDirectEffectDenied)
      expect(decision.forbiddenDirectEffects).toContain(effectKind)
    },
  )

  test('rejects decisions that carry write-authority flags', async () => {
    const decision = await Effect.runPromise(decideBlueprintContinuation(baseTurn))

    await expect(
      Effect.runPromise(
        assertBlueprintContinuationDecisionEvidenceOnly({
          ...decision,
          noDeploy: false,
        }),
      ),
    ).rejects.toBeInstanceOf(BlueprintContinuationDirectEffectDenied)
  })
})
