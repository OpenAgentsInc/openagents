import { describe, expect, test } from 'vitest'

import {
  SAMPLE_DECISION_SUITE_CONFIG,
  TINY_TEST_CONFIG,
} from './fixtures'
import {
  buildCellId,
  decodeBenchmarkMatrixConfig,
  expandMatrix,
  expectedCellCount,
  laneAvailability,
  verificationExpectationForWorkload,
} from './matrix'

describe('benchmark matrix — expansion', () => {
  test('expands to exactly the cross-product cardinality', () => {
    const cells = expandMatrix(SAMPLE_DECISION_SUITE_CONFIG)
    // 4 targets × 4 workloads × 3 shapes × 2 transports × 2 sampling = 192.
    expect(expectedCellCount(SAMPLE_DECISION_SUITE_CONFIG)).toBe(192)
    expect(cells.length).toBe(192)
  })

  test('tiny config expands to a hand-checkable cell count', () => {
    // 2 targets × 1 workload × 1 shape × 1 transport × 1 sampling = 2.
    expect(expectedCellCount(TINY_TEST_CONFIG)).toBe(2)
    expect(expandMatrix(TINY_TEST_CONFIG).length).toBe(2)
  })

  test('expansion is deterministic — identical config yields identical ordered cell ids', () => {
    const a = expandMatrix(SAMPLE_DECISION_SUITE_CONFIG).map(c => c.cellId)
    const b = expandMatrix(SAMPLE_DECISION_SUITE_CONFIG).map(c => c.cellId)
    expect(a).toEqual(b)
    // No duplicate cell ids (each axis combination is unique).
    expect(new Set(a).size).toBe(a.length)
  })

  test('cell id encodes every axis value', () => {
    const id = buildCellId({
      lane: 'fireworks',
      engine: 'provider-native',
      workload: 'chat',
      shapeId: 'short-chat',
      transport: 'streaming',
      sampling: { temperature: 0.2, reasoningEffort: 'medium' },
    })
    expect(id).toBe(
      'fireworks|provider-native|chat|short-chat|streaming|t0.2|rmedium',
    )
  })

  test('each cell carries the honest lane-availability label', () => {
    const cells = expandMatrix(SAMPLE_DECISION_SUITE_CONFIG)
    const fireworks = cells.find(c => c.lane === 'fireworks')
    const pylon = cells.find(c => c.lane === 'pylon-whole-small')
    const psionic = cells.find(c => c.lane === 'psionic-shard-wan')
    expect(fireworks?.laneAvailability).toBe('available')
    expect(pylon?.laneAvailability).toBe('not_yet_available')
    expect(psionic?.laneAvailability).toBe('not_yet_available')
  })

  test('verification expectation is derived from workload (scored on outcome)', () => {
    expect(verificationExpectationForWorkload('chat')).toBe('none')
    expect(verificationExpectationForWorkload('khala-code-artifact-gen')).toBe(
      'test_passed',
    )
    expect(verificationExpectationForWorkload('verifier-run')).toBe(
      'test_passed',
    )
    expect(
      verificationExpectationForWorkload('long-context-codebase-question'),
    ).toBe('seeded')
  })

  test('lane availability table is the single source of truth', () => {
    expect(laneAvailability('vertex-anthropic')).toBe('available')
    expect(laneAvailability('fireworks')).toBe('available')
    expect(laneAvailability('partner-passthrough')).toBe('available')
    expect(laneAvailability('pylon-whole-small')).toBe('not_yet_available')
    expect(laneAvailability('psionic-shard-wan')).toBe('not_yet_available')
  })

  test('the sample + tiny configs decode against the schema', () => {
    expect(() =>
      decodeBenchmarkMatrixConfig(SAMPLE_DECISION_SUITE_CONFIG),
    ).not.toThrow()
    expect(() => decodeBenchmarkMatrixConfig(TINY_TEST_CONFIG)).not.toThrow()
  })
})
