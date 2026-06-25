import { describe, expect, test } from 'vitest'

import {
  OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG,
  SAMPLE_DECISION_SUITE_CONFIG,
  TINY_TEST_CONFIG,
} from './fixtures'
import { makeFixtureLaneSeam, makeRealLaneSeam } from './lane-seam'
import {
  buildBenchmarkReport,
  checkReportPublicSafety,
  mean,
  percentile,
} from './report'
import { runBenchmark } from './runner'

describe('percentile + mean helpers (book Ch.1 §1.4.1)', () => {
  test('nearest-rank percentiles over a known distribution', () => {
    const values = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
    expect(percentile(values, 50)).toBe(50)
    expect(percentile(values, 90)).toBe(90)
    expect(percentile(values, 99)).toBe(100)
  })

  test('empty input is honest null, never a fabricated 0', () => {
    expect(percentile([], 50)).toBeNull()
    expect(mean([])).toBeNull()
  })

  test('single sample returns itself', () => {
    expect(percentile([42], 50)).toBe(42)
    expect(percentile([42], 99)).toBe(42)
  })

  test('mean is the arithmetic average', () => {
    expect(mean([10, 20, 30])).toBe(20)
  })
})

describe('benchmark report — aggregation', () => {
  test('groups are per (lane × workload), executed-only metrics, future lanes skipped', () => {
    const report = buildBenchmarkReport(
      runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam()),
    )
    // Two lanes × one workload, but pylon (future) is skipped → 2 groups exist
    // (the future-lane group has only skipped samples).
    const fireworks = report.groups.find(g => g.lane === 'fireworks')!
    const pylon = report.groups.find(g => g.lane === 'pylon-whole-small')!
    expect(fireworks.executedSamples).toBe(4)
    expect(fireworks.skippedSamples).toBe(0)
    expect(pylon.executedSamples).toBe(0)
    expect(pylon.skippedSamples).toBe(1)
    expect(pylon.laneAvailability).toBe('not_yet_available')
  })

  test('verification rate = accepted / attempted (all artifact-gen samples pass in fixture)', () => {
    const report = buildBenchmarkReport(
      runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam()),
    )
    const fireworks = report.groups.find(g => g.lane === 'fireworks')!
    expect(fireworks.attemptedVerifications).toBe(4)
    expect(fireworks.acceptedOutcomes).toBe(4)
    expect(fireworks.verificationRate).toBe(1)
  })

  test('cost-per-accepted-outcome = total cost basis / accepted outcomes', () => {
    const report = buildBenchmarkReport(
      runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam()),
    )
    const fireworks = report.groups.find(g => g.lane === 'fireworks')!
    // Verify the relationship directly (the value is fixture-derived).
    expect(fireworks.costPerAcceptedOutcomeMsat).not.toBeNull()
    expect(fireworks.costPerAcceptedOutcomeMsat).toBeCloseTo(
      fireworks.totalCostBasisMsat / fireworks.acceptedOutcomes,
      6,
    )
  })

  test('a group with no accepted outcome has null cost-per-outcome, not a fake 0', () => {
    // Build a config whose only executable workload is chat (never accepted).
    const chatOnly = {
      ...TINY_TEST_CONFIG,
      id: 'chat-only-v1',
      workloads: ['chat'] as const,
      targets: [{ lane: 'fireworks', engine: 'provider-native' } as const],
    }
    const report = buildBenchmarkReport(
      runBenchmark(chatOnly, makeFixtureLaneSeam()),
    )
    const fireworks = report.groups.find(g => g.lane === 'fireworks')!
    expect(fireworks.acceptedOutcomes).toBe(0)
    expect(fireworks.verificationRate).toBeNull()
    expect(fireworks.costPerAcceptedOutcomeMsat).toBeNull()
    // Cost basis is still measured even with no accepted outcome (the finding:
    // money spent, nothing accepted).
    expect(fireworks.totalCostBasisMsat).toBeGreaterThan(0)
  })

  test('cache hit rate reflects cached input tokens / prompt tokens', () => {
    const report = buildBenchmarkReport(
      runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam()),
    )
    const fireworks = report.groups.find(g => g.lane === 'fireworks')!
    // 500 cacheable prefix × 0.8 hit = 400 cached, over a 1000-token prompt = 0.4.
    expect(fireworks.cacheHitRate).toBeCloseTo(0.4, 6)
  })

  test('OpenCode fixture compares Khala vs BigPickle with tool-call success', () => {
    const report = buildBenchmarkReport(
      runBenchmark(
        OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG,
        makeFixtureLaneSeam(),
      ),
    )
    const khala = report.groups.find(
      g => g.lane === 'khala' && g.workload === 'opencode-coding-task',
    )!
    const bigpickle = report.groups.find(
      g => g.lane === 'bigpickle' && g.workload === 'opencode-coding-task',
    )!

    expect(report.decisionGrade).toBe(false)
    expect(khala.executedSamples).toBe(5)
    expect(bigpickle.executedSamples).toBe(5)
    expect(khala.verificationRate).toBe(1)
    expect(bigpickle.verificationRate).toBe(0)
    expect(khala.toolCallSuccessRate).toBe(1)
    expect(bigpickle.toolCallSuccessRate).toBeCloseTo(2 / 3, 6)
    expect(khala.costPerAcceptedOutcomeMsat).not.toBeNull()
    expect(bigpickle.costPerAcceptedOutcomeMsat).toBeNull()
    expect(checkReportPublicSafety(report).safe).toBe(true)
  })

  test('latency percentiles populate for streaming, sample count matches', () => {
    const report = buildBenchmarkReport(
      runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam()),
    )
    const fireworks = report.groups.find(g => g.lane === 'fireworks')!
    expect(fireworks.ttftMs.sampleCount).toBe(4)
    expect(fireworks.ttftMs.p50).not.toBeNull()
    expect(fireworks.ttftMs.p99).not.toBeNull()
    expect(fireworks.perceivedTps.p50).not.toBeNull()
  })

  test('report ordering is deterministic and byte-stable', () => {
    const a = buildBenchmarkReport(
      runBenchmark(SAMPLE_DECISION_SUITE_CONFIG, makeFixtureLaneSeam()),
    )
    const b = buildBenchmarkReport(
      runBenchmark(SAMPLE_DECISION_SUITE_CONFIG, makeFixtureLaneSeam()),
    )
    expect(JSON.stringify(a)).toBe(JSON.stringify(b))
  })
})

describe('benchmark report — honesty + public-safety', () => {
  test('a fixture-lane report is NOT decision-grade and carries the illustrative notice', () => {
    const report = buildBenchmarkReport(
      runBenchmark(SAMPLE_DECISION_SUITE_CONFIG, makeFixtureLaneSeam()),
    )
    expect(report.seamId).toBe('fixture')
    expect(report.decisionGrade).toBe(false)
    expect(report.illustrativeNotice).toContain('ILLUSTRATIVE ONLY')
    expect(report.illustrativeNotice).toContain('REALISTIC')
  })

  test('synthetic-only groups are flagged', () => {
    const report = buildBenchmarkReport(
      runBenchmark(SAMPLE_DECISION_SUITE_CONFIG, makeFixtureLaneSeam()),
    )
    expect(report.groups.every(g => g.syntheticOnly)).toBe(true)
  })

  test('even an owner-armed real seam over SYNTHETIC shapes is not decision-grade', () => {
    // A real (canSpend) seam, but the shapes are still synthetic → the synthetic
    // guard keeps the report out of decision-grade. Real numbers need real traffic.
    const armedRealSeam = makeRealLaneSeam({
      armRealSweep: true,
      executor: (cell, sampleIndex) => ({
        promptTokens: cell.shape.inputTokens,
        completionTokens: cell.shape.outputTokens,
        totalTokens: cell.shape.inputTokens + cell.shape.outputTokens,
        cachedInputTokens: 0,
        ttftMs: 100 + sampleIndex,
        totalWallClockMs: 500,
        generationWallClockMs: 400,
        providerTimeMs: 480,
        gatewayOverheadMs: 20,
        verificationClass: 'test_passed',
        executedVerdict: 'passed',
        scalarReward: 1,
        verifierTimeMs: 100,
        costBasisMsat: 100,
        region: cell.lane,
      }),
    })
    const report = buildBenchmarkReport(
      runBenchmark(TINY_TEST_CONFIG, armedRealSeam),
    )
    expect(report.seamId).toBe('real')
    // Synthetic shapes → never decision-grade, even with a real seam.
    expect(report.decisionGrade).toBe(false)
  })

  test('the report is public-safe — no raw prompt/account/price/secret keys', () => {
    const report = buildBenchmarkReport(
      runBenchmark(SAMPLE_DECISION_SUITE_CONFIG, makeFixtureLaneSeam()),
    )
    const safety = checkReportPublicSafety(report)
    expect(safety.violations).toEqual([])
    expect(safety.safe).toBe(true)
  })

  test('the public-safety check actually trips on a forbidden key (guard is real)', () => {
    const report = buildBenchmarkReport(
      runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam()),
    )
    // Inject a forbidden field to prove the tripwire fires.
    const tampered = { ...report, leakedPrompt: 'do the thing' } as never
    const safety = checkReportPublicSafety(tampered)
    expect(safety.safe).toBe(false)
    expect(safety.violations).toContain('prompt')
  })
})
