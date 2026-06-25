import { describe, expect, test } from 'vitest'

import {
  LIVE_GYM_RUN_PROGRESS_FIXTURE,
  type GymRunProgress,
  formatRunProgressCount,
  formatRunProgressDuration,
  formatRunProgressPercent,
  runPhaseLabel,
  runProgressVisualizationOptions,
} from './runProgress'

const completedRun: GymRunProgress = {
  ...LIVE_GYM_RUN_PROGRESS_FIXTURE,
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
    expect(runPhaseLabel(LIVE_GYM_RUN_PROGRESS_FIXTURE)).toContain('in progress')
    expect(runPhaseLabel(completedRun)).toContain('not decision-grade')
  })
})

describe('runProgress three-effect adapter', () => {
  test('builds a fan-out field for a partial run with passed/failed/running/pending buckets', () => {
    const options = runProgressVisualizationOptions(LIVE_GYM_RUN_PROGRESS_FIXTURE)
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
      ...LIVE_GYM_RUN_PROGRESS_FIXTURE,
      counts: {
        ...LIVE_GYM_RUN_PROGRESS_FIXTURE.counts,
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

  test('the fixture never leaks private endpoint material', () => {
    const serialized = JSON.stringify(
      runProgressVisualizationOptions(LIVE_GYM_RUN_PROGRESS_FIXTURE),
    )
    expect(serialized).not.toContain('private_openai_compat')
    expect(serialized).not.toContain('Bearer')
    expect(serialized).not.toContain('https://')
  })
})
