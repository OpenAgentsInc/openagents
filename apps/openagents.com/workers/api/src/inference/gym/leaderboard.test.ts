import { describe, expect, test } from 'vitest'
import { Schema as S } from 'effect'

import {
  buildBenchmarkReport,
  type BenchmarkCell,
  type BenchmarkLaneSample,
  makeRealLaneSeam,
  runBenchmark,
} from '../benchmark'
import {
  BUNDLED_GYM_EXPERIMENT,
  OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  compileGymExperiment,
  runGymFixtureExperiment,
  type GymExperiment,
} from './experiment'
import {
  AgentClEval,
  AgentClEvalSchemaVersion,
  buildGymLeaderboardProjection,
  GymLeaderboardUnsafe,
  modelGymModuleAuthorSplit,
} from './leaderboard'

const REALISTIC_SHAPE = {
  id: 'observed-opencode-leaderboard-run',
  inputTokens: 1500,
  outputTokens: 500,
  cacheablePrefixTokens: 700,
  concurrency: 1,
  provenance: 'realistic',
} as const

const LEADERBOARD_EXPERIMENT: GymExperiment = {
  ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  id: 'gym-opencode-leaderboard-test-v1',
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
    ownerApprovalRef: 'approval.public.gym.leaderboard.test',
  },
}

const sampleForCost = (
  costBasisMsat: number,
): ((cell: BenchmarkCell, sampleIndex: number) => BenchmarkLaneSample) =>
  (cell, sampleIndex) => ({
    promptTokens: cell.shape.inputTokens,
    completionTokens: cell.shape.outputTokens,
    totalTokens: cell.shape.inputTokens + cell.shape.outputTokens,
    cachedInputTokens: Math.floor(cell.shape.cacheablePrefixTokens * 0.7),
    ttftMs: 210 + sampleIndex,
    totalWallClockMs: 3_300 + sampleIndex,
    generationWallClockMs: 3_000 + sampleIndex,
    providerTimeMs: 3_210 + sampleIndex,
    gatewayOverheadMs: 90,
    verificationClass: 'test_passed',
    executedVerdict: 'passed',
    scalarReward: 1,
    verifierTimeMs: 850,
    costBasisMsat,
    region: 'openagents',
    clientSurface: {
      client: 'opencode',
      taskRef: 'gym.leaderboard.opencode.edit-run-smoke.v1',
      configRef: `opencode.leaderboard.${cell.lane}.v1`,
      toolCallsAttempted: 2,
      toolCallsSucceeded: 2,
    },
  })

const decisionGradeReport = (costBasisMsat: number) => {
  const compiled = compileGymExperiment(LEADERBOARD_EXPERIMENT)
  const runSet = runBenchmark(
    compiled.matrixConfig,
    makeRealLaneSeam({
      armRealSweep: true,
      executor: sampleForCost(costBasisMsat),
    }),
  )
  return { compiled, report: buildBenchmarkReport(runSet) }
}

const AGENTCL_EVAL = {
  schemaVersion: AgentClEvalSchemaVersion,
  evalRef: 'eval.public.gym.agentcl.router_memory.v0',
  baseline: {
    evalRef: 'eval.public.gym.agentcl.router_memory.baseline',
    accuracyBps: 5_800,
    sampleCount: 120,
  },
  firstPass: {
    evalRef: 'eval.public.gym.agentcl.router_memory.first_pass',
    accuracyBps: 6_300,
    sampleCount: 120,
  },
  frozenSecondPass: {
    evalRef: 'eval.public.gym.agentcl.router_memory.frozen_second_pass',
    accuracyBps: 6_100,
    sampleCount: 120,
  },
  heldOut: {
    evalRef: 'eval.public.gym.agentcl.router_memory.held_out',
    accuracyBps: 5_900,
    sampleCount: 120,
  },
  plasticityGain: {
    gainBps: 500,
    evidenceRef: 'evidence.public.gym.agentcl.router_memory.pg',
  },
  stabilityGain: {
    gainBps: 300,
    evidenceRef: 'evidence.public.gym.agentcl.router_memory.sg',
  },
  generalizationGain: {
    gainBps: 100,
    evidenceRef: 'evidence.public.gym.agentcl.router_memory.gg',
  },
} as const

describe('Gym public leaderboard projection', () => {
  test('ranks only public-safe decision-grade reports by cost per accepted outcome', () => {
    const fast = decisionGradeReport(400)
    const slow = decisionGradeReport(900)
    const fixture = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)

    const projection = buildGymLeaderboardProjection([
      {
        ...slow,
        reportRef: 'report.gym.leaderboard.slow',
        receiptRef: 'receipt.gym.leaderboard.slow',
        candidateRef: 'candidate.gym.leaderboard.slow',
      },
      {
        ...fixture,
        reportRef: 'report.gym.leaderboard.fixture',
        receiptRef: 'receipt.gym.leaderboard.fixture',
        candidateRef: 'candidate.gym.leaderboard.fixture',
      },
      {
        ...fast,
        reportRef: 'report.gym.leaderboard.fast',
        receiptRef: 'receipt.gym.leaderboard.fast',
        candidateRef: 'candidate.gym.leaderboard.fast',
      },
    ])

    expect(projection.rowCount).toBe(2)
    expect(projection.rows.map(row => row.reportRef)).toEqual([
      'report.gym.leaderboard.fast',
      'report.gym.leaderboard.slow',
    ])
    expect(projection.rows.map(row => row.rank)).toEqual([1, 2])
    expect(projection.rows[0]).toMatchObject({
      acceptedOutcomes: 5,
      costPerAcceptedOutcomeMsat: 400,
      verificationRateBps: 10_000,
    })
    expect(projection.excludedReports).toEqual([
      {
        reportRef: 'report.gym.leaderboard.fixture',
        reason: 'not_decision_grade',
      },
    ])
    expect(JSON.stringify(projection)).not.toContain('illustrativeNotice')
    expect(projection.caveatRefs).toContain(
      'caveat.public.gym.leaderboard.decision_grade_only',
    )
  })

  test('rejects unsafe leaderboard refs before projection', () => {
    const report = decisionGradeReport(400)

    expect(() =>
      buildGymLeaderboardProjection([
        {
          ...report,
          reportRef: 'raw_prompt.private',
          receiptRef: 'receipt.gym.leaderboard.safe',
          candidateRef: 'candidate.gym.leaderboard.safe',
        },
      ]),
    ).toThrow(GymLeaderboardUnsafe)
  })

  test('requires separate AgentCL PG, SG, and GG evidence for learning claims', () => {
    const report = decisionGradeReport(400)

    const projection = buildGymLeaderboardProjection([
      {
        ...report,
        reportRef: 'report.gym.leaderboard.memory_claim_missing_agentcl',
        receiptRef: 'receipt.gym.leaderboard.memory_claim_missing_agentcl',
        candidateRef: 'candidate.gym.leaderboard.memory_claim_missing_agentcl',
        learningClaim: {
          claimRef: 'claim.public.gym.memory_improves.router_memory',
          kind: 'memory_improves',
        },
      },
      {
        ...report,
        reportRef: 'report.gym.leaderboard.memory_claim_agentcl',
        receiptRef: 'receipt.gym.leaderboard.memory_claim_agentcl',
        candidateRef: 'candidate.gym.leaderboard.memory_claim_agentcl',
        learningClaim: {
          claimRef: 'claim.public.gym.memory_improves.router_memory.agentcl',
          kind: 'continually_learns',
        },
        agentClEval: S.decodeUnknownSync(AgentClEval)(AGENTCL_EVAL),
      },
    ])

    expect(projection.rows.map(row => row.reportRef)).toEqual([
      'report.gym.leaderboard.memory_claim_agentcl',
    ])
    expect(projection.excludedReports).toContainEqual({
      reportRef: 'report.gym.leaderboard.memory_claim_missing_agentcl',
      reason: 'agentcl_evidence_missing',
    })
  })

  test('rejects single-number memory-improvement evidence for AgentCL claims', () => {
    const report = decisionGradeReport(400)
    const singleNumberEval = {
      schemaVersion: AgentClEvalSchemaVersion,
      evalRef: 'eval.public.gym.agentcl.single_number',
      baseline: AGENTCL_EVAL.baseline,
      firstPass: AGENTCL_EVAL.firstPass,
      frozenSecondPass: AGENTCL_EVAL.frozenSecondPass,
      heldOut: AGENTCL_EVAL.heldOut,
      plasticityGain: AGENTCL_EVAL.plasticityGain,
      stabilityGain: AGENTCL_EVAL.stabilityGain,
    }

    expect(() =>
      buildGymLeaderboardProjection([
        {
          ...report,
          reportRef: 'report.gym.leaderboard.single_number_memory_claim',
          receiptRef: 'receipt.gym.leaderboard.single_number_memory_claim',
          candidateRef: 'candidate.gym.leaderboard.single_number_memory_claim',
          learningClaim: {
            claimRef: 'claim.public.gym.memory_improves.single_number',
            kind: 'memory_improves',
          },
          agentClEval: S.decodeUnknownSync(AgentClEval)(singleNumberEval),
        },
      ]),
    ).toThrow()
  })
})

describe('Gym module author split projection', () => {
  test('models owner-armed author shares from composition evidence without settlement authority', () => {
    const { report } = decisionGradeReport(400)
    const projection = modelGymModuleAuthorSplit({
      report,
      reportRef: 'report.gym.leaderboard.fast',
      ownerArmed: true,
      grossRevenueMsat: 100_000,
      contributorShareMsat: 20_000,
      contributions: [
        {
          moduleRef: 'module.public.gym.router_memory.v1',
          authorRef: 'author.public.alice',
          programSignatureRef: 'program_signature.public.khala_router.v1',
          evidenceRef: 'evidence.public.gym.trace_decomposition.router_memory',
          weightBps: 7_000,
        },
        {
          moduleRef: 'module.public.gym.tool_planner.v1',
          authorRef: 'author.public.bob',
          programSignatureRef: 'program_signature.public.khala_tool_plan.v1',
          evidenceRef: 'evidence.public.gym.trace_decomposition.tool_planner',
          weightBps: 3_000,
        },
      ],
    })

    expect(projection.state).toBe('modeled')
    expect(projection.blockerRefs).toEqual([])
    expect(projection.shares.map(share => share.shareMsat)).toEqual([
      14_000,
      6_000,
    ])
    expect(projection.authorPayoutClaimAllowed).toBe(false)
    expect(projection.marketplaceListingAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.caveatRefs).toContain(
      'caveat.public.gym.author_split.modeled_not_settled',
    )
  })

  test('blocks author split modeling without owner arming or complete evidence', () => {
    const { report } = decisionGradeReport(400)
    const projection = modelGymModuleAuthorSplit({
      report,
      reportRef: 'report.gym.leaderboard.fast',
      ownerArmed: false,
      grossRevenueMsat: 100_000,
      contributorShareMsat: 20_000,
      contributions: [
        {
          moduleRef: 'module.public.gym.router_memory.v1',
          authorRef: 'author.public.alice',
          programSignatureRef: 'program_signature.public.khala_router.v1',
          evidenceRef: 'evidence.public.gym.trace_decomposition.router_memory',
          weightBps: 6_000,
        },
      ],
    })

    expect(projection.state).toBe('blocked')
    expect(projection.shares).toEqual([])
    expect(projection.blockerRefs).toEqual([
      'blocker.gym.author_split.contribution_weights_must_sum_to_10000',
      'blocker.gym.author_split.owner_arming_missing',
    ])
  })
})
