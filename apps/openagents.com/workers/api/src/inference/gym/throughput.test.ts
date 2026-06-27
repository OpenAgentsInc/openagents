import { describe, expect, test } from 'vitest'

import { NOT_MEASURED } from '../khala-telemetry'
import {
  GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP,
  type GymThroughputEnvironmentSpec,
  type GymThroughputLeverActual,
  type GymThroughputRolloutMeasuredConfiguration,
  type GymThroughputRolloutMeasurementEvidence,
  type GymThroughputRolloutRecommendation,
  type GymThroughputSample,
  buildGymThroughputOwnerArmedRolloutRunArtifact,
  buildGymThroughputReport,
  buildGymThroughputRolloutReadout,
  decodeGymThroughputEnvironmentSpec,
  decodeGymThroughputSample,
  expandGymThroughputOptimizationSweep,
  recommendGymThroughputRollout,
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

const measuredConfigurationFromActualLever = (
  lever: GymThroughputLeverActual,
): GymThroughputRolloutMeasuredConfiguration => ({
  maxNumSeqs: lever.maxNumSeqs,
  prefixCachingEnabled: lever.enablePrefixCaching,
  chunkedPrefillEnabled: lever.enableChunkedPrefill,
  speculativeDecodeEnabled:
    lever.lowBatchSpeculativeDecoding.policy === 'enabled_below_batch',
  lowBatchSpeculativeDecoding: lever.lowBatchSpeculativeDecoding,
})

const measuredConfigurationFromRecommendation = (
  recommendation: GymThroughputRolloutRecommendation,
): GymThroughputRolloutMeasuredConfiguration => {
  const selection = recommendation.selection!
  return {
    maxNumSeqs: selection.maxNumSeqs,
    prefixCachingEnabled: selection.prefixCachingEnabled,
    chunkedPrefillEnabled: selection.chunkedPrefillEnabled,
    speculativeDecodeEnabled:
      selection.lowBatchSpeculativeDecoding.policy === 'enabled_below_batch',
    lowBatchSpeculativeDecoding: selection.lowBatchSpeculativeDecoding,
  }
}

const measuredGlmRolloutRecommendation = () => {
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

  return recommendGymThroughputRollout({
    report,
    lane: 'glm-52',
    maxInteractiveItlP90Multiplier: 1.5,
  })
}

const measuredRolloutEvidence = (
  recommendation: GymThroughputRolloutRecommendation,
): GymThroughputRolloutMeasurementEvidence => {
  const selection = recommendation.selection!
  const beforeConfiguration = measuredConfigurationFromActualLever(
    passedActualLever(0),
  )
  const afterConfiguration =
    measuredConfigurationFromRecommendation(recommendation)
  const before = {
    configuration: beforeConfiguration,
    aggregateTokensPerSecond: selection.baselineAggregateTps,
    interTokenLatencyP90Ms: selection.baselineInterTokenLatencyP90Ms,
    ttftP90Ms: selection.baselineTtftP90Ms,
  }
  const after = {
    configuration: afterConfiguration,
    aggregateTokensPerSecond: selection.aggregateTps,
    interTokenLatencyP90Ms: selection.interTokenLatencyP90Ms,
    ttftP90Ms: selection.ttftP90Ms,
  }

  return {
    schemaVersion: 'openagents.gym.throughput_rollout_measurement_evidence.v1',
    evidenceRef: 'evidence.gym.glm_52.vllm.issue_6320.measured_rollout.001',
    evidenceKind: 'measured_rollout',
    publicSafe: true,
    liveVllmFlags: [...selection.vllmFlags],
    before,
    after,
    expectedVsActual: {
      expectedAfter: after,
      actualAfter: after,
      expectedAggregateTpsLiftPercent: selection.aggregateTpsLiftPercent,
      actualAggregateTpsLiftPercent: selection.aggregateTpsLiftPercent,
      maxNumSeqsMatches: true,
      prefixCachingMatches: true,
      chunkedPrefillMatches: true,
      speculativeDecodingMatches: true,
    },
    publicEvidenceRefs: [
      'report.gym.throughput.glm_52.vllm.issue_6320.measurement.001',
      'receipt.gym.throughput.glm_52.vllm.issue_6320.before_after.001',
    ],
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
    expect(lane?.concurrencyPoints[0]?.actualThroughputLevers[0]).toEqual(
      actualThroughputLevers,
    )
  })

  test('selects a measured owner-armed rollout knee and emits public-safe vLLM flags', () => {
    const recommendation = measuredGlmRolloutRecommendation()

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

  test('builds a public-safe owner-armed GLM vLLM rollout run artifact without applying flags', () => {
    const recommendation = measuredGlmRolloutRecommendation()

    const artifact = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
    })

    expect(artifact.schemaVersion).toBe(
      'openagents.gym.throughput_owner_armed_rollout_run.v1',
    )
    expect(artifact.status).toBe('ready_to_apply')
    expect(artifact.canApplyLiveFlags).toBe(true)
    expect(artifact.blockers).toEqual([])
    expect(artifact.applicationPlan).toEqual({
      target: {
        lane: 'glm-52',
        engine: 'vllm',
        modelRef: '0xSero/GLM-5.2-504B',
      },
      sweepRef: 'sweep.gym.glm_52.vllm_throughput_levers.v1',
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
      applyMode: 'owner_armed_manual',
      selection: recommendation.selection,
      vllmFlags: recommendation.selection?.vllmFlags,
    })
    expect(JSON.stringify(artifact)).not.toContain('https://')
    expect(JSON.stringify(artifact)).not.toContain('/Users/')
  })

  test('builds an inert rollout readout with typed progress and measured lift evidence', () => {
    const recommendation = measuredGlmRolloutRecommendation()
    const rolloutRun = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
    })

    const readout = buildGymThroughputRolloutReadout({
      generatedAt: '2026-06-26T13:05:00.000Z',
      rolloutRun,
      progressEvidence: {
        schemaVersion: 'openagents.gym.throughput_rollout_progress_evidence.v1',
        rolloutRef: 'rollout.gym.glm_52.vllm.issue_6320.measurement.001',
        observedAt: '2026-06-26T13:04:00.000Z',
        status: 'measured_lift',
        ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
        progressPercent: 100,
        publicEvidenceRefs: [
          'report.gym.throughput.glm_52.vllm.issue_6320.measurement.001',
        ],
        baselineAggregateTps: recommendation.selection!.baselineAggregateTps,
        measuredAggregateTps: recommendation.selection!.aggregateTps,
        measuredThroughputLiftPercent:
          recommendation.selection!.aggregateTpsLiftPercent,
        rolloutMeasurementEvidence: measuredRolloutEvidence(recommendation),
      },
    })

    expect(readout.schemaVersion).toBe(
      'openagents.gym.throughput_rollout_readout.v1',
    )
    expect(readout.status).toBe('measured')
    expect(readout.metadataOnly).toBe(true)
    expect(readout.canMutateInfrastructure).toBe(false)
    expect(readout.measuredLiftPercent).toBeCloseTo(96.55, 2)
    expect(readout.rolloutMeasurementEvidence?.after.configuration).toEqual({
      maxNumSeqs: 4,
      prefixCachingEnabled: true,
      chunkedPrefillEnabled: true,
      speculativeDecodeEnabled: true,
      lowBatchSpeculativeDecoding: {
        policy: 'enabled_below_batch',
        maxBatchSize: 4,
        mode: 'n_gram',
      },
    })
    expect(
      readout.rolloutMeasurementEvidence?.before.aggregateTokensPerSecond,
    ).toBe(recommendation.selection!.baselineAggregateTps)
    expect(
      readout.rolloutMeasurementEvidence?.after.aggregateTokensPerSecond,
    ).toBe(recommendation.selection!.aggregateTps)
    expect(
      readout.rolloutMeasurementEvidence?.before.interTokenLatencyP90Ms,
    ).toBe(recommendation.selection!.baselineInterTokenLatencyP90Ms)
    expect(
      readout.rolloutMeasurementEvidence?.after.interTokenLatencyP90Ms,
    ).toBe(recommendation.selection!.interTokenLatencyP90Ms)
    expect(readout.rolloutMeasurementEvidence?.before.ttftP90Ms).toBe(
      recommendation.selection!.baselineTtftP90Ms,
    )
    expect(readout.rolloutMeasurementEvidence?.after.ttftP90Ms).toBe(
      recommendation.selection!.ttftP90Ms,
    )
    expect(readout.rolloutMeasurementEvidence?.liveVllmFlags).toEqual(
      recommendation.selection!.vllmFlags,
    )
    expect(
      readout.rolloutMeasurementEvidence?.expectedVsActual.maxNumSeqsMatches,
    ).toBe(true)
    expect(readout.evidenceChecklist.map(item => item.check)).toEqual([
      'owner_arm_ref',
      'live_max_num_seqs',
      'live_vllm_flags',
      'prefix_cache',
      'chunked_prefill',
      'speculative_decode',
      'before_tokens_per_second',
      'after_tokens_per_second',
      'before_inter_token_latency_p90',
      'after_inter_token_latency_p90',
      'before_ttft_p90',
      'after_ttft_p90',
      'expected_vs_actual_configuration',
      'expected_vs_actual_lift',
      'rollout_progress_status',
      'rollout_progress_percent',
      'progress_totals_match_measurement',
      'public_evidence_refs',
    ])
    expect(readout.evidenceChecklist.map(item => item.status)).toEqual(
      Array.from({ length: 18 }, () => 'satisfied'),
    )
    expect(
      readout.evidenceChecklist.find(item => item.check === 'live_max_num_seqs'),
    ).toMatchObject({
      expected: 4,
      actual: 4,
      publicEvidenceRefs: [
        'report.gym.throughput.glm_52.vllm.issue_6320.measurement.001',
        'receipt.gym.throughput.glm_52.vllm.issue_6320.before_after.001',
      ],
    })
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'prefix_cache',
      ),
    ).toMatchObject({ expected: true, actual: true })
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'chunked_prefill',
      ),
    ).toMatchObject({ expected: true, actual: true })
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'speculative_decode',
      ),
    ).toMatchObject({ expected: true, actual: true })
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'live_vllm_flags',
      ),
    ).toMatchObject({
      status: 'satisfied',
      expected:
        '--max-num-seqs=4 --enable-prefix-caching --enable-chunked-prefill --speculative-config={"method":"n_gram","disable_at_batch_size":5}',
      actual:
        '--max-num-seqs=4 --enable-prefix-caching --enable-chunked-prefill --speculative-config={"method":"n_gram","disable_at_batch_size":5}',
    })
    expect(readout.blockers).toEqual([])
    expect(readout.operatorAcceptance).toMatchObject({
      status: 'accepted_for_stress_and_benchmark',
      canStartIssue6317Stress: true,
      canStartIssue6312Benchmark: true,
      remainingChecks: [],
    })
    expect(
      readout.operatorAcceptance.requirements.every(requirement =>
        requirement.blocksIssueRefs.includes('#6317') &&
        requirement.blocksIssueRefs.includes('#6312'),
      ),
    ).toBe(true)
    expect(JSON.stringify(readout)).not.toContain('https://')
    expect(JSON.stringify(readout)).not.toContain('/Users/')
  })

  test('treats equivalent live vLLM flags as satisfied regardless of report order', () => {
    const recommendation = measuredGlmRolloutRecommendation()
    const rolloutRun = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
    })
    const evidence = measuredRolloutEvidence(recommendation)
    const reorderedEvidence: GymThroughputRolloutMeasurementEvidence = {
      ...evidence,
      liveVllmFlags: [...evidence.liveVllmFlags].reverse(),
    }

    const readout = buildGymThroughputRolloutReadout({
      generatedAt: '2026-06-26T13:09:00.000Z',
      rolloutRun,
      progressEvidence: {
        schemaVersion: 'openagents.gym.throughput_rollout_progress_evidence.v1',
        rolloutRef: 'rollout.gym.glm_52.vllm.issue_6320.measurement.001',
        observedAt: '2026-06-26T13:08:00.000Z',
        status: 'measured_lift',
        ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
        progressPercent: 100,
        publicEvidenceRefs: [
          'report.gym.throughput.glm_52.vllm.issue_6320.measurement.001',
        ],
        baselineAggregateTps: recommendation.selection!.baselineAggregateTps,
        measuredAggregateTps: recommendation.selection!.aggregateTps,
        measuredThroughputLiftPercent:
          recommendation.selection!.aggregateTpsLiftPercent,
        rolloutMeasurementEvidence: reorderedEvidence,
      },
    })

    expect(readout.blockers).not.toContain('live_engine_flags_mismatch')
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'live_vllm_flags',
      ),
    ).toMatchObject({ status: 'satisfied' })
    expect(readout.operatorAcceptance).toMatchObject({
      status: 'accepted_for_stress_and_benchmark',
      canStartIssue6317Stress: true,
      canStartIssue6312Benchmark: true,
      remainingChecks: [],
    })
  })

  test('fails closed when rollout readout progress evidence is missing, mismatched, or lacks real measurement', () => {
    const recommendation = measuredGlmRolloutRecommendation()
    const rolloutRun = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
    })

    const missingEvidence = buildGymThroughputRolloutReadout({
      generatedAt: '2026-06-26T13:05:00.000Z',
      rolloutRun,
    })
    const missingMeasurement = buildGymThroughputRolloutReadout({
      generatedAt: '2026-06-26T13:06:00.000Z',
      rolloutRun,
      progressEvidence: {
        schemaVersion: 'openagents.gym.throughput_rollout_progress_evidence.v1',
        rolloutRef: 'rollout.gym.glm_52.vllm.issue_6320.measurement.001',
        observedAt: '2026-06-26T13:05:00.000Z',
        status: 'measured_lift',
        ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
        progressPercent: 100,
        publicEvidenceRefs: [
          'report.gym.throughput.glm_52.vllm.issue_6320.measurement.001',
        ],
        baselineAggregateTps: recommendation.selection!.baselineAggregateTps,
        measuredAggregateTps: recommendation.selection!.aggregateTps,
        measuredThroughputLiftPercent:
          recommendation.selection!.aggregateTpsLiftPercent,
      },
    })
    const mismatchedOwnerArm = buildGymThroughputRolloutReadout({
      generatedAt: '2026-06-26T13:07:00.000Z',
      rolloutRun,
      progressEvidence: {
        schemaVersion: 'openagents.gym.throughput_rollout_progress_evidence.v1',
        rolloutRef: 'rollout.gym.glm_52.vllm.issue_6320.measurement.002',
        observedAt: '2026-06-26T13:05:00.000Z',
        status: 'measured_lift',
        ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.other.v1',
        progressPercent: 100,
        publicEvidenceRefs: [
          'report.gym.throughput.glm_52.vllm.issue_6320.measurement.002',
        ],
        baselineAggregateTps: 171,
        measuredAggregateTps: 170,
        measuredThroughputLiftPercent: -0.58,
      },
    })

    expect(missingEvidence.status).toBe('blocked')
    expect(missingEvidence.measuredLiftPercent).toBe(NOT_MEASURED)
    expect(missingEvidence.blockers).toEqual(['missing_progress_evidence'])
    expect(
      missingEvidence.evidenceChecklist.find(
        item => item.check === 'after_tokens_per_second',
      ),
    ).toMatchObject({ status: 'missing', expected: 171, actual: null })
    expect(missingEvidence.operatorAcceptance).toMatchObject({
      status: 'blocked_before_stress_and_benchmark',
      canStartIssue6317Stress: false,
      canStartIssue6312Benchmark: false,
    })
    expect(missingEvidence.operatorAcceptance.remainingChecks).toContain(
      'rollout_progress_status',
    )
    expect(missingMeasurement.status).toBe('blocked')
    expect(missingMeasurement.rolloutMeasurementEvidence).toBeNull()
    expect(missingMeasurement.blockers).toEqual([
      'missing_rollout_measurement_evidence',
    ])
    expect(
      missingMeasurement.evidenceChecklist.find(
        item => item.check === 'live_max_num_seqs',
      ),
    ).toMatchObject({ status: 'missing', expected: 4, actual: null })
    expect(mismatchedOwnerArm.status).toBe('blocked')
    expect(mismatchedOwnerArm.blockers).toEqual([
      'owner_arm_ref_mismatch',
      'measured_lift_not_positive',
      'missing_rollout_measurement_evidence',
    ])
    expect(
      mismatchedOwnerArm.evidenceChecklist.find(
        item => item.check === 'owner_arm_ref',
      ),
    ).toMatchObject({
      status: 'mismatch',
      expected: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
      actual: 'owner_arm.gym.glm_52.vllm_rollout.other.v1',
    })
  })

  test('fails closed when measured rollout evidence does not match the selected levers or progress totals', () => {
    const recommendation = measuredGlmRolloutRecommendation()
    const rolloutRun = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
    })
    const evidence = measuredRolloutEvidence(recommendation)
    const mismatchedEvidence: GymThroughputRolloutMeasurementEvidence = {
      ...evidence,
      after: {
        ...evidence.after,
        configuration: {
          ...evidence.after.configuration,
          maxNumSeqs: 8,
        },
        aggregateTokensPerSecond: 120,
      },
      expectedVsActual: {
        ...evidence.expectedVsActual,
        actualAfter: {
          ...evidence.expectedVsActual.actualAfter,
          configuration: {
            ...evidence.expectedVsActual.actualAfter.configuration,
            maxNumSeqs: 8,
          },
          aggregateTokensPerSecond: 120,
        },
        maxNumSeqsMatches: false,
        actualAggregateTpsLiftPercent: 5,
      },
    }

    const readout = buildGymThroughputRolloutReadout({
      generatedAt: '2026-06-26T13:08:00.000Z',
      rolloutRun,
      progressEvidence: {
        schemaVersion: 'openagents.gym.throughput_rollout_progress_evidence.v1',
        rolloutRef: 'rollout.gym.glm_52.vllm.issue_6320.measurement.003',
        observedAt: '2026-06-26T13:07:00.000Z',
        status: 'measured_lift',
        ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
        progressPercent: 100,
        publicEvidenceRefs: [
          'report.gym.throughput.glm_52.vllm.issue_6320.measurement.003',
        ],
        baselineAggregateTps: recommendation.selection!.baselineAggregateTps,
        measuredAggregateTps: recommendation.selection!.aggregateTps,
        measuredThroughputLiftPercent:
          recommendation.selection!.aggregateTpsLiftPercent,
        rolloutMeasurementEvidence: mismatchedEvidence,
      },
    })

    expect(readout.status).toBe('blocked')
    expect(readout.blockers).toEqual([
      'rollout_measurement_selection_mismatch',
      'expected_actual_evidence_mismatch',
      'progress_measurement_mismatch',
    ])
    expect(
      readout.evidenceChecklist.find(item => item.check === 'live_max_num_seqs'),
    ).toMatchObject({ status: 'mismatch', expected: 4, actual: 8 })
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'progress_totals_match_measurement',
      ),
    ).toMatchObject({
      status: 'mismatch',
      expected: 'measurement_totals',
      actual: 'progress_totals',
    })
  })

  test('keeps #6317 and #6312 blocked when live flags, TTFT, or progress are incomplete', () => {
    const recommendation = measuredGlmRolloutRecommendation()
    const rolloutRun = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
    })
    const evidence = measuredRolloutEvidence(recommendation)
    const incompleteEvidence: GymThroughputRolloutMeasurementEvidence = {
      ...evidence,
      liveVllmFlags: [{ name: '--max-num-seqs', value: '2' }],
      before: {
        ...evidence.before,
        ttftP90Ms: NOT_MEASURED,
      },
      after: {
        ...evidence.after,
        ttftP90Ms: NOT_MEASURED,
      },
    }

    const readout = buildGymThroughputRolloutReadout({
      generatedAt: '2026-06-26T13:09:00.000Z',
      rolloutRun,
      progressEvidence: {
        schemaVersion: 'openagents.gym.throughput_rollout_progress_evidence.v1',
        rolloutRef: 'rollout.gym.glm_52.vllm.issue_6320.measurement.004',
        observedAt: '2026-06-26T13:08:00.000Z',
        status: 'measured_lift',
        ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
        progressPercent: 75,
        publicEvidenceRefs: [
          'report.gym.throughput.glm_52.vllm.issue_6320.measurement.004',
        ],
        baselineAggregateTps: recommendation.selection!.baselineAggregateTps,
        measuredAggregateTps: recommendation.selection!.aggregateTps,
        measuredThroughputLiftPercent:
          recommendation.selection!.aggregateTpsLiftPercent,
        rolloutMeasurementEvidence: incompleteEvidence,
      },
    })

    expect(readout.status).toBe('blocked')
    expect(readout.blockers).toEqual([
      'rollout_progress_incomplete',
      'rollout_measurement_incomplete',
      'live_engine_flags_mismatch',
    ])
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'live_vllm_flags',
      ),
    ).toMatchObject({
      status: 'mismatch',
      expected:
        '--max-num-seqs=4 --enable-prefix-caching --enable-chunked-prefill --speculative-config={"method":"n_gram","disable_at_batch_size":5}',
      actual: '--max-num-seqs=2',
    })
    expect(
      readout.evidenceChecklist.find(item => item.check === 'before_ttft_p90'),
    ).toMatchObject({ status: 'missing' })
    expect(
      readout.evidenceChecklist.find(item => item.check === 'after_ttft_p90'),
    ).toMatchObject({ status: 'missing' })
    expect(
      readout.evidenceChecklist.find(
        item => item.check === 'rollout_progress_percent',
      ),
    ).toMatchObject({ status: 'mismatch', expected: 100, actual: 75 })
    expect(readout.operatorAcceptance).toMatchObject({
      status: 'blocked_before_stress_and_benchmark',
      canStartIssue6317Stress: false,
      canStartIssue6312Benchmark: false,
      remainingChecks: [
        'live_vllm_flags',
        'before_ttft_p90',
        'after_ttft_p90',
        'rollout_progress_percent',
      ],
    })
  })

  test('keeps a decision-grade recommendation inert until the owner arm ref exists', () => {
    const recommendation = measuredGlmRolloutRecommendation()

    const artifact = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: '   ',
    })

    expect(artifact.status).toBe('blocked')
    expect(artifact.canApplyLiveFlags).toBe(false)
    expect(artifact.ownerArmRef).toBeNull()
    expect(artifact.applicationPlan).toBeNull()
    expect(artifact.blockers).toEqual(['missing_owner_arm_ref'])
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

  test('does not mint an apply-ready artifact from a blocked recommendation', () => {
    const glmSpec = spec({
      target: {
        lane: 'glm-52',
        engine: 'vllm',
        modelRef: '0xSero/GLM-5.2-504B',
      },
      concurrencyRamp: [2],
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
          interTokenLatencyMs: NOT_MEASURED,
          actualThroughputLevers: passedActualLever(0),
        }),
      ],
    })
    const recommendation = recommendGymThroughputRollout({
      report,
      lane: 'glm-52',
      maxInteractiveItlP90Multiplier: 1.5,
    })

    const artifact = buildGymThroughputOwnerArmedRolloutRunArtifact({
      generatedAt: '2026-06-26T13:00:00.000Z',
      recommendation,
      ownerArmRef: 'owner_arm.gym.glm_52.vllm_rollout.issue_6320.v1',
    })

    expect(artifact.status).toBe('blocked')
    expect(artifact.canApplyLiveFlags).toBe(false)
    expect(artifact.applicationPlan).toBeNull()
    expect(artifact.blockers).toEqual([
      'recommendation_not_decision_grade',
      'recommendation_has_blockers',
      'missing_selection',
    ])
  })
})
