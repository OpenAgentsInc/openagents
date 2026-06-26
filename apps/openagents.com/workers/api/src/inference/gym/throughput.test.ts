import { describe, expect, test } from 'vitest'

import { NOT_MEASURED } from '../khala-telemetry'
import {
  GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP,
  buildGymThroughputReport,
  decodeGymThroughputEnvironmentSpec,
  decodeGymThroughputSample,
  expandGymThroughputOptimizationSweep,
  recommendGymThroughputRollout,
  type GymThroughputEnvironmentSpec,
  type GymThroughputLeverActual,
  type GymThroughputSample,
} from './throughput'

const spec = (
  overrides: Partial<GymThroughputEnvironmentSpec> = {},
): GymThroughputEnvironmentSpec => ({
  schemaVersion: 'openagents.gym.throughput_environment.v1',
  environmentRef: 'throughput-concurrency',
  target: {
    lane: 'gpt-oss-20b',
    engine: 'vllm',
    modelRef: 'openagents/khala-oss-20b',
  },
  promptProfile: 'gym-oss-short-code-prompt',
  concurrencyRamp: [1, 2, 4],
  samplesPerConcurrency: 2,
  degradationThresholdMultiplier: 1.5,
  serving: {
    speculationMode: 'n_gram',
  },
  ...overrides,
})

const sample = (
  concurrency: number,
  sampleIndex: number,
  overrides: Partial<GymThroughputSample> = {},
): GymThroughputSample => ({
  schemaVersion: 'openagents.gym.throughput_sample.v1',
  lane: 'gpt-oss-20b',
  engine: 'vllm',
  modelRef: 'openagents/khala-oss-20b',
  concurrency,
  sampleIndex,
  status: 'ok',
  ttftMs: 100 + concurrency,
  totalWallClockMs: 1000,
  perceivedTps: 50,
  interTokenLatencyMs: 20,
  completionTokens: 200,
  speculationMode: 'n_gram',
  speculationAcceptanceRate: 0.7,
  ...overrides,
})

const passedActualLever = (index: number): GymThroughputLeverActual => {
  const expectedLever =
    GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP.expectedLevers[index]!
  return {
    ...expectedLever,
    quantization: {
      ...expectedLever.quantization,
      gateStatus: 'passed',
    },
    kvHeadroomGate: {
      ...expectedLever.kvHeadroomGate,
      observedFreeKvCachePercent: 24,
      gateStatus: 'passed',
    },
  }
}

describe('Gym throughput/concurrency report (#6244)', () => {
  test('builds a repeatable lane report and detects latency degradation', () => {
    const input = {
      generatedAt: '2026-06-25T12:00:00.000Z',
      specs: [spec()],
      samples: [
        sample(1, 0, { totalWallClockMs: 1000, perceivedTps: 50 }),
        sample(1, 1, { totalWallClockMs: 1100, perceivedTps: 52 }),
        sample(2, 0, { totalWallClockMs: 1250, perceivedTps: 55 }),
        sample(2, 1, { totalWallClockMs: 1300, perceivedTps: 56 }),
        sample(4, 0, { totalWallClockMs: 1900, perceivedTps: 57 }),
        sample(4, 1, {
          totalWallClockMs: 2200,
          perceivedTps: 58,
          speculationAcceptanceRate: 0.8,
        }),
      ],
    }
    const report = buildGymThroughputReport(input)
    const reportAgain = buildGymThroughputReport(input)

    expect(report.schemaVersion).toBe(
      'openagents.gym.throughput_concurrency_report.v1',
    )
    expect(JSON.stringify(report)).toBe(JSON.stringify(reportAgain))
    const lane = report.lanes[0]
    expect(lane?.degradation).toEqual({
      concurrency: 4,
      reason: 'latency_degraded',
    })
    expect(lane?.concurrencyPoints.map(point => point.concurrency)).toEqual([
      1, 2, 4,
    ])
    expect(lane?.concurrencyPoints[0]?.totalWallClockMs.p90).toBe(1100)
    expect(lane?.concurrencyPoints[0]?.aggregateTps).toBe(102)
    expect(
      lane?.concurrencyPoints[2]?.speculationAcceptanceRate.sampleCount,
    ).toBe(2)
    expect(lane?.concurrencyPoints[2]?.speculationAcceptanceRate.p50).toBe(0.7)
    expect(lane?.speculationMode).toBe('n_gram')
  })

  test('reports quota-limited concurrency before latency degradation', () => {
    const report = buildGymThroughputReport({
      generatedAt: '2026-06-25T12:00:00.000Z',
      specs: [spec()],
      samples: [
        sample(1, 0),
        sample(2, 0, {
          status: 'quota_limited',
          errorClass: 'provider_quota_exceeded',
          ttftMs: NOT_MEASURED,
          totalWallClockMs: NOT_MEASURED,
          perceivedTps: NOT_MEASURED,
          interTokenLatencyMs: NOT_MEASURED,
          completionTokens: NOT_MEASURED,
          speculationAcceptanceRate: NOT_MEASURED,
        }),
        sample(4, 0, { totalWallClockMs: 4000 }),
      ],
    })

    const lane = report.lanes[0]
    expect(lane?.degradation).toEqual({
      concurrency: 2,
      reason: 'quota_limited',
    })
    expect(lane?.concurrencyPoints[1]?.quotaLimitedSamples).toBe(1)
    expect(lane?.concurrencyPoints[1]?.totalWallClockMs.sampleCount).toBe(0)
  })

  test('keeps measured zero distinct from not_measured', () => {
    const report = buildGymThroughputReport({
      generatedAt: '2026-06-25T12:00:00.000Z',
      specs: [spec({ concurrencyRamp: [1], samplesPerConcurrency: 2 })],
      samples: [
        sample(1, 0, {
          ttftMs: 0,
          perceivedTps: 0,
          interTokenLatencyMs: NOT_MEASURED,
          speculationAcceptanceRate: NOT_MEASURED,
        }),
        sample(1, 1, {
          ttftMs: NOT_MEASURED,
          perceivedTps: NOT_MEASURED,
          interTokenLatencyMs: NOT_MEASURED,
          speculationAcceptanceRate: NOT_MEASURED,
        }),
      ],
    })

    const point = report.lanes[0]?.concurrencyPoints[0]
    expect(point?.ttftMs.p50).toBe(0)
    expect(point?.ttftMs.sampleCount).toBe(1)
    expect(point?.perceivedTps.p50).toBe(0)
    expect(point?.perceivedTps.sampleCount).toBe(1)
    expect(point?.interTokenLatencyMs.p50).toBeNull()
    expect(point?.interTokenLatencyMs.sampleCount).toBe(0)
    expect(point?.speculationAcceptanceRate.p50).toBeNull()
  })

  test('decodes environment specs and samples at the schema boundary', () => {
    expect(decodeGymThroughputEnvironmentSpec(spec()).environmentRef).toBe(
      'throughput-concurrency',
    )
    expect(decodeGymThroughputSample(sample(1, 0)).ttftMs).toBe(101)
    expect(() =>
      decodeGymThroughputSample({
        ...sample(1, 0),
        status: 'not_a_status',
      }),
    ).toThrow()
  })

  test('expands GLM vLLM throughput optimization sweep metadata deterministically', () => {
    const glmSpec = spec({
      target: {
        lane: 'glm-52',
        engine: 'vllm',
        modelRef: '0xSero/GLM-5.2-504B',
      },
      serving: {
        speculationMode: 'n_gram',
        optimizationSweep: GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP,
      },
    })

    const expanded = expandGymThroughputOptimizationSweep(glmSpec)
    const expandedAgain = expandGymThroughputOptimizationSweep(glmSpec)

    expect(JSON.stringify(expanded)).toBe(JSON.stringify(expandedAgain))
    expect(expanded.map(lever => lever.maxNumSeqs)).toEqual([2, 4, 8, 16])
    expect(expanded.map(lever => lever.label)).toEqual([
      'vllm.max_num_seqs.2.prefix_cache.chunked_prefill.nvfp4',
      'vllm.max_num_seqs.4.prefix_cache.chunked_prefill.nvfp4',
      'vllm.max_num_seqs.8.prefix_cache.chunked_prefill.nvfp4',
      'vllm.max_num_seqs.16.prefix_cache.chunked_prefill.nvfp4',
    ])
    expect(expanded[0]?.enablePrefixCaching).toBe(true)
    expect(expanded[0]?.enableChunkedPrefill).toBe(true)
    expect(expanded[0]?.lowBatchSpeculativeDecoding).toEqual({
      policy: 'enabled_below_batch',
      maxBatchSize: 4,
      mode: 'n_gram',
    })
    expect(expanded[0]?.quantization).toEqual({
      mode: 'nvfp4',
      qualityGateRef: 'gate.gym.glm_52.reap_504b.nvfp4.accepted_outcome.v1',
    })
    expect(expanded[0]?.kvHeadroomGate).toEqual({
      minFreeKvCachePercent: 15,
      action: 'skip',
    })
  })

  test('reports public-safe expected and actual throughput lever fields', () => {
    const expectedLever =
      GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP.expectedLevers[1]!
    const actualThroughputLevers = passedActualLever(1)
    const report = buildGymThroughputReport({
      generatedAt: '2026-06-26T12:00:00.000Z',
      specs: [
        spec({
          target: {
            lane: 'glm-52',
            engine: 'vllm',
            modelRef: '0xSero/GLM-5.2-504B',
          },
          concurrencyRamp: [1],
          serving: {
            speculationMode: 'n_gram',
            optimizationSweep: GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP,
          },
        }),
      ],
      samples: [
        sample(1, 0, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          speculationMode: 'n_gram',
          actualThroughputLevers,
        }),
      ],
    })

    const lane = report.lanes[0]
    expect(lane?.optimizationSweep?.sweepRef).toBe(
      'sweep.gym.glm_52.vllm_throughput_levers.v1',
    )
    expect(lane?.throughputLeverLabels).toEqual([
      'vllm.max_num_seqs.2.prefix_cache.chunked_prefill.nvfp4',
      'vllm.max_num_seqs.4.prefix_cache.chunked_prefill.nvfp4',
      'vllm.max_num_seqs.8.prefix_cache.chunked_prefill.nvfp4',
      'vllm.max_num_seqs.16.prefix_cache.chunked_prefill.nvfp4',
    ])
    expect(lane?.expectedThroughputLevers[1]).toEqual(expectedLever)
    expect(
      lane?.concurrencyPoints[0]?.actualThroughputLevers[0],
    ).toEqual(actualThroughputLevers)
  })

  test('selects a measured owner-armed rollout knee and emits public-safe vLLM flags', () => {
    const glmSpec = spec({
      target: {
        lane: 'glm-52',
        engine: 'vllm',
        modelRef: '0xSero/GLM-5.2-504B',
      },
      concurrencyRamp: [2, 4, 8],
      serving: {
        speculationMode: 'n_gram',
        optimizationSweep: GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP,
      },
    })
    const report = buildGymThroughputReport({
      generatedAt: '2026-06-26T12:00:00.000Z',
      specs: [glmSpec],
      samples: [
        sample(2, 0, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          perceivedTps: 42,
          interTokenLatencyMs: 20,
          ttftMs: 900,
          actualThroughputLevers: passedActualLever(0),
        }),
        sample(2, 1, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          perceivedTps: 45,
          interTokenLatencyMs: 21,
          ttftMs: 920,
          actualThroughputLevers: passedActualLever(0),
        }),
        sample(4, 0, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          perceivedTps: 84,
          interTokenLatencyMs: 27,
          ttftMs: 520,
          actualThroughputLevers: passedActualLever(1),
        }),
        sample(4, 1, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          perceivedTps: 87,
          interTokenLatencyMs: 28,
          ttftMs: 530,
          actualThroughputLevers: passedActualLever(1),
        }),
        sample(8, 0, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          perceivedTps: 90,
          interTokenLatencyMs: 55,
          ttftMs: 510,
          actualThroughputLevers: passedActualLever(2),
        }),
      ],
    })

    const recommendation = recommendGymThroughputRollout({
      report,
      lane: 'glm-52',
      maxInteractiveItlP90Multiplier: 1.5,
    })

    expect(recommendation.decisionGrade).toBe(true)
    expect(recommendation.blockers).toEqual([])
    expect(recommendation.selection?.maxNumSeqs).toBe(4)
    expect(recommendation.selection?.aggregateTps).toBe(171)
    expect(recommendation.selection?.aggregateTpsLiftPercent).toBeCloseTo(
      96.55,
      2,
    )
    expect(recommendation.selection?.vllmFlags).toEqual([
      { name: '--max-num-seqs', value: '4' },
      { name: '--enable-prefix-caching' },
      { name: '--enable-chunked-prefill' },
      {
        name: '--speculative-config',
        value: '{"method":"n_gram","disable_at_batch_size":5}',
      },
    ])
    expect(JSON.stringify(recommendation)).not.toContain('https://')
  })

  test('blocks rollout when measured throughput violates the interactive ITL guard', () => {
    const glmSpec = spec({
      target: {
        lane: 'glm-52',
        engine: 'vllm',
        modelRef: '0xSero/GLM-5.2-504B',
      },
      concurrencyRamp: [2, 4],
      serving: {
        speculationMode: 'n_gram',
        optimizationSweep: GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP,
      },
    })
    const report = buildGymThroughputReport({
      generatedAt: '2026-06-26T12:00:00.000Z',
      specs: [glmSpec],
      samples: [
        sample(2, 0, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          perceivedTps: 40,
          interTokenLatencyMs: 20,
          actualThroughputLevers: passedActualLever(0),
        }),
        sample(4, 0, {
          lane: 'glm-52',
          engine: 'vllm',
          modelRef: '0xSero/GLM-5.2-504B',
          perceivedTps: 120,
          interTokenLatencyMs: 80,
          actualThroughputLevers: passedActualLever(1),
        }),
      ],
    })

    const recommendation = recommendGymThroughputRollout({
      report,
      lane: 'glm-52',
      maxInteractiveItlP90Multiplier: 1.5,
    })

    expect(recommendation.decisionGrade).toBe(false)
    expect(recommendation.selection).toBeNull()
    expect(recommendation.blockers).toEqual(['interactive_itl_slo_exceeded'])
  })
})
