import { describe, expect, test } from 'vitest'

import {
  GLM_52_REAP_POOL_TARGET,
  SAMPLE_DECISION_SUITE_CONFIG,
} from './fixtures'
import type { SequenceShape } from './matrix'
import {
  GLM_STRESS_DEMAND_CLIENT,
  GLM_STRESS_DEMAND_KIND,
  GLM_STRESS_DEMAND_SOURCE,
  GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD,
  buildGlmContinuousStressReport,
  buildGlmContinuousStressPlan,
  buildGlmContinuousStressRunnerPlan,
  evaluateGlmExternalWinsProbe,
  evaluateGlmExternalWinsOpenAgentsResponse,
} from './stress-saturation-plan'

const stressShape = (id: string, concurrency: number): SequenceShape => ({
  id,
  inputTokens: 2400,
  outputTokens: 1200,
  cacheablePrefixTokens: 1600,
  concurrency,
  provenance: 'synthetic',
  requestClass: 'interactive_stream',
  source: 'synthetic_fixture',
})

describe('GLM continuous stress saturation plan (#6317 prep slice)', () => {
  test('returns a typed blocker artifact until live scheduler evidence exists', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('opencode-burst', 8)],
      workloads: ['opencode-coding-task'],
    })

    expect(plan.state).toBe('blocked_missing_live_scheduler_evidence')
    if (plan.state !== 'blocked_missing_live_scheduler_evidence') {
      throw new Error('stress plan unexpectedly armed without scheduler evidence')
    }
    expect(plan.demandKind).toBe(GLM_STRESS_DEMAND_KIND)
    expect(plan.demandSource).toBe(GLM_STRESS_DEMAND_SOURCE)
    expect(plan.demandClient).toBe(GLM_STRESS_DEMAND_CLIENT)
    expect(plan.blockerRefs).toContain(
      'blocker.glm_continuous_stress.missing_live_headroom_evidence',
    )
    expect(plan.blockerRefs).toContain(
      'blocker.glm_continuous_stress.missing_external_wins_preemption_evidence',
    )
    expect(plan.blockerRefs).toContain(
      'blocker.glm_continuous_stress.missing_rollout_guard_evidence',
    )
  })

  test('keeps external-wins by refusing to arm when reserved headroom is consumed', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat', 'opencode-coding-task'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 4,
        reservedExternalSlots: 4,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'accepted',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: true,
      },
    })

    expect(plan.state).toBe('blocked_missing_live_scheduler_evidence')
    if (plan.state !== 'blocked_missing_live_scheduler_evidence') {
      throw new Error('stress plan unexpectedly armed with consumed headroom')
    }
    expect(plan.blockerRefs).toContain(
      'blocker.glm_continuous_stress.external_headroom_unavailable',
    )
  })

  test('arms only bounded internal_stress cells when headroom and evidence exist', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat', 'opencode-coding-task'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 10,
        reservedExternalSlots: 3,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'accepted',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: true,
      },
    })

    expect(plan.state).toBe('armed')
    if (plan.state !== 'armed') {
      throw new Error('stress plan did not arm with evidence and headroom')
    }
    expect(plan.demandKind).toBe('internal_stress')
    expect(plan.maxStressConcurrency).toBe(7)
    expect(plan.reservedExternalSlots).toBe(3)
    expect(plan.cells).toHaveLength(4)
    expect(plan.cells.every(cell => cell.lane === 'glm-52')).toBe(true)
    expect(plan.cells.every(cell => cell.laneAvailability === 'available')).toBe(
      true,
    )
  })

  test('stays blocked until #6318 external-wins proof is accepted', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat', 'opencode-coding-task'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 10,
        reservedExternalSlots: 3,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'blocked',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: true,
      },
    })

    expect(plan.state).toBe('blocked_missing_live_scheduler_evidence')
    if (plan.state !== 'blocked_missing_live_scheduler_evidence') {
      throw new Error('stress plan unexpectedly armed without #6318 acceptance')
    }
    expect(plan.reasons).toContain('external_wins_proof_not_accepted')
    expect(plan.blockerRefs).toContain(
      'blocker.glm_continuous_stress.external_wins_proof_not_accepted',
    )
  })

  test('stays blocked until #6320 rollout acceptance allows #6317 stress', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat', 'opencode-coding-task'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 10,
        reservedExternalSlots: 3,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'accepted',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: false,
      },
    })

    expect(plan.state).toBe('blocked_missing_live_scheduler_evidence')
    if (plan.state !== 'blocked_missing_live_scheduler_evidence') {
      throw new Error('stress plan unexpectedly armed without #6320 acceptance')
    }
    expect(plan.reasons).toContain(
      'throughput_rollout_not_accepted_for_stress',
    )
    expect(plan.blockerRefs).toContain(
      'blocker.glm_continuous_stress.throughput_rollout_not_accepted_for_stress',
    )
  })

  test('runner stays fail-closed when the gated stress plan is blocked', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('opencode-burst', 8)],
      workloads: ['opencode-coding-task'],
    })
    const runnerPlan = buildGlmContinuousStressRunnerPlan({
      generatedAt: '2026-06-26T00:00:00.000Z',
      tickRef: 'tick.glm.stress.blocked.v1',
      plan,
    })

    expect(runnerPlan.state).toBe('blocked')
    expect(runnerPlan.canDispatch).toBe(false)
    expect(runnerPlan.globalMaxConcurrency).toBe(0)
    expect(runnerPlan.dispatchCells).toEqual([])
    expect(runnerPlan.blockerRefs).toContain(
      'blocker.glm_continuous_stress.missing_external_wins_preemption_evidence',
    )
  })

  test('runner emits bounded public-safe internal_stress dispatch cells only after proof gates pass', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat', 'opencode-coding-task'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 10,
        reservedExternalSlots: 3,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'accepted',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: true,
      },
    })
    const runnerPlan = buildGlmContinuousStressRunnerPlan({
      generatedAt: '2026-06-26T00:00:00.000Z',
      tickRef: 'tick.glm.stress.ready.v1',
      plan,
    })

    expect(runnerPlan.state).toBe('ready')
    expect(runnerPlan.canDispatch).toBe(true)
    expect(runnerPlan.globalMaxConcurrency).toBe(7)
    expect(runnerPlan.reservedExternalSlots).toBe(3)
    expect(runnerPlan.evidenceRefs).toEqual([
      'evidence.glm.live_headroom.oracle.v1',
      'evidence.glm.external_wins.preemption.v1',
      'evidence.glm.stress.rollout_guard.v1',
    ])
    expect(runnerPlan.dispatchCells).toHaveLength(4)
    expect(
      runnerPlan.dispatchCells.every(
        cell =>
          cell.demandKind === GLM_STRESS_DEMAND_KIND &&
          cell.demandSource === GLM_STRESS_DEMAND_SOURCE &&
          cell.demandClient === GLM_STRESS_DEMAND_CLIENT &&
          cell.globalMaxConcurrency === 7 &&
          cell.requestHeaders['x-openagents-demand-kind'] ===
            GLM_STRESS_DEMAND_KIND &&
          cell.requestHeaders['x-openagents-client'] ===
            GLM_STRESS_DEMAND_CLIENT,
      ),
    ).toBe(true)
    expect(JSON.stringify(runnerPlan)).not.toMatch(
      /x-openagents-demand-client|prompt|completion|bearer|token|secret|https?:\/\//i,
    )
  })

  test('report uses the explicit measurement window for aggregate and goodput rates', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 10,
        reservedExternalSlots: 3,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'accepted',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: true,
      },
    })
    const runnerPlan = buildGlmContinuousStressRunnerPlan({
      generatedAt: '2026-06-26T00:00:00.000Z',
      tickRef: 'tick.glm.stress.report.v1',
      plan,
    })
    const firstCell = runnerPlan.dispatchCells[0]
    if (firstCell === undefined) {
      throw new Error('expected a dispatch cell for report test')
    }

    const report = buildGlmContinuousStressReport({
      generatedAt: '2026-06-26T00:00:01.000Z',
      runnerPlan,
      throughputMeasurementWindowMs: 3000,
      observations: [
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.1',
          status: 'ok',
          outputTokens: 1200,
          goodputTokens: 1100,
          wallClockMs: 3000,
          ttftMs: 120,
          interTokenLatencyP50Ms: 6,
          interTokenLatencyP90Ms: 18,
          interTokenLatencyP99Ms: 32,
        },
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.2',
          status: 'ok',
          outputTokens: 800,
          goodputTokens: 700,
          wallClockMs: 2000,
          ttftMs: 240,
          interTokenLatencyP50Ms: 8,
          interTokenLatencyP90Ms: 24,
          interTokenLatencyP99Ms: 40,
        },
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.3',
          status: 'preempted_for_external',
          outputTokens: 0,
          wallClockMs: 250,
        },
        {
          cellId: firstCell.cellId,
          status: 'deferred_no_headroom',
          outputTokens: 0,
          wallClockMs: 0,
        },
      ],
    })

    expect(report.runnerState).toBe('ready')
    expect(report.throughputMeasurementWindowMs).toBe(3000)
    expect(report.aggregateTokPerSecond).toBeCloseTo(2000 / 3, 6)
    expect(report.goodputTokPerSecond).toBe(600)
    expect(report.errorRate).toBe(0)
    expect(report.okCount).toBe(2)
    expect(report.preemptedCount).toBe(1)
    expect(report.deferredCount).toBe(1)
    expect(report.overloadFailureCount).toBe(0)
    expect(report.backoff).toMatchObject({
      action: 'hold',
      currentConcurrency: 7,
      recommendedNextConcurrency: 7,
      maxStressConcurrency: 7,
      errorRateBackoffThreshold: GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD,
      observedErrorRate: 0,
      overloadFailureCount: 0,
      reasonRefs: ['backoff.glm_continuous_stress.none'],
    })
    expect(report.replicaRefs).toEqual([
      'replica.glm.us-central1-a.1',
      'replica.glm.us-central1-a.2',
      'replica.glm.us-central1-a.3',
    ])
    expect(report.latencyMs.ttftMs).toEqual({
      p50: 120,
      p90: 240,
      p99: 240,
      mean: 180,
      sampleCount: 2,
    })
    expect(report.latencyMs.interTokenLatencyP90Ms).toEqual({
      p50: 18,
      p90: 24,
      p99: 24,
      mean: 21,
      sampleCount: 2,
    })
    expect(report.replicaRollups).toHaveLength(3)
    expect(report.replicaRollups[0]).toMatchObject({
      replicaRef: 'replica.glm.us-central1-a.1',
      aggregateTokPerSecond: 400,
      goodputTokPerSecond: 1100 / 3,
      outputTokens: 1200,
      goodputTokens: 1100,
      okCount: 1,
      deferredCount: 0,
      preemptedCount: 0,
      failedCount: 0,
    })
    expect(report.replicaRollups[0]?.latencyMs.ttftMs.sampleCount).toBe(1)
    expect(report.replicaRollups[1]).toMatchObject({
      replicaRef: 'replica.glm.us-central1-a.2',
      aggregateTokPerSecond: 800 / 3,
      goodputTokPerSecond: 700 / 3,
      outputTokens: 800,
      goodputTokens: 700,
      okCount: 1,
      deferredCount: 0,
      preemptedCount: 0,
      failedCount: 0,
    })
    expect(report.replicaRollups[1]?.latencyMs.ttftMs.sampleCount).toBe(1)
    expect(report.replicaRollups[2]).toMatchObject({
      replicaRef: 'replica.glm.us-central1-a.3',
      aggregateTokPerSecond: null,
      goodputTokPerSecond: null,
      outputTokens: 0,
      goodputTokens: 0,
      okCount: 0,
      deferredCount: 0,
      preemptedCount: 1,
      failedCount: 0,
    })
    expect(report.replicaRollups[2]?.latencyMs.ttftMs.sampleCount).toBe(0)
    expect(JSON.stringify(report)).not.toMatch(
      /prompt|completion|bearer|secret|https?:\/\//i,
    )
  })

  test('report leaves throughput rates unmeasured without a positive measurement window', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 10,
        reservedExternalSlots: 3,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'accepted',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: true,
      },
    })
    const runnerPlan = buildGlmContinuousStressRunnerPlan({
      generatedAt: '2026-06-26T00:00:00.000Z',
      tickRef: 'tick.glm.stress.report.unmeasured_window.v1',
      plan,
    })
    const firstCell = runnerPlan.dispatchCells[0]
    if (firstCell === undefined) {
      throw new Error('expected a dispatch cell for report test')
    }

    const report = buildGlmContinuousStressReport({
      generatedAt: '2026-06-26T00:00:01.000Z',
      runnerPlan,
      throughputMeasurementWindowMs: 0,
      observations: [
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.1',
          status: 'ok',
          outputTokens: 1200,
          goodputTokens: 1100,
          wallClockMs: 3000,
        },
      ],
    })

    expect(report.throughputMeasurementWindowMs).toBeNull()
    expect(report.aggregateTokPerSecond).toBeNull()
    expect(report.goodputTokPerSecond).toBeNull()
    expect(report.replicaRollups[0]).toMatchObject({
      aggregateTokPerSecond: null,
      goodputTokPerSecond: null,
      outputTokens: 1200,
      goodputTokens: 1100,
    })
  })

  test('report recommends a lower next concurrency when overload failures appear', () => {
    const plan = buildGlmContinuousStressPlan({
      matrixConfig: SAMPLE_DECISION_SUITE_CONFIG,
      target: GLM_52_REAP_POOL_TARGET,
      shapes: [stressShape('saturation-knee', 16)],
      workloads: ['chat'],
      headroom: {
        healthyReplicaCount: 10,
        aggregateAvailableSlots: 10,
        reservedExternalSlots: 3,
        externalDemandActive: false,
      },
      evidence: {
        liveHeadroomEvidenceRef: 'evidence.glm.live_headroom.oracle.v1',
        externalWinsPreemptionEvidenceRef:
          'evidence.glm.external_wins.preemption.v1',
        externalWinsProofStatus: 'accepted',
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
        throughputRolloutCanStartIssue6317Stress: true,
      },
    })
    const runnerPlan = buildGlmContinuousStressRunnerPlan({
      generatedAt: '2026-06-26T00:00:00.000Z',
      tickRef: 'tick.glm.stress.report.overload_window.v1',
      plan,
    })
    const firstCell = runnerPlan.dispatchCells[0]
    if (firstCell === undefined) {
      throw new Error('expected a dispatch cell for overload report test')
    }

    const report = buildGlmContinuousStressReport({
      generatedAt: '2026-06-26T00:00:02.000Z',
      runnerPlan,
      throughputMeasurementWindowMs: 2000,
      observations: [
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.1',
          status: 'ok',
          outputTokens: 1000,
          goodputTokens: 900,
          wallClockMs: 2000,
        },
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.2',
          status: 'ok',
          outputTokens: 1000,
          goodputTokens: 900,
          wallClockMs: 2000,
        },
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.3',
          status: 'failed',
          httpStatus: 502,
          failureKind: 'gateway_overload',
          outputTokens: 9999,
          wallClockMs: 500,
        },
        {
          cellId: firstCell.cellId,
          replicaRef: 'replica.glm.us-central1-a.4',
          status: 'failed',
          failureKind: 'timeout',
          outputTokens: 9999,
          wallClockMs: 10_000,
        },
      ],
    })

    expect(report.aggregateTokPerSecond).toBe(1000)
    expect(report.goodputTokPerSecond).toBe(900)
    expect(report.errorRate).toBe(0.5)
    expect(report.failedCount).toBe(2)
    expect(report.overloadFailureCount).toBe(2)
    expect(report.backoff).toMatchObject({
      action: 'decrease',
      currentConcurrency: 7,
      recommendedNextConcurrency: 5,
      maxStressConcurrency: 7,
      errorRateBackoffThreshold: GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD,
      observedErrorRate: 0.5,
      overloadFailureCount: 2,
    })
    expect(report.backoff.reasonRefs).toEqual([
      'backoff.glm_continuous_stress.error_rate_over_budget',
      'backoff.glm_continuous_stress.overload_failures_observed',
    ])
    expect(JSON.stringify(report)).not.toMatch(
      /prompt|completion|bearer|secret|https?:\/\//i,
    )
  })

  test('accepts external-wins proof only when preempted external demand stays on GLM primary', () => {
    const proof = evaluateGlmExternalWinsProbe({
      externalHttpStatus: 200,
      fallbackReason: null,
      schedulerPreemptionEvidenceRef:
        'scheduler.preemption.internal_stress.request_123',
      schedulerPreemptionTargetOutcome: 'preempted_yielded',
      servedLane: 'glm_primary',
      usageTotalTokens: 614,
    })

    expect(proof.status).toBe('accepted')
    expect(proof.blockerRefs).toEqual([])
    expect(proof.evidenceRefs).toEqual([
      'scheduler.preemption.internal_stress.request_123',
    ])
    expect(proof.usageTotalTokens).toBe(614)
    expect(JSON.stringify(proof)).not.toMatch(
      /prompt|completion|bearer|secret|https?:\/\//i,
    )
  })

  test('blocks #6318 proof when empty GLM content falls through to weaker fallback', () => {
    const proof = evaluateGlmExternalWinsProbe({
      externalHttpStatus: 200,
      fallbackReason: 'empty_assistant_content',
      schedulerPreemptionEvidenceRef:
        'scheduler.preemption.internal_stress.request_empty',
      schedulerPreemptionTargetOutcome: 'preempted_yielded',
      servedLane: 'weaker_fallback',
      usageTotalTokens: 614,
    })

    expect(proof.status).toBe('blocked')
    expect(proof.reasons).toEqual([
      'fallback_after_preemption',
      'served_lane_not_glm_primary',
      'empty_glm_content_after_preemption',
    ])
    expect(proof.blockerRefs).toEqual([
      'blocker.glm_external_wins.fallback_after_preemption',
      'blocker.glm_external_wins.served_lane_not_glm_primary',
      'blocker.glm_external_wins.empty_glm_content_after_preemption',
    ])
  })

  test('blocks #6318 proof when scheduler preemption evidence is missing', () => {
    const proof = evaluateGlmExternalWinsProbe({
      externalHttpStatus: 200,
      fallbackReason: null,
      servedLane: 'glm_primary',
    })

    expect(proof.status).toBe('blocked')
    expect(proof.reasons).toEqual(['missing_scheduler_preemption'])
    expect(proof.schedulerPreemptionTargetOutcome).toBe('missing')
  })

  test('derives accepted #6318 proof from the public OpenAgents response block', () => {
    const proof = evaluateGlmExternalWinsOpenAgentsResponse({
      externalHttpStatus: 200,
      body: {
        openagents: {
          served_model: 'openagents/glm-5.2-reap-504b',
          supply_lane: 'hydralisk',
          worker: 'hydralisk-vllm-glm-5p2-reap-504b',
          routing: {
            fallback_reason: null,
            scheduler_preemption: {
              evidence_ref:
                'scheduler.preemption.internal_stress.request_live',
              reason: 'external_preemption',
              target_demand_class: 'internal_stress',
              target_outcome: 'preempted_yielded',
            },
          },
        },
        usage: {
          total_tokens: 614,
        },
      },
    })

    expect(proof.status).toBe('accepted')
    expect(proof.servedLane).toBe('glm_primary')
    expect(proof.fallbackReason).toBeNull()
    expect(proof.schedulerPreemptionTargetOutcome).toBe('preempted_yielded')
    expect(proof.usageTotalTokens).toBe(614)
  })

  test('derives blocked #6318 proof when public response overflowed to a weaker lane', () => {
    const proof = evaluateGlmExternalWinsOpenAgentsResponse({
      externalHttpStatus: 200,
      body: {
        openagents: {
          served_model: 'accounts/fireworks/models/glm-4.5',
          supply_lane: 'fireworks',
          worker: 'fireworks',
          routing: {
            fallback_reason: 'empty_assistant_content',
            scheduler_preemption: {
              evidence_ref:
                'scheduler.preemption.internal_stress.request_fallback',
              reason: 'external_preemption',
              target_demand_class: 'internal_stress',
              target_outcome: 'preempted_yielded',
            },
          },
        },
        usage: {
          total_tokens: 614,
        },
      },
    })

    expect(proof.status).toBe('blocked')
    expect(proof.servedLane).toBe('weaker_fallback')
    expect(proof.reasons).toEqual([
      'fallback_after_preemption',
      'served_lane_not_glm_primary',
      'empty_glm_content_after_preemption',
    ])
  })
})
