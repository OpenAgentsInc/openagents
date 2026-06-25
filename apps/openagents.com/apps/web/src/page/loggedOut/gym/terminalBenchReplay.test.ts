import { describe, expect, test } from 'vitest'

import {
  TERMINAL_BENCH_VISUAL_REPLAY,
  terminalBenchReplayTotals,
  terminalBenchVisualizationOptions,
} from './terminalBenchReplay'

describe('Terminal-Bench Gym visual replay', () => {
  test('summarizes fixture lanes without claiming decision-grade status', () => {
    expect(terminalBenchReplayTotals(TERMINAL_BENCH_VISUAL_REPLAY)).toEqual({
      acceptedTasks: 129,
      failingTasks: 56,
      notStartedTasks: 3,
      totalTasks: 188,
      measuredLaneCount: 2,
      totalCostBasisMsat: 1_260_000,
    })

    expect(TERMINAL_BENCH_VISUAL_REPLAY).toMatchObject({
      datasetRef: 'terminal-bench@2.0',
      decisionGrade: false,
      publicSafe: true,
      rawArtifactsIncluded: false,
    })
  })

  test('projects lanes into three-effect visualization options', () => {
    const options = terminalBenchVisualizationOptions(
      TERMINAL_BENCH_VISUAL_REPLAY,
    )

    const nodes = options.nodes ?? []
    const beams = options.beams ?? []
    const worldItems = options.worldItems ?? []

    expect(nodes.map(node => [node.id, node.status])).toEqual(
      expect.arrayContaining([
        ['claim:external', 'sealed'],
        ['run:terminal-bench', 'active'],
        ['lane:glm-reap-504b-g4-tp4-mtp2-rp105', 'verified'],
        ['lane:glm-reap-504b-g4-tp4-minp-rp105', 'blocked'],
        ['lane:glm-reap-504b-g4-dual-tp4-minp-rp105', 'queued'],
        ['report:comparison', 'sealed'],
      ]),
    )
    expect(beams).toHaveLength(
      TERMINAL_BENCH_VISUAL_REPLAY.lanes.length,
    )
    expect(beams.every(beam => (beam.sourceRefs ?? []).length > 0)).toBe(true)
    expect(options.motionPolicy?.evidence).toBe('required')
    expect(
      worldItems.some(item =>
        (item.lines ?? []).includes('Verse integration deferred'),
      ),
    ).toBe(true)
  })

  test('keeps the public fixture free of raw or secret-bearing data', () => {
    const serialized = JSON.stringify(TERMINAL_BENCH_VISUAL_REPLAY)

    expect(serialized).not.toMatch(
      /bearer|api[_-]?key|raw prompt|completion text|wallet|mnemonic|token/i,
    )
  })
})
