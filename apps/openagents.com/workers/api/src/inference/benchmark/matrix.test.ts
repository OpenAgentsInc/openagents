import { describe, expect, test } from 'vitest'

import {
  GLM_52_REAP_POOL_TARGET,
  KHALA_GLM_PROVIDER_OBSERVED_SWEEP_CONFIG,
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
    // 8 targets × 4 workloads × 3 shapes × 2 transports × 2 sampling = 384.
    expect(expectedCellCount(SAMPLE_DECISION_SUITE_CONFIG)).toBe(384)
    expect(cells.length).toBe(384)
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

  test('profiled targets are candidate-aware and keep GLM pool metadata', () => {
    const cells = expandMatrix({
      ...TINY_TEST_CONFIG,
      id: 'profiled-glm-test-v1',
      targets: [GLM_52_REAP_POOL_TARGET],
    })
    const first = cells[0]!
    expect(first.candidateRef).toBe(
      'hydralisk.glm_52_reap_504b.pool.vllm.tp4x2.v1',
    )
    expect(first.cellId).toContain(
      'profile:hydralisk.glm_52_reap_504b.pool.vllm.tp4x2.v1',
    )
    expect(first.targetProfile?.replicaPoolRef).toBe(
      'pool.hydralisk.glm_52_reap_504b',
    )
    expect(first.targetProfile?.replicaCount).toBe(2)
    expect(first.targetProfile?.routeRole).toBe('first')
  })

  test('sample suite includes the requested provider field and GLM pool', () => {
    const candidateRefs = SAMPLE_DECISION_SUITE_CONFIG.targets.map(
      target => target.profile?.profileRef ?? `${target.lane}/${target.engine}`,
    )
    expect(candidateRefs).toEqual(
      expect.arrayContaining([
        'fireworks.deepseek_v4_flash.provider_native.v1',
        'hydralisk.gpt_oss_120b.vllm.v1',
        'hydralisk.gpt_oss_20b.vllm.v1',
        'vertex.gemini_2_5_flash.provider_native.v1',
        'hydralisk.glm_52_reap_504b.pool.vllm.tp4x2.v1',
      ]),
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
    expect(verificationExpectationForWorkload('opencode-coding-task')).toBe(
      'test_passed',
    )
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
    expect(laneAvailability('khala')).toBe('available')
    expect(laneAvailability('bigpickle')).toBe('fixture_only')
    expect(laneAvailability('gemini-free')).toBe('fixture_only')
    expect(laneAvailability('openai-gpt')).toBe('fixture_only')
    expect(laneAvailability('claude')).toBe('fixture_only')
    expect(laneAvailability('vertex-anthropic')).toBe('available')
    expect(laneAvailability('fireworks')).toBe('available')
    expect(laneAvailability('partner-passthrough')).toBe('available')
    expect(laneAvailability('gpt-oss-20b')).toBe('available')
    expect(laneAvailability('gpt-oss-120b')).toBe('available')
    expect(laneAvailability('glm-52')).toBe('available')
    expect(laneAvailability('pylon-whole-small')).toBe('not_yet_available')
    expect(laneAvailability('psionic-shard-wan')).toBe('not_yet_available')
  })

  test('the sample + tiny configs decode against the schema', () => {
    expect(() =>
      decodeBenchmarkMatrixConfig(SAMPLE_DECISION_SUITE_CONFIG),
    ).not.toThrow()
    expect(() => decodeBenchmarkMatrixConfig(TINY_TEST_CONFIG)).not.toThrow()
    expect(() =>
      decodeBenchmarkMatrixConfig(KHALA_GLM_PROVIDER_OBSERVED_SWEEP_CONFIG),
    ).not.toThrow()
  })

  test('observed sweep template carries public-safe shape evidence', () => {
    const shape = KHALA_GLM_PROVIDER_OBSERVED_SWEEP_CONFIG.shapes[0]!
    expect(shape.provenance).toBe('realistic')
    expect(shape.requestClass).toBe('interactive_stream')
    expect(shape.source).toBe('operator_export')
    expect(shape.observedRequestCount).toBe(560)
    expect(shape.observedTrafficEvidenceRef).toBe(
      'evidence.openagents.token_usage_events.fireworks_mix.2026_06_25',
    )
  })
})
