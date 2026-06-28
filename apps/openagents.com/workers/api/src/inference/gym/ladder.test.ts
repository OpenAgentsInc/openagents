import { describe, expect, test } from 'vitest'

import {
  buildBenchmarkReport,
  type BenchmarkCell,
  type BenchmarkLaneSample,
  makeRealLaneSeam,
  runBenchmark,
} from '../benchmark'
import {
  OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  compileGymExperiment,
  runGymFixtureExperiment,
  BUNDLED_GYM_EXPERIMENT,
  type GymExperiment,
} from './experiment'
import {
  buildGymLadderLeaderboard,
  GYM_LADDER_RECURRING_CONFIG,
  GYM_LADDER_RUNGS,
  GymLadderLeaderboardSchemaVersion,
  gymLadderSnapshotRef,
} from './ladder'
import type { GymLeaderboardReportInput } from './leaderboard'
import { buildMirrorCodeRun } from './mirrorcode-contract'

const REALISTIC_SHAPE = {
  id: 'observed-opencode-ladder-run',
  inputTokens: 1500,
  outputTokens: 500,
  cacheablePrefixTokens: 700,
  concurrency: 1,
  provenance: 'realistic',
} as const

const LADDER_EXPERIMENT: GymExperiment = {
  ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  id: 'gym-opencode-ladder-test-v1',
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
    ownerApprovalRef: 'approval.public.gym.ladder.test',
  },
}

const sampleForCost =
  (
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
      taskRef: 'gym.ladder.opencode.edit-run-smoke.v1',
      configRef: `opencode.ladder.${cell.lane}.v1`,
      toolCallsAttempted: 2,
      toolCallsSucceeded: 2,
    },
  })

const decisionGradeReport = (costBasisMsat: number) => {
  const compiled = compileGymExperiment(LADDER_EXPERIMENT)
  const runSet = runBenchmark(
    compiled.matrixConfig,
    makeRealLaneSeam({
      armRealSweep: true,
      executor: sampleForCost(costBasisMsat),
    }),
  )
  return { compiled, report: buildBenchmarkReport(runSet) }
}

const khalaInput = (costBasisMsat: number): GymLeaderboardReportInput => {
  const built = decisionGradeReport(costBasisMsat)
  return {
    ...built,
    reportRef: 'report.gym.ladder.khala',
    receiptRef: 'receipt.gym.ladder.khala',
    candidateRef: `khala.ladder.${costBasisMsat}`,
  }
}

describe('Gym ladder recurring config', () => {
  test('defines the deliberate rungs with owner gates + internal demand tagging', () => {
    expect(GYM_LADDER_RUNGS.map(r => r.rung)).toEqual([
      'rung1',
      'rung2',
      'rung3',
      'rung4',
    ])
    expect(GYM_LADDER_RUNGS[0]?.opponentLanes).toContain('bigpickle')
    expect(GYM_LADDER_RUNGS[1]?.opponentLanes).toEqual([
      'gemini-free',
      'gpt-oss-20b',
      'gpt-oss-120b',
      'glm-52',
    ])
    expect(GYM_LADDER_RUNGS[2]?.opponentLanes).toEqual([
      'openai-gpt',
      'claude',
      'vertex-gemini',
    ])
    expect(GYM_LADDER_RUNGS[3]?.benchmarkFamily).toBe(
      'mirrorcode_public_bucket',
    )
    for (const rung of GYM_LADDER_RUNGS) {
      expect(rung.ownerGateRef).toMatch(/^gate\.owner\.gym\.ladder\./)
    }
    // #6298: gym traffic must be segmented internal / gym_ladder.
    expect(GYM_LADDER_RECURRING_CONFIG.demandKind).toBe('internal')
    expect(GYM_LADDER_RECURRING_CONFIG.demandSource).toBe('gym_ladder')
    expect(GYM_LADDER_RECURRING_CONFIG.publishPath).toBe(
      '/api/public/gym/leaderboard',
    )
  })

  test('snapshot ref is a public-safe stable ref', () => {
    const ref = gymLadderSnapshotRef(
      GYM_LADDER_RECURRING_CONFIG.ladderRef,
      '2026-06-26T00:00:00.000Z',
    )
    expect(ref.startsWith('snapshot.')).toBe(true)
    expect(ref).not.toMatch(/[^A-Za-z0-9_.-]/)
  })
})

describe('Gym ladder leaderboard projection', () => {
  test('empty input yields all rungs awaiting_owner with their gate refs', () => {
    const ladder = buildGymLadderLeaderboard([])
    expect(ladder.schemaVersion).toBe(GymLadderLeaderboardSchemaVersion)
    expect(ladder.decisionGradeRowCount).toBe(0)
    expect(ladder.rungs.map(r => r.state)).toEqual([
      'awaiting_owner',
      'awaiting_owner',
      'awaiting_owner',
      'awaiting_owner',
    ])
    const rung1 = ladder.rungs[0]!
    expect(rung1.entries).toEqual([])
    expect(rung1.blockerRefs).toContain(
      'blocker.gym.ladder.no_decision_grade_khala_row',
    )
    expect(rung1.blockerRefs).toContain(
      'gate.owner.gym.ladder.rung1.real_seam_with_bigpickle_model_id',
    )
    // bigpickle is a fixture_only lane today -> honest fixture-only blocker.
    expect(rung1.blockerRefs).toContain(
      'blocker.gym.ladder.opponent_lane_fixture_only.bigpickle',
    )
    const rung4 = ladder.rungs.find(r => r.rung === 'rung4')!
    expect(rung4.blockerRefs).toContain(
      'blocker.gym.ladder.mirrorcode_no_decision_grade_public_bucket_run',
    )
    expect(rung4.blockerRefs).toContain(
      'gate.owner.gym.ladder.rung4.mirrorcode_public_bucket_decision_grade',
    )
  })

  test('a decision-grade Khala-only run still leaves rungs awaiting opponents', () => {
    const ladder = buildGymLadderLeaderboard([khalaInput(400)])
    expect(ladder.decisionGradeRowCount).toBe(1)
    for (const rung of ladder.rungs) {
      expect(rung.state).toBe('awaiting_owner')
      // The Khala row exists, so that blocker should be gone.
      expect(rung.blockerRefs).not.toContain(
        'blocker.gym.ladder.no_decision_grade_khala_row',
      )
    }
  })

  test('publishes rung1 when Big Pickle is measured decision-grade alongside Khala', () => {
    const khala = decisionGradeReport(400)
    const bigPickle = decisionGradeReport(900)
    const ladder = buildGymLadderLeaderboard([
      {
        ...khala,
        reportRef: 'report.gym.ladder.khala',
        receiptRef: 'receipt.gym.ladder.khala',
        candidateRef: 'khala.ladder.run',
      },
      {
        ...bigPickle,
        reportRef: 'report.gym.ladder.bigpickle',
        receiptRef: 'receipt.gym.ladder.bigpickle',
        candidateRef: 'bigpickle.ladder.run',
      },
    ])
    const rung1 = ladder.rungs.find(r => r.rung === 'rung1')!
    expect(rung1.state).toBe('published')
    expect(rung1.blockerRefs).toEqual([])
    expect(rung1.entries.map(e => e.lane)).toEqual(['khala', 'bigpickle'])
    // Khala is cheaper -> ranked first within the rung.
    expect(rung1.entries[0]?.lane).toBe('khala')
    expect(rung1.entries[0]?.costPerAcceptedOutcomeMsat).toBe(400)
    expect(rung1.entries[1]?.costPerAcceptedOutcomeMsat).toBe(900)
    // Rungs 2 and 3 (no measured opponents) stay awaiting_owner.
    expect(ladder.rungs.find(r => r.rung === 'rung2')?.state).toBe(
      'awaiting_owner',
    )
    expect(ladder.rungs.find(r => r.rung === 'rung3')?.state).toBe(
      'awaiting_owner',
    )
  })

  test('publishes rung4 from decision-grade MirrorCode public-bucket runs with exact token rows', () => {
    const mirrorCodeRun = buildMirrorCodeRun({
      runId: 'mc-s-cal-python-0001',
      model: 'openagents/khala',
      taskId: 'cal',
      bucket: 'S',
      language: 'python',
      status: 'passed',
      passRate: 0.73,
      tokens: { total: 1_000_000_001 },
      exactTokenUsageEventRefs: [
        'token_usage_event.gym_mirrorcode.cal.0001',
        'token_usage_event.gym_mirrorcode.cal.0002',
      ],
      startedAt: '2026-06-27T00:00:00.000Z',
      finishedAt: '2026-06-27T02:00:00.000Z',
      summary: 'Decision-grade public S-bucket MirrorCode cal run.',
      grade: 'decision_grade',
    })
    const ladder = buildGymLadderLeaderboard([], undefined, [mirrorCodeRun])
    const rung4 = ladder.rungs.find(r => r.rung === 'rung4')!
    expect(rung4.state).toBe('published')
    expect(rung4.benchmarkFamily).toBe('mirrorcode_public_bucket')
    expect(rung4.passRateBps).toBe(7300)
    expect(rung4.tokensTotal).toBe(1_000_000_001)
    expect(rung4.exactTokenUsageEventRefs).toEqual([
      'token_usage_event.gym_mirrorcode.cal.0001',
      'token_usage_event.gym_mirrorcode.cal.0002',
    ])
    expect(rung4.tokenAttributionProofRefs).toEqual([
      'proof.gym.mirrorcode.exact_token_rows.mc-s-cal-python-0001',
    ])
    expect(rung4.entries[0]?.passRateBps).toBe(7300)
    expect(rung4.entries[0]?.benchmarkFamily).toBe(
      'mirrorcode_public_bucket',
    )
    expect(ladder.caveatRefs).toContain(
      'caveat.public.gym.ladder.mirrorcode_exact_token_rows_required',
    )
    expect(ladder.caveatRefs).toContain(
      'caveat.public.gym.ladder.mirrorcode_traffic_tagged_internal_gym_mirrorcode',
    )
  })

  test('never publishes fixture / non-decision-grade reports as a rung measurement', () => {
    const fixture = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    const ladder = buildGymLadderLeaderboard([
      {
        ...fixture,
        reportRef: 'report.gym.ladder.fixture',
        receiptRef: 'receipt.gym.ladder.fixture',
        candidateRef: 'bigpickle.fixture.run',
      },
    ])
    expect(ladder.decisionGradeRowCount).toBe(0)
    expect(ladder.excludedReports).toContainEqual({
      reportRef: 'report.gym.ladder.fixture',
      reason: 'not_decision_grade',
    })
    for (const rung of ladder.rungs) {
      expect(rung.state).toBe('awaiting_owner')
    }
  })

  test('carries the honest ladder caveats and no illustrative leakage', () => {
    const ladder = buildGymLadderLeaderboard([khalaInput(400)])
    expect(ladder.caveatRefs).toContain(
      'caveat.public.gym.ladder.fixture_or_synthetic_never_published',
    )
    expect(ladder.caveatRefs).toContain(
      'caveat.public.gym.ladder.no_beats_frontier_claim_from_single_run',
    )
    expect(JSON.stringify(ladder)).not.toContain('illustrativeNotice')
  })
})
