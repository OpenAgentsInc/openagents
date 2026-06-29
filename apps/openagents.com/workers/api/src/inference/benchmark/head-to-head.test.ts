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
} from '../gym/experiment'
import type { GymLeaderboardReportInput } from '../gym/leaderboard'
import {
  buildKhalaHeadToHead,
  KHALA_HEAD_TO_HEAD_COMPARATORS,
  KHALA_HEAD_TO_HEAD_RECURRING_CONFIG,
  KhalaHeadToHeadSchemaVersion,
  khalaHeadToHeadSnapshotRef,
} from './head-to-head'

const REALISTIC_SHAPE = {
  id: 'observed-head-to-head-run',
  inputTokens: 1500,
  outputTokens: 500,
  cacheablePrefixTokens: 700,
  concurrency: 1,
  provenance: 'realistic',
} as const

const H2H_EXPERIMENT: GymExperiment = {
  ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  id: 'khala-head-to-head-test-v1',
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
    ownerApprovalRef: 'approval.public.khala.head_to_head.test',
  },
}

// Build a decision-grade run with a controllable cost-per-accepted-outcome AND a
// controllable solve rate (one passing + one failing sample => 50% when
// `passRate` is 0.5). The runner runs `samplesPerCell` samples; we drive the
// verdict by passing/failing on the sample index.
const sampleFor =
  (
    costBasisMsat: number,
    passSample: (sampleIndex: number) => boolean,
  ): ((cell: BenchmarkCell, sampleIndex: number) => BenchmarkLaneSample) =>
  (cell, sampleIndex) => {
    const passed = passSample(sampleIndex)
    return {
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
      executedVerdict: passed ? 'passed' : 'failed',
      scalarReward: passed ? 1 : 0,
      verifierTimeMs: 850,
      // Cost basis is recorded on every sample; cost-per-accepted-outcome is
      // total cost / accepted, so it scales with the per-sample cost.
      costBasisMsat,
      region: 'openagents',
      clientSurface: {
        client: 'opencode',
        taskRef: 'khala.head_to_head.opencode.edit-run-smoke.v1',
        configRef: `opencode.head_to_head.${cell.lane}.v1`,
        toolCallsAttempted: 2,
        toolCallsSucceeded: passed ? 2 : 1,
      },
    }
  }

const decisionGradeReport = (
  costBasisMsat: number,
  passSample: (sampleIndex: number) => boolean = () => true,
) => {
  const compiled = compileGymExperiment(H2H_EXPERIMENT)
  const runSet = runBenchmark(
    compiled.matrixConfig,
    makeRealLaneSeam({
      armRealSweep: true,
      executor: sampleFor(costBasisMsat, passSample),
    }),
  )
  return { compiled, report: buildBenchmarkReport(runSet) }
}

const inputFor = (
  candidateRef: string,
  reportRef: string,
  costBasisMsat: number,
  passSample: (sampleIndex: number) => boolean = () => true,
): GymLeaderboardReportInput => ({
  ...decisionGradeReport(costBasisMsat, passSample),
  reportRef,
  receiptRef: `receipt.${reportRef}`,
  candidateRef,
})

describe('Khala head-to-head recurring config', () => {
  test('defines the developer-default comparator set with owner gates + internal demand tagging', () => {
    const lanes = KHALA_HEAD_TO_HEAD_COMPARATORS.map(c => c.lane)
    expect(lanes).toContain('bigpickle')
    expect(lanes).toContain('gemini-free')
    expect(lanes).toContain('openai-gpt')
    expect(lanes).toContain('claude')
    // khala is never a comparator (it is the protagonist).
    expect(lanes).not.toContain('khala')
    for (const comparator of KHALA_HEAD_TO_HEAD_COMPARATORS) {
      expect(comparator.ownerGateRef).toMatch(/^gate\.owner\.khala\.head_to_head\./)
    }
    // #6298: head-to-head traffic must be segmented internal / head_to_head.
    expect(KHALA_HEAD_TO_HEAD_RECURRING_CONFIG.demandKind).toBe('internal')
    expect(KHALA_HEAD_TO_HEAD_RECURRING_CONFIG.demandSource).toBe('head_to_head')
    expect(KHALA_HEAD_TO_HEAD_RECURRING_CONFIG.publishPath).toBe(
      '/api/public/khala/head-to-head',
    )
  })

  test('snapshot ref is a public-safe stable ref', () => {
    const ref = khalaHeadToHeadSnapshotRef(
      KHALA_HEAD_TO_HEAD_RECURRING_CONFIG.headToHeadRef,
      '2026-06-26T00:00:00.000Z',
    )
    expect(ref.startsWith('snapshot.')).toBe(true)
    expect(ref).not.toMatch(/[^A-Za-z0-9_.-]/)
  })
})

describe('Khala head-to-head projection', () => {
  test('empty input yields all matchups awaiting_owner with their gate refs', () => {
    const h2h = buildKhalaHeadToHead([])
    expect(h2h.schemaVersion).toBe(KhalaHeadToHeadSchemaVersion)
    expect(h2h.decisionGradeRowCount).toBe(0)
    expect(h2h.khala).toBeNull()
    expect(h2h.matchups.every(m => m.state === 'awaiting_owner')).toBe(true)
    const bigPickle = h2h.matchups.find(m => m.lane === 'bigpickle')!
    expect(bigPickle.khala).toBeNull()
    expect(bigPickle.comparator).toBeNull()
    expect(bigPickle.verdict).toBeNull()
    expect(bigPickle.blockerRefs).toContain(
      'blocker.khala.head_to_head.no_decision_grade_khala_row',
    )
    expect(bigPickle.blockerRefs).toContain(
      'gate.owner.khala.head_to_head.bigpickle.real_seam_with_model_id',
    )
    // bigpickle is fixture_only today -> honest fixture-only blocker.
    expect(bigPickle.blockerRefs).toContain(
      'blocker.khala.head_to_head.comparator_lane_fixture_only.bigpickle',
    )
  })

  test('a decision-grade Khala-only run sets the protagonist but leaves matchups awaiting comparators', () => {
    const h2h = buildKhalaHeadToHead([
      inputFor('khala.head_to_head.run', 'report.khala.head_to_head.khala', 400),
    ])
    expect(h2h.decisionGradeRowCount).toBe(1)
    expect(h2h.khala?.costPerAcceptedOutcomeMsat).toBe(400)
    expect(h2h.khala?.inputTokens).toBe(7_500)
    expect(h2h.khala?.outputTokens).toBe(2_500)
    expect(h2h.khala?.totalTokens).toBe(10_000)
    expect(h2h.khala?.meanWallClockMs).toBe(3_302)
    for (const matchup of h2h.matchups) {
      expect(matchup.state).toBe('awaiting_owner')
      expect(matchup.blockerRefs).not.toContain(
        'blocker.khala.head_to_head.no_decision_grade_khala_row',
      )
    }
  })

  test('publishes a matchup with a measured comparator and scores both axes (khala_wins_both)', () => {
    const h2h = buildKhalaHeadToHead([
      // Khala: cheaper (400) and higher solve (all pass).
      inputFor('khala.head_to_head.run', 'report.khala.head_to_head.khala', 400),
      // Big Pickle comparator: pricier (900) and lower solve (every other sample fails).
      inputFor(
        'bigpickle.head_to_head.run',
        'report.khala.head_to_head.bigpickle',
        900,
        sampleIndex => sampleIndex % 2 === 0,
      ),
    ])
    const bigPickle = h2h.matchups.find(m => m.lane === 'bigpickle')!
    expect(bigPickle.state).toBe('published')
    expect(bigPickle.blockerRefs).toEqual([])
    expect(bigPickle.khala?.lane).toBe('khala')
    expect(bigPickle.comparator?.lane).toBe('bigpickle')
    expect(bigPickle.verdict).toBe('khala_wins_both')
    // Cost delta = comparator - khala (positive => Khala cheaper per accepted
    // outcome). Big Pickle is pricier AND fails half its samples, so its
    // cost-per-accepted-outcome is strictly above Khala's.
    expect(bigPickle.costPerAcceptedOutcomeDeltaMsat).not.toBeNull()
    expect(bigPickle.costPerAcceptedOutcomeDeltaMsat!).toBeGreaterThan(0)
    expect(bigPickle.khala!.costPerAcceptedOutcomeMsat).toBe(400)
    expect(bigPickle.comparator!.costPerAcceptedOutcomeMsat).toBeGreaterThan(400)
    expect(bigPickle.khala!.totalTokens).toBe(10_000)
    expect(bigPickle.comparator!.totalTokens).toBe(10_000)
    expect(bigPickle.khala!.meanWallClockMs).toBe(3_302)
    expect(bigPickle.comparator!.meanWallClockMs).toBe(3_302)
    // Khala solved more -> positive solve-rate delta.
    expect(bigPickle.solveRateDeltaBps).not.toBeNull()
    expect(bigPickle.solveRateDeltaBps!).toBeGreaterThan(0)
    // Other matchups stay awaiting_owner.
    expect(h2h.matchups.find(m => m.lane === 'claude')?.state).toBe(
      'awaiting_owner',
    )
  })

  test('scores comparator_ahead when the comparator is cheaper and solves more', () => {
    const h2h = buildKhalaHeadToHead([
      // Khala: pricier (1000) and lower solve (every other fails).
      inputFor(
        'khala.head_to_head.run',
        'report.khala.head_to_head.khala',
        1000,
        sampleIndex => sampleIndex % 2 === 0,
      ),
      // Claude comparator: cheaper (300) and all pass.
      inputFor(
        'claude.head_to_head.run',
        'report.khala.head_to_head.claude',
        300,
      ),
    ])
    const claude = h2h.matchups.find(m => m.lane === 'claude')!
    expect(claude.state).toBe('published')
    expect(claude.verdict).toBe('comparator_ahead')
    // Cost delta = comparator - khala (negative => Khala pricier per accepted).
    // Claude is cheaper AND all-pass; Khala is pricier AND fails half its
    // samples, so its cost-per-accepted-outcome is strictly above Claude's.
    expect(claude.costPerAcceptedOutcomeDeltaMsat).not.toBeNull()
    expect(claude.costPerAcceptedOutcomeDeltaMsat!).toBeLessThan(0)
    expect(claude.comparator!.costPerAcceptedOutcomeMsat).toBe(300)
    expect(claude.khala!.costPerAcceptedOutcomeMsat).toBeGreaterThan(300)
  })

  test('never publishes fixture / non-decision-grade reports as a matchup measurement', () => {
    const fixture = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    const h2h = buildKhalaHeadToHead([
      {
        ...fixture,
        reportRef: 'report.khala.head_to_head.fixture',
        receiptRef: 'receipt.khala.head_to_head.fixture',
        candidateRef: 'bigpickle.fixture.run',
      },
    ])
    expect(h2h.decisionGradeRowCount).toBe(0)
    expect(h2h.excludedReports).toContainEqual({
      reportRef: 'report.khala.head_to_head.fixture',
      reason: 'not_decision_grade',
    })
    expect(h2h.matchups.every(m => m.state === 'awaiting_owner')).toBe(true)
  })

  test('carries the honest head-to-head caveats and the two-axis framing', () => {
    const h2h = buildKhalaHeadToHead([
      inputFor('khala.head_to_head.run', 'report.khala.head_to_head.khala', 400),
    ])
    expect(h2h.caveatRefs).toContain(
      'caveat.public.khala.head_to_head.fixture_or_synthetic_never_published',
    )
    expect(h2h.caveatRefs).toContain(
      'caveat.public.khala.head_to_head.scored_on_solve_rate_and_cost_per_accepted_outcome',
    )
    expect(h2h.caveatRefs).toContain(
      'caveat.public.khala.head_to_head.no_beats_frontier_claim_from_single_run',
    )
    // No illustrative/decision-grade-internal leakage in the published shape.
    expect(JSON.stringify(h2h)).not.toContain('illustrativeNotice')
  })
})
