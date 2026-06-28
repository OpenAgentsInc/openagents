import { describe, expect, test } from 'vitest'

import {
  GLM_LIVE_ADAPTIVE_STRESS_ARTIFACT_SCHEMA,
  GLM_LIVE_ADAPTIVE_STRESS_SUPERVISOR_SCHEMA,
  GLM_REAP_SERVED_MODEL,
  buildGlmLiveAdaptiveStressArtifact,
  buildGlmLiveAdaptiveStressSupervisorTick,
  classifyGlmLiveAdaptiveStressFailure,
  decideGlmLiveAdaptiveStressConcurrency,
  decideGlmLiveAdaptiveStressWindowBreaker,
  glmLiveAdaptiveStressHeaders,
  summarizeGlmLiveAdaptiveStress,
  type GlmLiveAdaptiveStressObservation,
} from './live-adaptive-stress-runner'
import {
  GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA,
  GLM_STRESS_DEMAND_KIND,
  GLM_STRESS_DEMAND_SOURCE,
} from './stress-saturation-plan'

const okObservation = (
  requestRef: string,
  totalTokens = 1024,
): GlmLiveAdaptiveStressObservation => ({
  requestRef,
  status: 'ok',
  httpStatus: 200,
  failureKind: null,
  servedModel: GLM_REAP_SERVED_MODEL,
  provider: 'hydralisk-vllm-glm-5p2-reap-504b',
  worker: 'hydralisk-vllm-glm-5p2-reap-504b',
  inputTokens: 620,
  outputTokens: totalTokens - 620,
  totalTokens,
  usageTruth: 'exact',
  ttftMs: 1200,
  wallClockMs: 4000,
})

const overloadObservation = (
  requestRef: string,
): GlmLiveAdaptiveStressObservation => ({
  requestRef,
  status: 'failed',
  httpStatus: 502,
  failureKind: 'gateway_overload',
  servedModel: null,
  provider: null,
  worker: null,
  inputTokens: 0,
  outputTokens: 0,
  totalTokens: 0,
  usageTruth: 'missing',
  ttftMs: null,
  wallClockMs: 500,
})

describe('GLM live adaptive stress runner utilities (#6317)', () => {
  test('emits the bounded stress demand headers without auth material', () => {
    const headers = glmLiveAdaptiveStressHeaders(
      'issue6317-adaptive-20260627T1410Z',
      'issue6317-adaptive-20260627T1410Z-000001',
    )

    expect(headers).toEqual({
      'x-openagents-client': 'issue6317-adaptive-20260627T1410Z',
      'x-openagents-demand-kind': GLM_STRESS_DEMAND_KIND,
      'x-openagents-demand-source': GLM_STRESS_DEMAND_SOURCE,
      'x-openagents-request-ref':
        'issue6317-adaptive-20260627T1410Z-000001',
    })
    expect(JSON.stringify(headers)).not.toMatch(/bearer|secret|token/i)
  })

  test('maps the live overload ladder to the same 6 -> 4 -> 3 -> 2 backoff shape', () => {
    const first = decideGlmLiveAdaptiveStressConcurrency({
      cleanWindowIncreaseThreshold: 3,
      consecutiveCleanWindows: 0,
      currentConcurrency: 6,
      maxConcurrency: 9,
      minConcurrency: 2,
      observations: [
        ...Array.from({ length: 23 }, (_, index) =>
          okObservation(`ok-${index}`),
        ),
        ...Array.from({ length: 12 }, (_, index) =>
          overloadObservation(`failed-${index}`),
        ),
      ],
    })
    const second = decideGlmLiveAdaptiveStressConcurrency({
      cleanWindowIncreaseThreshold: 3,
      consecutiveCleanWindows: 0,
      currentConcurrency: 4,
      maxConcurrency: 9,
      minConcurrency: 2,
      observations: [
        ...Array.from({ length: 21 }, (_, index) =>
          okObservation(`ok-2-${index}`),
        ),
        ...Array.from({ length: 11 }, (_, index) =>
          overloadObservation(`failed-2-${index}`),
        ),
      ],
    })
    const third = decideGlmLiveAdaptiveStressConcurrency({
      cleanWindowIncreaseThreshold: 3,
      consecutiveCleanWindows: 0,
      currentConcurrency: 3,
      maxConcurrency: 9,
      minConcurrency: 2,
      observations: [
        ...Array.from({ length: 14 }, (_, index) =>
          okObservation(`ok-3-${index}`),
        ),
        ...Array.from({ length: 8 }, (_, index) =>
          overloadObservation(`failed-3-${index}`),
        ),
      ],
    })

    expect(first).toMatchObject({
      action: 'decrease',
      nextConcurrency: 4,
      overloadFailureCount: 12,
    })
    expect(second).toMatchObject({
      action: 'decrease',
      nextConcurrency: 3,
      overloadFailureCount: 11,
    })
    expect(third).toMatchObject({
      action: 'decrease',
      nextConcurrency: 2,
      overloadFailureCount: 8,
    })
  })

  test('holds clean windows before probing one higher concurrency', () => {
    const hold = decideGlmLiveAdaptiveStressConcurrency({
      cleanWindowIncreaseThreshold: 3,
      consecutiveCleanWindows: 1,
      currentConcurrency: 2,
      maxConcurrency: 5,
      minConcurrency: 2,
      observations: [okObservation('ok-clean-1'), okObservation('ok-clean-2')],
    })
    const increase = decideGlmLiveAdaptiveStressConcurrency({
      cleanWindowIncreaseThreshold: 3,
      consecutiveCleanWindows: 2,
      currentConcurrency: 2,
      maxConcurrency: 5,
      minConcurrency: 2,
      observations: [okObservation('ok-clean-3'), okObservation('ok-clean-4')],
    })

    expect(hold).toMatchObject({
      action: 'hold',
      nextConcurrency: 2,
      nextConsecutiveCleanWindows: 2,
    })
    expect(increase).toMatchObject({
      action: 'increase',
      nextConcurrency: 3,
      nextConsecutiveCleanWindows: 0,
      reasonRefs: [
        'runner.glm_live_adaptive_stress.clean_windows_allow_probe',
      ],
    })
  })

  test('pauses to the floor when external preemption is observed', () => {
    const decision = decideGlmLiveAdaptiveStressConcurrency({
      cleanWindowIncreaseThreshold: 3,
      consecutiveCleanWindows: 2,
      currentConcurrency: 5,
      maxConcurrency: 9,
      minConcurrency: 2,
      observations: [
        okObservation('ok-before-preempt'),
        {
          ...overloadObservation('yielded'),
          failureKind: 'rate_limited',
          httpStatus: 429,
          status: 'preempted_for_external',
        },
      ],
    })

    expect(decision).toMatchObject({
      action: 'pause',
      nextConcurrency: 2,
      nextConsecutiveCleanWindows: 0,
      preemptedCount: 1,
      reasonRefs: ['runner.glm_live_adaptive_stress.external_preemption_observed'],
    })
  })

  test('trips an intra-window breaker before a fast overload storm can refill', () => {
    const cleanProbe = decideGlmLiveAdaptiveStressWindowBreaker({
      currentConcurrency: 5,
      observations: [
        okObservation('ok-before-breaker-1'),
        okObservation('ok-before-breaker-2'),
      ],
    })
    const overloadBreaker = decideGlmLiveAdaptiveStressWindowBreaker({
      currentConcurrency: 5,
      observations: [
        overloadObservation('fast-overload-1'),
        overloadObservation('fast-overload-2'),
        overloadObservation('fast-overload-3'),
      ],
    })
    const mixedBreaker = decideGlmLiveAdaptiveStressWindowBreaker({
      currentConcurrency: 5,
      observations: [
        okObservation('mixed-ok-1'),
        okObservation('mixed-ok-2'),
        overloadObservation('mixed-overload-1'),
      ],
    })

    expect(cleanProbe).toMatchObject({
      failedCount: 0,
      observedCount: 2,
      tripped: false,
    })
    expect(overloadBreaker).toMatchObject({
      failedCount: 3,
      observedCount: 3,
      overloadFailureCount: 3,
      tripped: true,
    })
    expect(overloadBreaker.reasonRefs).toEqual([
      'runner.glm_live_adaptive_stress.overload_failures_observed',
      'runner.glm_live_adaptive_stress.error_rate_over_budget',
    ])
    expect(mixedBreaker).toMatchObject({
      failedCount: 1,
      observedCount: 3,
      overloadFailureCount: 1,
      tripped: true,
    })
    expect(mixedBreaker.reasonRefs).toEqual([
      'runner.glm_live_adaptive_stress.error_rate_over_budget',
    ])
  })

  test('summarizes exact GLM receipts separately from non-GLM fallback receipts', () => {
    const summary = summarizeGlmLiveAdaptiveStress([
      okObservation('glm-1', 1100),
      okObservation('glm-2', 900),
      {
        ...okObservation('fallback-1', 777),
        servedModel: 'fireworks/deepseek-v3',
      },
      overloadObservation('failed-1'),
    ])

    expect(summary).toMatchObject({
      failedCount: 1,
      inputTokens: 1240,
      nonGlmOkCount: 1,
      nonGlmTokens: 777,
      okCount: 3,
      outputTokens: 760,
      totalTokens: 2000,
    })
    expect(summary.failureByStatus).toEqual({ '502': 1 })
    expect(summary.failureByKind).toEqual({ gateway_overload: 1 })
  })

  test('builds a public-safe artifact without prompts, URLs, or bearer-shaped fields', () => {
    const artifact = buildGlmLiveAdaptiveStressArtifact({
      demandClient: 'issue6317-adaptive-20260627T1410Z',
      durationMs: 120000,
      finalConcurrency: 2,
      generatedAt: '2026-06-27T14:10:00.000Z',
      initialConcurrency: 6,
      maxConcurrency: 9,
      maxTokens: 512,
      minConcurrency: 2,
      model: 'openagents/khala',
      observations: [okObservation('glm-1', 1100), overloadObservation('bad-1')],
      runId: 'issue6317-adaptive-20260627T1410Z',
      windowMs: 60000,
      windows: [
        {
          completedAt: '2026-06-27T14:11:00.000Z',
          decision: decideGlmLiveAdaptiveStressConcurrency({
            cleanWindowIncreaseThreshold: 3,
            consecutiveCleanWindows: 0,
            currentConcurrency: 6,
            maxConcurrency: 9,
            minConcurrency: 2,
            observations: [okObservation('glm-1', 1100), overloadObservation('bad-1')],
          }),
          startedAt: '2026-06-27T14:10:00.000Z',
        },
      ],
    })

    expect(artifact.schema).toBe(GLM_LIVE_ADAPTIVE_STRESS_ARTIFACT_SCHEMA)
    expect(artifact.telemetrySchema).toBe(GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA)
    expect(artifact.publicSafe).toBe(true)
    expect(artifact.summary.totalTokens).toBe(1100)
    expect(JSON.stringify(artifact)).not.toMatch(
      /prompt|completion|bearer|secret|https?:\/\//i,
    )
  })

  test('classifies bounded curl/http failures for public artifacts', () => {
    expect(classifyGlmLiveAdaptiveStressFailure(502, 0)).toBe(
      'gateway_overload',
    )
    expect(classifyGlmLiveAdaptiveStressFailure(500, 0)).toBe(
      'provider_overload',
    )
    expect(classifyGlmLiveAdaptiveStressFailure(429, 0)).toBe('rate_limited')
    expect(classifyGlmLiveAdaptiveStressFailure(null, 28)).toBe('timeout')
  })

  test('supervisor dispatches the next continuous window with public telemetry only', () => {
    const previousArtifact = buildGlmLiveAdaptiveStressArtifact({
      demandClient: 'issue6317-continuous',
      durationMs: 60_000,
      finalConcurrency: 3,
      generatedAt: '2026-06-27T15:00:00.000Z',
      initialConcurrency: 2,
      maxConcurrency: 6,
      maxTokens: 512,
      minConcurrency: 2,
      model: 'openagents/khala',
      observations: [okObservation('glm-clean-1', 1200)],
      runId: 'issue6317-continuous-0001',
      windowMs: 60_000,
      windows: [],
    })

    const tick = buildGlmLiveAdaptiveStressSupervisorTick({
      cadenceMs: 60_000,
      externalDemandActive: false,
      generatedAt: '2026-06-27T15:01:00.000Z',
      initialConcurrency: 2,
      maxConcurrency: 6,
      maxTicks: 10,
      minConcurrency: 2,
      model: 'openagents/khala',
      previousArtifact,
      runIdPrefix: 'issue6317-continuous',
      tickIndex: 1,
    })

    expect(tick).toMatchObject({
      action: 'dispatch_next_window',
      demandKind: GLM_STRESS_DEMAND_KIND,
      demandSource: GLM_STRESS_DEMAND_SOURCE,
      nextEarliestStartAt: '2026-06-27T15:02:00.000Z',
      nextRunId: 'issue6317-continuous-0002',
      previousFinalConcurrency: 3,
      previousRunId: 'issue6317-continuous-0001',
      publicSafe: true,
      recommendedInitialConcurrency: 3,
      schema: GLM_LIVE_ADAPTIVE_STRESS_SUPERVISOR_SCHEMA,
      telemetrySchema: GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA,
    })
    expect(tick.previousSummary).toMatchObject({
      failedCount: 0,
      okCount: 1,
      totalTokens: 1200,
    })
    expect(tick.reasonRefs).toEqual([
      'supervisor.glm_live_adaptive_stress.continuous_window_ready',
      'supervisor.glm_live_adaptive_stress.previous_window_clean',
    ])
    expect(JSON.stringify(tick)).not.toMatch(
      /prompt|completion|bearer|secret|https?:\/\//i,
    )
  })

  test('supervisor yields instantly to external demand and keeps stress at the floor', () => {
    const previousArtifact = buildGlmLiveAdaptiveStressArtifact({
      demandClient: 'issue6317-continuous',
      durationMs: 60_000,
      finalConcurrency: 5,
      generatedAt: '2026-06-27T15:00:00.000Z',
      initialConcurrency: 5,
      maxConcurrency: 6,
      maxTokens: 512,
      minConcurrency: 2,
      model: 'openagents/khala',
      observations: [overloadObservation('failed-before-yield')],
      runId: 'issue6317-continuous-0003',
      windowMs: 60_000,
      windows: [],
    })

    const tick = buildGlmLiveAdaptiveStressSupervisorTick({
      cadenceMs: 60_000,
      externalDemandActive: true,
      generatedAt: '2026-06-27T15:04:00.000Z',
      initialConcurrency: 5,
      maxConcurrency: 6,
      maxTicks: 10,
      minConcurrency: 2,
      model: 'openagents/khala',
      previousArtifact,
      runIdPrefix: 'issue6317-continuous',
      tickIndex: 4,
    })

    expect(tick).toMatchObject({
      action: 'yield_to_external',
      nextRunId: null,
      recommendedInitialConcurrency: 2,
    })
    expect(tick.reasonRefs).toEqual([
      'supervisor.glm_live_adaptive_stress.external_demand_active',
      'supervisor.glm_live_adaptive_stress.previous_backoff_carried_forward',
    ])
  })

  test('supervisor completes after the configured bounded tick budget', () => {
    const tick = buildGlmLiveAdaptiveStressSupervisorTick({
      cadenceMs: 60_000,
      externalDemandActive: false,
      generatedAt: '2026-06-27T15:10:00.000Z',
      initialConcurrency: 2,
      maxConcurrency: 6,
      maxTicks: 10,
      minConcurrency: 2,
      model: 'openagents/khala',
      runIdPrefix: 'issue6317-continuous',
      tickIndex: 10,
    })

    expect(tick).toMatchObject({
      action: 'complete',
      nextEarliestStartAt: null,
      nextRunId: null,
      previousRunId: null,
      recommendedInitialConcurrency: 2,
    })
    expect(tick.reasonRefs).toEqual([
      'supervisor.glm_live_adaptive_stress.max_ticks_completed',
    ])
  })
})
