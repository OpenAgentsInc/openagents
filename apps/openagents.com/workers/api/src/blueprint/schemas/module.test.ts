import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BlueprintModuleVersion,
  blueprintModuleVersionCanSelfPromote,
  BlueprintModuleVersion as BlueprintModuleVersionSchema,
  blueprintModuleVersionIsProduction,
  blueprintModuleVersionReleaseStateIsValid,
  blueprintModuleVersionRequiresOperatorPromotion,
} from './module'

const candidateModuleFixture: BlueprintModuleVersion = {
  artifactRefs: ['artifact.prompt_candidate'],
  deprecatedAt: null,
  id: 'module_version.continuation_prompt.candidate_1',
  implementationRef: 'prompt.autopilot_continuation.candidate_1',
  moduleKind: 'optimizer_candidate',
  moduleRef: 'module.autopilot_continuation_prompt',
  programSignatureId: 'program_signature.autopilot_continuation.v1',
  programTypeId: 'program_type.autopilot_continuation',
  provenance: {
    createdByRef: 'optimizer.retained_failure_loop',
    optimizerRunId: 'optimizer_run.retained_failure_1',
    retainedFailureRefs: ['failure.site_revision_missing_images'],
    sourceModuleVersionId: 'module_version.continuation_prompt.v1',
    trainingDataRefs: ['fixture.first_batch_sites'],
  },
  releaseDecision: null,
  releaseState: 'release_candidate',
  rollbackOfModuleVersionId: null,
  scorecards: [
    {
      higherIsBetter: true,
      metricRef: 'metric.fixture_pass_rate',
      scoreRef: 'score.fixture_pass_rate',
      value: 0.94,
    },
  ],
  status: 'candidate',
  versionRef: 'module_version.continuation_prompt.candidate_1',
}

const promotedModuleFixture: BlueprintModuleVersion = {
  ...candidateModuleFixture,
  id: 'module_version.continuation_prompt.v2',
  moduleKind: 'model_prompt',
  provenance: {
    ...candidateModuleFixture.provenance,
    createdByRef: 'operator.chris',
  },
  releaseDecision: {
    decidedAt: '2026-06-05T00:00:00.000Z',
    decidedByRef: 'operator.chris',
    decisionRef: 'decision.promote_continuation_prompt_v2',
    reasonRef: 'reason.fixture_passed_operator_review',
    releaseGateRef: 'gate.continuation_regression',
  },
  releaseState: 'production',
  status: 'promoted',
  versionRef: 'module_version.continuation_prompt.v2',
}

describe('Blueprint Module Version schema', () => {
  test('decode optimizer candidate module versions without granting production', () => {
    expect(
      S.decodeUnknownSync(BlueprintModuleVersionSchema)(candidateModuleFixture),
    ).toEqual(candidateModuleFixture)
    expect(blueprintModuleVersionCanSelfPromote(candidateModuleFixture)).toBe(
      false,
    )
    expect(blueprintModuleVersionRequiresOperatorPromotion(candidateModuleFixture)).toBe(
      true,
    )
    expect(blueprintModuleVersionReleaseStateIsValid(candidateModuleFixture)).toBe(
      true,
    )
    expect(blueprintModuleVersionIsProduction(candidateModuleFixture)).toBe(
      false,
    )
  })

  test('requires release decision for production modules', () => {
    expect(
      S.decodeUnknownSync(BlueprintModuleVersionSchema)(promotedModuleFixture),
    ).toEqual(promotedModuleFixture)
    expect(blueprintModuleVersionIsProduction(promotedModuleFixture)).toBe(true)
    expect(
      blueprintModuleVersionReleaseStateIsValid({
        ...promotedModuleFixture,
        releaseDecision: null,
      }),
    ).toBe(false)
  })

  test('requires rollback and deprecation anchors for terminal release states', () => {
    expect(
      blueprintModuleVersionReleaseStateIsValid({
        ...promotedModuleFixture,
        releaseState: 'rolled_back',
        rollbackOfModuleVersionId: null,
        status: 'rolled_back',
      }),
    ).toBe(false)
    expect(
      blueprintModuleVersionReleaseStateIsValid({
        ...promotedModuleFixture,
        deprecatedAt: null,
        releaseState: 'deprecated',
        status: 'deprecated',
      }),
    ).toBe(false)
  })
})
