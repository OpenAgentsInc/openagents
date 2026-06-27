import { describe, expect, test } from 'vitest'

import {
  KHALA_GLM_PROVIDER_OBSERVED_SWEEP_CONFIG,
  SAMPLE_DECISION_SUITE_CONFIG,
  TINY_TEST_CONFIG,
} from './fixtures'
import type { BenchmarkMatrixConfig } from './matrix'
import {
  KHALA_ONLY_DECISION_SLICE,
  KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
} from './real-sweep-config'
import { preflightRealBenchmarkSweep } from './real-sweep-plan'

const REALISTIC_AVAILABLE_CONFIG: BenchmarkMatrixConfig = {
  ...TINY_TEST_CONFIG,
  id: 'realistic-available-test-v1',
  targets: [{ lane: 'fireworks', engine: 'provider-native' }],
  shapes: [
    {
      id: 'observed-code-artifact',
      inputTokens: 1400,
      outputTokens: 500,
      cacheablePrefixTokens: 900,
      concurrency: 2,
      provenance: 'realistic',
    },
  ],
  samplesPerCell: 3,
}

describe('real benchmark sweep preflight', () => {
  test('blocks unconfirmed or uncapped sweeps before any real seam can be armed', () => {
    const preflight = preflightRealBenchmarkSweep(TINY_TEST_CONFIG, {
      ownerConfirmed: false,
      maxBillableSamples: 100,
    })

    expect(preflight.canArmRealSeam).toBe(false)
    expect(preflight.decisionGradeEligible).toBe(false)
    expect(preflight.blockers.map(blocker => blocker.code)).toEqual([
      'owner_confirmation_missing',
      'owner_approval_ref_missing',
      'budget_cap_missing',
    ])
  })

  test('counts only available lane cells toward the billable sample upper bound', () => {
    const preflight = preflightRealBenchmarkSweep(TINY_TEST_CONFIG, {
      ownerConfirmed: true,
      ownerApprovalRef: 'owner-approved-real-sweep:test',
      budgetCapMsat: 10_000,
      maxBillableSamples: 4,
    })

    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.executableCells).toBe(1)
    expect(preflight.skippedFutureCells).toBe(1)
    expect(preflight.executableSampleUpperBound).toBe(4)
    expect(preflight.billableSampleUpperBound).toBe(4)
    expect(preflight.billableLanes).toEqual(['fireworks'])
    expect(preflight.warnings.map(warning => warning.code)).toContain(
      'future_lanes_skipped',
    )
  })

  test('synthetic traffic can arm a smoke but is not decision-grade', () => {
    const preflight = preflightRealBenchmarkSweep(TINY_TEST_CONFIG, {
      ownerConfirmed: true,
      ownerApprovalRef: 'owner-approved-real-sweep:test',
      budgetCapMsat: 10_000,
      maxBillableSamples: 4,
    })

    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.decisionGradeEligible).toBe(false)
    expect(preflight.syntheticShapes).toBe(1)
    expect(preflight.warnings.map(warning => warning.code)).toContain(
      'synthetic_traffic_not_decision_grade',
    )
  })

  test('realistic traffic with owner approval and caps is decision-grade eligible', () => {
    const preflight = preflightRealBenchmarkSweep(REALISTIC_AVAILABLE_CONFIG, {
      ownerConfirmed: true,
      ownerApprovalRef: 'owner-approved-real-sweep:test',
      budgetCapMsat: 10_000,
      maxBillableSamples: 3,
      trafficEvidence: [
        {
          shapeId: 'observed-code-artifact',
          evidenceRef:
            'receipt.public.khala_traffic_shape.observed_code_artifact',
          observedRequestCount: 7,
          source: 'gateway_telemetry',
          publicSafe: true,
        },
      ],
    })

    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.decisionGradeEligible).toBe(true)
    expect(preflight.blockers).toEqual([])
    expect(preflight.warnings).toEqual([])
    expect(preflight.executableSampleUpperBound).toBe(3)
    expect(preflight.billableSampleUpperBound).toBe(3)
    expect(preflight.realTrafficEvidenceRefs).toEqual([
      'receipt.public.khala_traffic_shape.observed_code_artifact',
    ])
  })

  test('observed sweep config can carry public-safe traffic evidence inline', () => {
    const preflight = preflightRealBenchmarkSweep(
      KHALA_GLM_PROVIDER_OBSERVED_SWEEP_CONFIG,
      {
        ownerConfirmed: true,
        ownerApprovalRef: 'owner-approved-real-sweep:glm-provider-matrix',
        budgetCapMsat: 50_000,
        maxBillableSamples: 120,
      },
    )

    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.decisionGradeEligible).toBe(true)
    expect(preflight.realTrafficEvidenceRefs).toEqual([
      'evidence.openagents.token_usage_events.fireworks_mix.2026_06_25',
    ])
    expect(preflight.executableSampleUpperBound).toBe(120)
    expect(preflight.billableSampleUpperBound).toBe(60)
    expect(preflight.billableLanes).toEqual([
      'fireworks',
      'vertex-anthropic',
      'vertex-gemini',
    ])
    expect(preflight.warnings.map(warning => warning.code)).toEqual([
      'future_lanes_skipped',
    ])
  })

  test('separates total executable samples from billable samples for the full OQ5 suite', () => {
    const preflight = preflightRealBenchmarkSweep(
      KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
      {
        ownerConfirmed: true,
        ownerApprovalRef: 'owner-approved-real-sweep:oq5',
        budgetCapMsat: 500_000,
        maxBillableSamples: 240,
      },
    )

    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.decisionGradeEligible).toBe(true)
    expect(preflight.executableCells).toBe(64)
    expect(preflight.executableSampleUpperBound).toBe(320)
    expect(preflight.billableSampleUpperBound).toBe(240)
    expect(preflight.billableLanes).toEqual([
      'fireworks',
      'vertex-anthropic',
      'vertex-gemini',
    ])
  })

  test('does not count Khala-only own-capacity samples as billable or decision-grade eligible', () => {
    const preflight = preflightRealBenchmarkSweep(
      KHALA_ONLY_DECISION_SLICE,
      {
        ownerConfirmed: true,
        ownerApprovalRef: 'owner-approved-real-sweep:khala-only',
        budgetCapMsat: 1,
        maxBillableSamples: 1,
      },
    )

    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.decisionGradeEligible).toBe(false)
    expect(preflight.executableCells).toBe(16)
    expect(preflight.executableSampleUpperBound).toBe(80)
    expect(preflight.billableSampleUpperBound).toBe(0)
    expect(preflight.billableLanes).toEqual([])
  })

  test('can cap only the owner-armed billable lanes', () => {
    const preflight = preflightRealBenchmarkSweep(
      KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
      {
        ownerConfirmed: true,
        ownerApprovalRef: 'owner-approved-real-sweep:fireworks-only',
        budgetCapMsat: 500_000,
        maxBillableSamples: 80,
        billableLanes: ['fireworks'],
      },
    )

    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.executableSampleUpperBound).toBe(320)
    expect(preflight.billableSampleUpperBound).toBe(80)
    expect(preflight.billableLanes).toEqual(['fireworks'])
  })

  test('realistic traffic still needs observed Khala traffic evidence', () => {
    const preflight = preflightRealBenchmarkSweep(REALISTIC_AVAILABLE_CONFIG, {
      ownerConfirmed: true,
      ownerApprovalRef: 'owner-approved-real-sweep:test',
      budgetCapMsat: 10_000,
      maxBillableSamples: 3,
    })

    expect(preflight.canArmRealSeam).toBe(false)
    expect(preflight.decisionGradeEligible).toBe(false)
    expect(preflight.blockers.map(blocker => blocker.code)).toContain(
      'real_traffic_evidence_missing',
    )
  })

  test('traffic evidence must be public-safe and backed by observed requests', () => {
    const preflight = preflightRealBenchmarkSweep(REALISTIC_AVAILABLE_CONFIG, {
      ownerConfirmed: true,
      ownerApprovalRef: 'owner-approved-real-sweep:test',
      budgetCapMsat: 10_000,
      maxBillableSamples: 3,
      trafficEvidence: [
        {
          shapeId: 'observed-code-artifact',
          evidenceRef:
            'receipt.public.khala_traffic_shape.observed_code_artifact',
          observedRequestCount: 0,
          source: 'operator_export',
          publicSafe: false,
        },
      ],
    })

    expect(preflight.canArmRealSeam).toBe(false)
    expect(preflight.blockers.map(blocker => blocker.code)).toContain(
      'real_traffic_evidence_invalid',
    )
  })

  test('sample cap is enforced against the expanded matrix before spend', () => {
    const preflight = preflightRealBenchmarkSweep(
      SAMPLE_DECISION_SUITE_CONFIG,
      {
        ownerConfirmed: true,
        ownerApprovalRef: 'owner-approved-real-sweep:test',
        budgetCapMsat: 10_000,
        maxBillableSamples: 119,
      },
    )

    expect(preflight.canArmRealSeam).toBe(false)
    expect(preflight.blockers.map(blocker => blocker.code)).toContain(
      'billable_sample_cap_exceeded',
    )
  })

  test('non-finite sample cap is rejected explicitly', () => {
    const preflight = preflightRealBenchmarkSweep(TINY_TEST_CONFIG, {
      ownerConfirmed: true,
      ownerApprovalRef: 'owner-approved-real-sweep:test',
      budgetCapMsat: 10_000,
      maxBillableSamples: Number.NaN,
    })

    expect(preflight.canArmRealSeam).toBe(false)
    expect(preflight.blockers.map(blocker => blocker.code)).toContain(
      'billable_sample_cap_missing',
    )
  })
})
