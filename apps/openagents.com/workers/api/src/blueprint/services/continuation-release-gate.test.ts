import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
  AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES,
  AUTOPILOT_CONTINUATION_RELEASE_GATES,
} from '../fixtures/autopilot-continuation-signatures'
import type { BlueprintModuleVersion } from '../schemas/module'
import type { BlueprintProgramSignature } from '../schemas/program'
import {
  BlueprintContinuationReleaseGateResult,
} from '../schemas/continuation-release-gate'
import type { BlueprintReleaseGate } from '../schemas/release-gate'
import { evaluateBlueprintContinuationReleaseGate } from './continuation-release-gate'

const continuationSignature = AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES.find(
  signature => signature.id === 'program_signature.autopilot.continue.v1',
)!
const continuationModule = AUTOPILOT_CONTINUATION_MODULE_VERSIONS.find(
  moduleVersion =>
    moduleVersion.id === 'module_version.autopilot.continue.candidate_1',
)!
const draftGate = AUTOPILOT_CONTINUATION_RELEASE_GATES.find(
  gate => gate.targetRef === continuationSignature.id,
)!

const approvedGate = (
  overrides: Partial<BlueprintReleaseGate> = {},
): BlueprintReleaseGate => ({
  decidedByRef: 'operator.chris',
  decision: 'approved',
  decisionReasonRef: 'decision.continuation_release.reviewed',
  fixturePassState: 'passed',
  fixtureRefs: ['fixture.continuation.continue.v1'],
  id: 'release_gate.autopilot.continue.v1',
  policyState: 'compliant',
  receiptRefs: [
    'receipt.continuation_release.approval',
    'receipt.continuation_release.rollback_anchor',
  ],
  reviewState: 'approved',
  rollbackPosture: 'verified',
  scorecardRef: 'scorecard.continuation_release.continue',
  selfPromotionAttempt: false,
  targetKind: 'program_signature',
  targetRef: continuationSignature.id,
  ...overrides,
})

describe('Blueprint continuation release gate', () => {
  test('allows a continuation Program Signature only with a complete operator-approved gate', () => {
    const result = evaluateBlueprintContinuationReleaseGate({
      gate: approvedGate(),
      target: {
        _tag: 'ProgramSignature',
        programSignature: continuationSignature,
      },
    })

    expect(S.decodeUnknownSync(BlueprintContinuationReleaseGateResult)(result))
      .toEqual(result)
    expect(result.canPromote).toBe(true)
    expect(result.failureRefs).toEqual([])
  })

  test('allows a continuation Module Version only with a matching module-version gate', () => {
    const result = evaluateBlueprintContinuationReleaseGate({
      gate: approvedGate({
        id: 'release_gate.autopilot.continue.module_candidate_1',
        targetKind: 'module_version',
        targetRef: continuationModule.id,
      }),
      target: {
        _tag: 'ModuleVersion',
        moduleVersion: continuationModule,
      },
    })

    expect(result.canPromote).toBe(true)
    expect(result.targetKind).toBe('module_version')
    expect(result.targetRef).toBe(continuationModule.id)
  })

  test('blocks seeded draft gates from granting runtime authority by themselves', () => {
    const result = evaluateBlueprintContinuationReleaseGate({
      gate: draftGate,
      target: {
        _tag: 'ProgramSignature',
        programSignature: continuationSignature,
      },
    })

    expect(result.canPromote).toBe(false)
    expect(result.failureRefs).toEqual(
      expect.arrayContaining([
        'failure.continuation_release.fixtures_not_passed',
        'failure.continuation_release.operator_decision_missing',
        'failure.continuation_release.rollback_anchor_missing',
      ]),
    )
  })

  test('rejects missing fixtures, missing rollback anchor, missing operator decision, and self-promotion', () => {
    const result = evaluateBlueprintContinuationReleaseGate({
      gate: approvedGate({
        decidedByRef: 'agent.self_promoter',
        decisionReasonRef: null,
        fixtureRefs: [],
        receiptRefs: ['receipt.continuation_release.approval'],
        selfPromotionAttempt: true,
      }),
      target: {
        _tag: 'ProgramSignature',
        programSignature: continuationSignature,
      },
    })

    expect(result.canPromote).toBe(false)
    expect(result.failureRefs).toEqual(
      expect.arrayContaining([
        'failure.continuation_release.fixture_refs_missing',
        'failure.continuation_release.operator_decision_missing',
        'failure.continuation_release.rollback_anchor_missing',
        'failure.continuation_release.self_promotion_attempt',
      ]),
    )
  })

  test('rejects target mismatch and non-continuation targets', () => {
    const nonContinuationSignature: BlueprintProgramSignature = {
      ...continuationSignature,
      id: 'program_signature.autopilot.email_decisioning.v1',
      programTypeId: 'program_type.autopilot.email_decisioning',
      supportsContinuation: false,
    }
    const alreadyProductionModule: BlueprintModuleVersion = {
      ...continuationModule,
      id: 'module_version.autopilot.continue.production_1',
      releaseDecision: {
        decidedAt: '2026-06-06T00:00:00.000Z',
        decidedByRef: 'operator.chris',
        decisionRef: 'decision.promoted',
        reasonRef: 'reason.promoted',
        releaseGateRef: 'release_gate.autopilot.continue.module_candidate_1',
      },
      releaseState: 'production',
      status: 'promoted',
    }
    const signatureResult = evaluateBlueprintContinuationReleaseGate({
      gate: approvedGate({
        targetRef: 'program_signature.other.v1',
      }),
      target: {
        _tag: 'ProgramSignature',
        programSignature: nonContinuationSignature,
      },
    })
    const moduleResult = evaluateBlueprintContinuationReleaseGate({
      gate: approvedGate({
        targetKind: 'module_version',
        targetRef: alreadyProductionModule.id,
      }),
      target: {
        _tag: 'ModuleVersion',
        moduleVersion: alreadyProductionModule,
      },
    })

    expect(signatureResult.canPromote).toBe(false)
    expect(signatureResult.failureRefs).toEqual(
      expect.arrayContaining([
        'failure.continuation_release.target_not_continuation',
        'failure.continuation_release.target_ref_mismatch',
      ]),
    )
    expect(moduleResult.canPromote).toBe(false)
    expect(moduleResult.failureRefs).toContain(
      'failure.continuation_release.module_already_production',
    )
  })
})
