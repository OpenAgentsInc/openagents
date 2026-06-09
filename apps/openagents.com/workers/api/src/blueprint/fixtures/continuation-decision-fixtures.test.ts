import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { decideBlueprintContinuation } from '../services/continuation-decision'
import {
  BLUEPRINT_CONTINUATION_DECISION_FIXTURES,
  BlueprintContinuationDecisionFixture as BlueprintContinuationDecisionFixtureSchema,
  blueprintContinuationDecisionFixtureHasPrivateMaterial,
} from './continuation-decision-fixtures'

describe('Blueprint continuation decision fixtures', () => {
  test('decode every retained first-batch fixture and cover each decision kind', () => {
    expect(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.map(
        fixture => fixture.expectedDecision,
      ),
    ).toStrictEqual([
      'continue',
      'test',
      'fix',
      'summarize',
      'request_context',
      'retry_account',
      'stop',
      'escalate',
      'prepare_review',
    ])

    expect(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.map(fixture =>
        S.decodeUnknownSync(BlueprintContinuationDecisionFixtureSchema)(
          fixture,
        ),
      ),
    ).toEqual(BLUEPRINT_CONTINUATION_DECISION_FIXTURES)
  })

  test('classifies fixtures through the continuation decision service', async () => {
    const decisions = await Promise.all(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.map(fixture =>
        Effect.runPromise(decideBlueprintContinuation(fixture.turnResult)),
      ),
    )

    expect(decisions.map(decision => decision.action)).toStrictEqual(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.map(
        fixture => fixture.expectedDecision,
      ),
    )
    expect(
      decisions.every(
        (decision, index) =>
          decision.programSignatureId ===
            BLUEPRINT_CONTINUATION_DECISION_FIXTURES[index]!
              .programSignatureId &&
          decision.evidenceRefs.length > 0 &&
          decision.receiptRefs.length > 0 &&
          decision.sourceAuthorityRefs.length > 0,
      ),
    ).toBe(true)
  })

  test('links every fixture to release-gate fixture refs and scorecards', () => {
    expect(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.every(
        fixture =>
          fixture.evalFixture.fixtureKind === 'continuation_decision' &&
          fixture.evalFixture.result === 'passed' &&
          fixture.evalFixture.id === `fixture.continuation.${fixture.id}` &&
          fixture.evalFixture.expectedOutputRef ===
            `expected.continuation.${fixture.expectedDecision}` &&
          fixture.evalFixture.scorecardRefs.length > 0 &&
          fixture.scorecardRefs.length > 0 &&
          fixture.sourceRefs.length > 0,
      ),
    ).toBe(true)
  })

  test('keeps retained fixtures free of raw private material', () => {
    expect(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.every(
        fixture => !blueprintContinuationDecisionFixtureHasPrivateMaterial(fixture),
      ),
    ).toBe(true)

    expect(
      blueprintContinuationDecisionFixtureHasPrivateMaterial({
        ...BLUEPRINT_CONTINUATION_DECISION_FIXTURES[0]!,
        sourceRefs: ['provider_token.raw_secret_material'],
      }),
    ).toBe(true)
  })
})
