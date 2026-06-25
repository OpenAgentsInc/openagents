import { describe, expect, test } from 'vitest'

import {
  buildBenchmarkReport,
  type BenchmarkCell,
  type BenchmarkLaneSample,
  makeRealLaneSeam,
  runBenchmark,
} from '../benchmark'
import { buildServedTokensIngestBody } from '../served-tokens-recorder'
import {
  buildGymDogfoodServedTokensInputs,
  buildGymTrainingRewardBundle,
  evaluateGymFlywheelCandidate,
  GymFlywheelUnsafe,
  summarizeGymCostPerAcceptedOutcome,
} from './flywheel'
import {
  OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  compileGymExperiment,
  type GymExperiment,
} from './experiment'

const REALISTIC_SHAPE = {
  id: 'observed-opencode-flywheel-run',
  inputTokens: 1600,
  outputTokens: 600,
  cacheablePrefixTokens: 800,
  concurrency: 1,
  provenance: 'realistic',
} as const

const FLYWHEEL_EXPERIMENT: GymExperiment = {
  ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  id: 'gym-opencode-khala-flywheel-test-v1',
  policy: {
    ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT.policy,
    fanout: {
      ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT.policy.fanout,
      lanes: ['khala'],
    },
  },
  shapes: [REALISTIC_SHAPE],
  budget: {
    spendCapMsat: 10_000_000,
    maxBillableSamples: 10,
    seam: 'real',
    ownerApprovalRef: 'approval.public.gym.flywheel.test',
  },
}

const sampleForCost = (
  costBasisMsat: number,
): ((cell: BenchmarkCell, sampleIndex: number) => BenchmarkLaneSample) =>
  (cell, sampleIndex) => ({
    promptTokens: cell.shape.inputTokens,
    completionTokens: cell.shape.outputTokens,
    totalTokens: cell.shape.inputTokens + cell.shape.outputTokens,
    cachedInputTokens: Math.floor(cell.shape.cacheablePrefixTokens * 0.75),
    ttftMs: 220 + sampleIndex,
    totalWallClockMs: 3_900 + sampleIndex,
    generationWallClockMs: 3_600 + sampleIndex,
    providerTimeMs: 3_820 + sampleIndex,
    gatewayOverheadMs: 80,
    verificationClass: 'test_passed',
    executedVerdict: 'passed',
    scalarReward: 1,
    verifierTimeMs: 900,
    costBasisMsat,
    region: 'openagents',
    clientSurface: {
      client: 'opencode',
      taskRef: 'gym.flywheel.opencode.edit-run-smoke.v1',
      configRef: `opencode.flywheel.${cell.lane}.v1`,
      toolCallsAttempted: 2,
      toolCallsSucceeded: 2,
    },
  })

const decisionGradeReport = (costBasisMsat: number) => {
  const compiled = compileGymExperiment(FLYWHEEL_EXPERIMENT)
  const runSet = runBenchmark(
    compiled.matrixConfig,
    makeRealLaneSeam({
      armRealSweep: true,
      executor: sampleForCost(costBasisMsat),
    }),
  )
  return { compiled, runSet, report: buildBenchmarkReport(runSet) }
}

describe('Gym training flywheel', () => {
  test('projects Gym reports into GEPA/TRINITY/Conductor reward bundles', () => {
    const { compiled, report } = decisionGradeReport(500)
    const bundle = buildGymTrainingRewardBundle({
      compiled,
      report,
      reportRef: 'report.gym.flywheel.candidate',
      candidateRef: 'candidate.gym.flywheel.shadow_01',
      candidateHash:
        'sha256:8000000000000000000000000000000000000000000000000000000000000001',
    })

    expect(bundle.decisionGrade).toBe(true)
    expect(bundle.consumers).toEqual(['gepa', 'trinity', 'conductor'])
    expect(bundle.psionicImportRef).toContain('psionic.import.gym_reward')
    expect(bundle.rows).toHaveLength(1)
    expect(bundle.rows[0]).toMatchObject({
      acceptedOutcomes: 5,
      attemptedVerifications: 5,
      consumers: ['gepa', 'trinity', 'conductor'],
      costPerAcceptedOutcomeMsat: 500,
      lane: 'khala',
      scalarReward: 9980,
      verificationRateBps: 10_000,
      workload: 'opencode-coding-task',
    })
  })

  test('builds Khala served-token inputs with internal Gym attribution', () => {
    const { runSet } = decisionGradeReport(500)
    const inputs = buildGymDogfoodServedTokensInputs({
      accountRef: 'agent:openagents-gym',
      runSet,
      demandClient: 'gym-opencode-runner',
    })

    expect(inputs).toHaveLength(5)
    expect(inputs.every(input => input.requestedModel === 'openagents/khala')).toBe(
      true,
    )
    expect(inputs[0]?.requestAttribution).toEqual({
      demandKind: 'internal',
      demandSource: 'openagents-gym',
      demandClient: 'gym-opencode-runner',
    })

    const body = buildServedTokensIngestBody({
      ...inputs[0]!,
      observedAt: '2026-06-25T00:00:00.000Z',
    })
    expect(body.safeMetadata).toMatchObject({
      demandKind: 'internal',
      demandSource: 'openagents-gym',
      demandClient: 'gym-opencode-runner',
    })
  })

  test('returns a cheaper decision-grade candidate to shadow and approval-gated runtime promotion', () => {
    const baseline = decisionGradeReport(900)
    const candidate = decisionGradeReport(450)
    const bundle = buildGymTrainingRewardBundle({
      compiled: candidate.compiled,
      report: candidate.report,
      reportRef: 'report.gym.flywheel.candidate',
      candidateRef: 'candidate.gym.flywheel.shadow_01',
      candidateHash:
        'sha256:8000000000000000000000000000000000000000000000000000000000000001',
    })
    const dogfoodInputs = buildGymDogfoodServedTokensInputs({
      accountRef: 'agent:openagents-gym',
      runSet: candidate.runSet,
    })

    expect(summarizeGymCostPerAcceptedOutcome(candidate.report)).toMatchObject({
      acceptedOutcomes: 5,
      costPerAcceptedOutcomeMsat: 450,
    })

    const shadow = evaluateGymFlywheelCandidate({
      compiled: candidate.compiled,
      baselineReport: baseline.report,
      candidateReport: candidate.report,
      baselineReportRef: 'report.gym.flywheel.heuristic',
      candidateReportRef: 'report.gym.flywheel.candidate',
      candidateRef: bundle.candidateRef,
      candidateHash: bundle.candidateHash,
      rewardBundleRefs: [bundle.bundleRef],
      psionicFrontierRefs: ['psionic_frontier.gym.flywheel.shadow_01'],
      dogfoodEventRefs: dogfoodInputs.map(input => input.requestId),
      requestedPromotion: 'runtime_promotion',
    })

    expect(shadow.candidateState).toBe('shadow')
    expect(shadow.runtimePromotionAllowed).toBe(false)
    expect(shadow.blockers).toContain(
      'blocker.gym.flywheel.runtime_promotion_requires_approval',
    )
    expect(shadow.reentryExperimentRef).toContain(
      'experiment.gym.head_to_head_reentry',
    )
    expect(shadow.costImprovementBps).toBe(5_000)

    const approved = evaluateGymFlywheelCandidate({
      compiled: candidate.compiled,
      baselineReport: baseline.report,
      candidateReport: candidate.report,
      baselineReportRef: 'report.gym.flywheel.heuristic',
      candidateReportRef: 'report.gym.flywheel.candidate',
      candidateRef: bundle.candidateRef,
      candidateHash: bundle.candidateHash,
      rewardBundleRefs: [bundle.bundleRef],
      psionicFrontierRefs: ['psionic_frontier.gym.flywheel.shadow_01'],
      dogfoodEventRefs: dogfoodInputs.map(input => input.requestId),
      requestedPromotion: 'runtime_promotion',
      runtimePromotionApprovalRef: 'approval.public.gym.flywheel.runtime_01',
    })

    expect(approved.candidateState).toBe('runtime_promotion_ready')
    expect(approved.runtimePromotionAllowed).toBe(true)
    expect(approved.runtimePromotionRef).toContain('runtime_promotion.gym')
    expect(approved.blockers).toEqual([])
  })

  test('blocks unsafe refs and non-improving candidates', () => {
    const baseline = decisionGradeReport(500)
    const candidate = decisionGradeReport(750)
    const bundle = buildGymTrainingRewardBundle({
      compiled: candidate.compiled,
      report: candidate.report,
      reportRef: 'report.gym.flywheel.candidate',
      candidateRef: 'candidate.gym.flywheel.shadow_01',
      candidateHash:
        'sha256:8000000000000000000000000000000000000000000000000000000000000001',
    })

    const blocked = evaluateGymFlywheelCandidate({
      compiled: candidate.compiled,
      baselineReport: baseline.report,
      candidateReport: candidate.report,
      baselineReportRef: 'report.gym.flywheel.heuristic',
      candidateReportRef: 'report.gym.flywheel.candidate',
      candidateRef: bundle.candidateRef,
      candidateHash: bundle.candidateHash,
      rewardBundleRefs: [bundle.bundleRef],
      psionicFrontierRefs: ['psionic_frontier.gym.flywheel.shadow_01'],
      dogfoodEventRefs: ['event.gym.dogfood.one'],
      requestedPromotion: 'shadow',
    })
    expect(blocked.candidateState).toBe('blocked')
    expect(blocked.blockers).toContain(
      'blocker.gym.flywheel.candidate_not_cheaper_than_heuristic',
    )

    expect(() =>
      buildGymTrainingRewardBundle({
        compiled: candidate.compiled,
        report: candidate.report,
        reportRef: 'raw_prompt.private',
        candidateRef: bundle.candidateRef,
        candidateHash: bundle.candidateHash,
      }),
    ).toThrow(GymFlywheelUnsafe)
  })
})
