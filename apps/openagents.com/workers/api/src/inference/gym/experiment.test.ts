import { describe, expect, test } from 'vitest'

import {
  BUNDLED_GYM_EXPERIMENT,
  compileGymExperiment,
  decodeGymExperiment,
  encodeGymExperiment,
  runGymFixtureExperiment,
} from './experiment'

describe('OpenAgents Gym experiment schema', () => {
  test('round-trips the bundled fixture experiment', () => {
    const decoded = decodeGymExperiment(BUNDLED_GYM_EXPERIMENT)
    const encoded = encodeGymExperiment(decoded)
    expect(encoded).toEqual(BUNDLED_GYM_EXPERIMENT)
  })

  test('rejects malformed configs at the schema boundary', () => {
    expect(() =>
      decodeGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        environment: 'terminal-bench',
      }),
    ).toThrow()
    expect(() =>
      decodeGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        samplesPerCell: 4,
      }),
    ).toThrow()
  })
})

describe('compileGymExperiment', () => {
  test('expands the bundled fixture experiment to the expected matrix cells', () => {
    const compiled = compileGymExperiment(BUNDLED_GYM_EXPERIMENT)
    // 4 lanes × 4 workloads × 3 shapes × 1 transport × 1 sampling.
    expect(compiled.expectedCellCount).toBe(48)
    expect(compiled.matrixConfig.id).toBe('gym:gym-fixture-decision-suite-v1')
    expect(compiled.matrixConfig.samplesPerCell).toBe(5)
    expect(compiled.policySelection.coordinator).toBe('heuristic-v0')
    expect(compiled.policySelection.fanout.lanes).toEqual([
      'fireworks',
      'vertex-anthropic',
      'pylon-whole-small',
      'psionic-shard-wan',
    ])
    expect(compiled.policySelection.skippedCells.length).toBe(24)
  })

  test('rejects seam: real before any lane seam can be constructed', () => {
    expect(() =>
      compileGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        budget: {
          ...BUNDLED_GYM_EXPERIMENT.budget,
          seam: 'real',
          ownerApprovalRef: 'approval:future',
        },
      }),
    ).toThrow(/fixture-only/)
  })

  test('rejects empty fanout and empty shapes with typed compile errors', () => {
    expect(() =>
      compileGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        policy: {
          ...BUNDLED_GYM_EXPERIMENT.policy,
          fanout: {
            ...BUNDLED_GYM_EXPERIMENT.policy.fanout,
            lanes: [],
          },
        },
      }),
    ).toThrow(/fanout/)
    expect(() =>
      compileGymExperiment({
        ...BUNDLED_GYM_EXPERIMENT,
        shapes: [],
      }),
    ).toThrow(/shapes/)
  })
})

describe('runGymFixtureExperiment', () => {
  test('is deterministic: same config produces byte-identical records and report', () => {
    const a = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    const b = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    expect(JSON.stringify(a.runSet)).toBe(JSON.stringify(b.runSet))
    expect(JSON.stringify(a.report)).toBe(JSON.stringify(b.report))
  })

  test('produces a public-safe illustrative report, not a decision-grade claim', () => {
    const result = runGymFixtureExperiment(BUNDLED_GYM_EXPERIMENT)
    expect(result.runSet.seamId).toBe('fixture')
    expect(result.runSet.seamCanSpend).toBe(false)
    expect(result.report.decisionGrade).toBe(false)
    expect(result.report.illustrativeNotice).toContain('ILLUSTRATIVE ONLY')
    expect(result.publicSafety.safe).toBe(true)
    expect(result.publicSafety.violations).toEqual([])
  })
})

