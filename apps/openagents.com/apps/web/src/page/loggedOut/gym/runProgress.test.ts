import { describe, expect, test } from 'vitest'

import {
  GYM_RUN_PROGRESS_SCHEMA,
  type GymRunProgress,
  formatRunProgressCount,
  formatRunProgressDuration,
  formatRunProgressPercent,
  runPhaseLabel,
  runProgressVisualizationOptions,
} from './runProgress'

// A test-local progress object. This is NOT a shipped fixture: it only exercises
// the pure formatting helpers and the three-effect adapter. The page itself
// renders an honest empty state until a real run is ingested.
const partialRun: GymRunProgress = {
  schemaVersion: GYM_RUN_PROGRESS_SCHEMA,
  runRef: 'run.gym.terminal_bench.test',
  jobRef: 'job.gym.harbor_terminal_bench.test',
  configId: 'gym.terminal_bench.test',
  environmentRef: 'terminal-bench',
  datasetRef: 'terminal-bench@2.0',
  runner: 'harbor',
  agent: 'terminus-2',
  profile: {
    profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
    publicLabel: 'GLM REAP test profile',
    model: 'openagents/glm-5.2-reap-504b',
    attribution: 'test attribution',
    hardwareProfile: 'test-hardware',
    contextWindowTokens: 250_000,
  },
  phase: 'running',
  decisionGrade: false,
  inProgress: true,
  publication: 'web_authorized',
  counts: {
    officialDenominator: 89,
    completed: 41,
    completedPassed: 27,
    completedFailed: 14,
    running: 4,
    pending: 44,
    error: 0,
    cancelled: 0,
  },
  passRateOverCompleted: 27 / 41,
  completionFraction: 41 / 89,
  tokens: {
    promptTokens: 1_840_000,
    completionTokens: 612_000,
    totalTokens: 2_452_000,
  },
  elapsedMs: 1_920_000,
  lastUpdatedAt: '2026-06-25T00:00:00.000Z',
  caveatRefs: ['caveat.gym.run_progress.partial_denominator_not_final_score'],
  blockerRefs: [],
}

const completedRun: GymRunProgress = {
  ...partialRun,
  phase: 'completed',
  inProgress: false,
  counts: {
    officialDenominator: 89,
    completed: 89,
    completedPassed: 61,
    completedFailed: 28,
    running: 0,
    pending: 0,
    error: 0,
    cancelled: 0,
  },
  passRateOverCompleted: 61 / 89,
  completionFraction: 1,
}

describe('runProgress formatting helpers', () => {
  test('formats null measurements as not measured, never 0', () => {
    expect(formatRunProgressPercent(null)).toBe('not measured')
    expect(formatRunProgressCount(null)).toBe('not measured')
    expect(formatRunProgressDuration(null)).toBe('not measured')
  })

  test('formats measured values', () => {
    expect(formatRunProgressPercent(0.659)).toBe('65.9%')
    expect(formatRunProgressCount(2_452_000)).toBe('2,452,000')
    expect(formatRunProgressDuration(1_920_000)).toBe('32m 0s')
  })

  test('phase label marks partial runs in-progress and finished runs not decision-grade', () => {
    expect(runPhaseLabel(partialRun)).toContain('in progress')
    expect(runPhaseLabel(completedRun)).toContain('not decision-grade')
  })
})

describe('runProgress three-effect adapter', () => {
  test('builds a fan-out field for a partial run with passed/failed/running/pending buckets', () => {
    const options = runProgressVisualizationOptions(partialRun)
    const nodeIds = (options.nodes ?? []).map(node => node.id)

    expect(nodeIds).toContain('run:progress')
    expect(nodeIds).toContain('bucket:passed')
    expect(nodeIds).toContain('bucket:failed')
    expect(nodeIds).toContain('bucket:running')
    expect(nodeIds).toContain('bucket:pending')
    expect(nodeIds).toContain('report:progress')

    // Beams fan out from the run hub to every bucket.
    expect((options.beams ?? []).map(beam => beam.toId)).toEqual([
      'bucket:passed',
      'bucket:failed',
      'bucket:running',
      'bucket:pending',
    ])

    // A verdict burst fires only when there are accepted (passed) tasks.
    expect((options.bursts ?? []).length).toBeGreaterThan(0)

    // The decision-grade operator signal is always present and honest.
    const decisionSignal = (options.operatorSignals ?? []).find(
      signal => signal.id === 'run-progress.decision-grade',
    )
    expect(decisionSignal?.detail).toContain('false')
  })

  test('a completed run still renders the field with a sealed report node', () => {
    const options = runProgressVisualizationOptions(completedRun)
    const reportNode = (options.nodes ?? []).find(
      node => node.id === 'report:progress',
    )
    expect(reportNode?.status).toBe('sealed')

    const inProgressSignal = (options.operatorSignals ?? []).find(
      signal => signal.id === 'run-progress.in-progress',
    )
    expect(inProgressSignal?.state).toBe('success')
  })

  test('a run with no passed tasks fires no verdict burst', () => {
    const options = runProgressVisualizationOptions({
      ...partialRun,
      counts: {
        ...partialRun.counts,
        completed: 0,
        completedPassed: 0,
        completedFailed: 0,
        running: 5,
        pending: 84,
      },
      passRateOverCompleted: null,
      completionFraction: 0,
    })
    expect(options.bursts ?? []).toHaveLength(0)
  })

  test('the adapter never leaks private endpoint material', () => {
    const serialized = JSON.stringify(
      runProgressVisualizationOptions(partialRun),
    )
    expect(serialized).not.toContain('private_openai_compat')
    expect(serialized).not.toContain('Bearer')
    expect(serialized).not.toContain('https://')
  })
})
