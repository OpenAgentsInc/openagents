import { describe, expect, test } from 'vitest'

import {
  type TerminalBenchVisualReplay,
  terminalBenchReplayTotals,
  terminalBenchVisualizationOptions,
} from './terminalBenchReplay'

// A test-local replay object. This is NOT a shipped fixture: the page renders an
// honest empty state until a real published report is ingested. This only proves
// the totals helper and the three-effect adapter project a real replay shape.
const replay: TerminalBenchVisualReplay = {
  schemaVersion: 'openagents.gym.terminal_bench_visual_replay.v1',
  replayRef: 'replay.gym.terminal_bench.test',
  generatedAt: '2026-06-25T00:00:00.000Z',
  datasetRef: 'terminal-bench@2.0',
  taskSetLabel: 'Terminal-Bench 2.0 official denominator',
  officialTotalTasks: 89,
  externalClaim: {
    label: 'external claimed target',
    claimedFullDenominatorSolveRate: 0.691,
    sourceRefs: ['source.test.external_claim'],
    caveatRefs: ['caveat.external_claim.not_openagents_result'],
  },
  lanes: [
    {
      profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
      label: 'lane A',
      model: 'zai/glm-5.2-504b-reap-nvfp4',
      state: 'accepted',
      acceptedTasks: 62,
      failingTasks: 27,
      notStartedTasks: 0,
      totalTasks: 89,
      costBasisMsat: 620_000,
      ttftMs: 380,
      perceivedTps: 51,
      aggregateTps: 51,
      producerDeviceRef: 'test.producer',
      verifierDeviceRef: 'test.verifier',
      distinctVerifierDevice: true,
      caveatRefs: [],
      blockerRefs: [],
      evidenceRefs: ['report.gym.terminal_bench_comparison.test'],
    },
    {
      profileRef: 'glm-reap-504b-g4-tp4-minp-rp105',
      label: 'lane B',
      model: 'zai/glm-5.2-504b-reap-nvfp4',
      state: 'failing',
      acceptedTasks: 60,
      failingTasks: 29,
      notStartedTasks: 0,
      totalTasks: 89,
      costBasisMsat: 570_000,
      ttftMs: null,
      perceivedTps: null,
      aggregateTps: null,
      producerDeviceRef: 'test.producer',
      verifierDeviceRef: 'test.verifier',
      distinctVerifierDevice: true,
      caveatRefs: [],
      blockerRefs: [],
      evidenceRefs: ['report.gym.terminal_bench_comparison.test'],
    },
    {
      profileRef: 'glm-reap-504b-g4-dual-tp4-minp-rp105',
      label: 'lane C',
      model: 'zai/glm-5.2-504b-reap-nvfp4',
      state: 'not_started',
      acceptedTasks: 7,
      failingTasks: 0,
      notStartedTasks: 3,
      totalTasks: 10,
      costBasisMsat: 70_000,
      ttftMs: 0,
      perceivedTps: null,
      aggregateTps: 0,
      producerDeviceRef: 'test.producer',
      verifierDeviceRef: null,
      distinctVerifierDevice: false,
      caveatRefs: [],
      blockerRefs: [],
      evidenceRefs: ['report.gym.terminal_bench_comparison.pilot.test'],
    },
  ],
  decisionGrade: false,
  publicSafe: true,
  rawArtifactsIncluded: false,
  caveatRefs: ['caveat.gym.terminal_bench.no_raw_prompts_or_completions'],
  blockerRefs: ['blocker.gym.terminal_bench.distinct_device_verifier_required'],
  deferredIntegration: {
    surface: 'autopilot_verse',
    state: 'deferred',
    note: 'Full Autopilot Verse integration is deferred; this surface proves the web visualization contract first.',
  },
}

describe('Terminal-Bench Gym visual replay adapter', () => {
  test('summarizes lanes without claiming decision-grade status', () => {
    expect(terminalBenchReplayTotals(replay)).toEqual({
      acceptedTasks: 129,
      failingTasks: 56,
      notStartedTasks: 3,
      totalTasks: 188,
      measuredLaneCount: 2,
      totalCostBasisMsat: 1_260_000,
    })

    expect(replay).toMatchObject({
      datasetRef: 'terminal-bench@2.0',
      decisionGrade: false,
      publicSafe: true,
      rawArtifactsIncluded: false,
    })
  })

  test('projects lanes into three-effect visualization options', () => {
    const options = terminalBenchVisualizationOptions(replay)

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
    expect(beams).toHaveLength(replay.lanes.length)
    expect(beams.every(beam => (beam.sourceRefs ?? []).length > 0)).toBe(true)
    expect(beams.every(beam => beam.generatedAt === replay.generatedAt)).toBe(
      true,
    )
    expect(options.motionPolicy?.evidence).toBe('required')
    expect(
      worldItems.some(item =>
        (item.lines ?? []).includes('Verse integration deferred'),
      ),
    ).toBe(true)
  })

  test('keeps the projected options free of secret-bearing leak shapes', () => {
    // The honest "no raw prompts or completions" operator-signal copy is allowed;
    // assert only actual leak SHAPES (auth headers, key markers, wallet material).
    const serialized = JSON.stringify(terminalBenchVisualizationOptions(replay))

    expect(serialized).not.toMatch(
      /bearer\s|api[_-]?key|prompt_text|completion_text|mnemonic|https?:\/\//i,
    )
  })
})
