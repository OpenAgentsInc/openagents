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
  buildGlmContinuousStressPlan,
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
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
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
        rolloutGuardEvidenceRef: 'evidence.glm.stress.rollout_guard.v1',
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
})
