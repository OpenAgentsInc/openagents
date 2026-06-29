import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BlueprintEvalFixture,
  BlueprintEvalFixture as BlueprintEvalFixtureSchema,
  type BlueprintReleaseGate,
  blueprintReleaseGateCanPromote,
  BlueprintReleaseGate as BlueprintReleaseGateSchema,
  blueprintReleaseGatePreservesRollbackEvidence,
} from './release-gate'

const fixture: BlueprintEvalFixture = {
  evidenceRefs: ['evidence.fixture_run_1'],
  expectedOutputRef: 'expected.continuation_continue',
  fixtureKind: 'continuation_decision',
  id: 'fixture.continuation_1',
  inputRef: 'input.continuation_1',
  result: 'passed',
  scorecardRefs: ['scorecard.continuation_1'],
}

const approvedGate: BlueprintReleaseGate = {
  decidedByRef: 'operator.chris',
  decision: 'approved',
  decisionReasonRef: 'reason.fixtures_reviewed',
  fixturePassState: 'passed',
  fixtureRefs: ['fixture.continuation_1'],
  id: 'release_gate.continuation_signature_v1',
  policyState: 'compliant',
  receiptRefs: ['receipt.release_approval_1', 'receipt.rollback_plan_1'],
  reviewState: 'approved',
  rollbackPosture: 'verified',
  scorecardRef: 'scorecard.continuation_release',
  selfPromotionAttempt: false,
  targetKind: 'program_signature',
  targetRef: 'program_signature.autopilot_continuation.v1',
}

describe('Blueprint Release Gate and eval fixture schemas', () => {
  test('decodes eval fixtures and approved release gates', () => {
    expect(S.decodeUnknownSync(BlueprintEvalFixtureSchema)(fixture)).toEqual(
      fixture,
    )
    expect(S.decodeUnknownSync(BlueprintReleaseGateSchema)(approvedGate)).toEqual(
      approvedGate,
    )
  })

  test('allows promotion only when fixtures, review, policy, rollback, scorecard, receipts, and decision pass', () => {
    expect(blueprintReleaseGateCanPromote(approvedGate)).toBe(true)
    expect(blueprintReleaseGatePreservesRollbackEvidence(approvedGate)).toBe(
      true,
    )
    expect(
      blueprintReleaseGateCanPromote({
        ...approvedGate,
        receiptRefs: [],
      }),
    ).toBe(false)
    expect(
      blueprintReleaseGateCanPromote({
        ...approvedGate,
        selfPromotionAttempt: true,
      }),
    ).toBe(false)
  })

  test('blocks promotion without rollback evidence or explicit approval', () => {
    expect(
      blueprintReleaseGateCanPromote({
        ...approvedGate,
        decision: null,
        decidedByRef: null,
      }),
    ).toBe(false)
    expect(
      blueprintReleaseGatePreservesRollbackEvidence({
        ...approvedGate,
        receiptRefs: ['receipt.release_approval_1'],
        rollbackPosture: 'ready',
      }),
    ).toBe(false)
  })
})
