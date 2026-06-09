import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BlueprintModuleVersion } from './module'
import {
  type BlueprintOptimizerRun,
  blueprintOptimizerCandidateRequiresReleaseGate,
  blueprintOptimizerOutputIsEvidenceOnly,
  blueprintOptimizerRunHasCandidateModules,
  BlueprintOptimizerRun as BlueprintOptimizerRunSchema,
} from './optimizer-run'

const candidateModule: BlueprintModuleVersion = {
  artifactRefs: ['artifact.optimizer_candidate_prompt'],
  deprecatedAt: null,
  id: 'module_version.autopilot.fix.candidate_2',
  implementationRef: 'prompt.autopilot.fix.candidate_2',
  moduleKind: 'optimizer_candidate',
  moduleRef: 'module.autopilot.fix',
  programSignatureId: 'program_signature.autopilot.fix.v1',
  programTypeId: 'program_type.autopilot.fix',
  provenance: {
    createdByRef: 'optimizer.retained_failure_replay',
    optimizerRunId: 'optimizer_run.fix_missing_image_1',
    retainedFailureRefs: ['failure.site_revision_missing_image'],
    sourceModuleVersionId: 'module_version.autopilot.fix.candidate_1',
    trainingDataRefs: ['fixture.first_batch_autopilot_runs'],
  },
  releaseDecision: null,
  releaseState: 'release_candidate',
  rollbackOfModuleVersionId: null,
  scorecards: [
    {
      higherIsBetter: true,
      metricRef: 'metric.fixture_pass_rate',
      scoreRef: 'score.optimizer_candidate_pass_rate',
      value: 0.91,
    },
  ],
  status: 'candidate',
  versionRef: 'module_version.autopilot.fix.candidate_2',
}

const optimizerRun: BlueprintOptimizerRun = {
  candidateModules: [
    {
      candidateState: 'needs_review',
      candidateSummaryRef: 'summary.fix_missing_image_candidate',
      moduleVersionId: 'module_version.autopilot.fix.candidate_2',
      releaseGateRef: 'release_gate.autopilot.fix.v1',
      scorecardRefs: ['score.optimizer_candidate_pass_rate'],
    },
  ],
  createdAt: '2026-06-05T00:00:00.000Z',
  evidenceRefs: ['evidence.retained_failure_replay_1'],
  id: 'optimizer_run.fix_missing_image_1',
  optimizerKind: 'retained_failure_replay',
  retainedFailureRefs: ['failure.site_revision_missing_image'],
  scorecardRefs: ['score.optimizer_candidate_pass_rate'],
  status: 'completed',
  updatedAt: '2026-06-05T00:00:00.000Z',
}

describe('Blueprint Optimizer Run schema', () => {
  test('decodes optimizer runs with candidate modules and scorecards', () => {
    expect(S.decodeUnknownSync(BlueprintOptimizerRunSchema)(optimizerRun)).toEqual(
      optimizerRun,
    )
    expect(blueprintOptimizerRunHasCandidateModules(optimizerRun)).toBe(true)
    expect(
      blueprintOptimizerCandidateRequiresReleaseGate(
        optimizerRun.candidateModules[0]!,
      ),
    ).toBe(true)
  })

  test('keeps optimizer output evidence-only until release-gated', () => {
    expect(
      blueprintOptimizerOutputIsEvidenceOnly(optimizerRun, [candidateModule]),
    ).toBe(true)
    expect(
      blueprintOptimizerOutputIsEvidenceOnly(optimizerRun, [
        {
          ...candidateModule,
          releaseDecision: {
            decidedAt: '2026-06-05T00:00:00.000Z',
            decidedByRef: 'optimizer.self',
            decisionRef: 'decision.self_promote',
            reasonRef: 'reason.invalid_self_promotion',
            releaseGateRef: 'release_gate.autopilot.fix.v1',
          },
          releaseState: 'production',
          status: 'promoted',
        },
      ]),
    ).toBe(false)
  })
})
