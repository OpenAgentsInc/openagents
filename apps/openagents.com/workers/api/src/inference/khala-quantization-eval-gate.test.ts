import { describe, expect, it } from 'vitest'
import type { AcceptanceVerdict } from './acceptance-runner/verdict'
import {
  DEFAULT_QUANT_GATE_POLICY,
  RealQuantSweepNotArmedError,
  collectRealQuantSweepSamples,
  decisionGradeBlockersForRealQuantSweepEvidence,
  runQuantizationEvalGate,
  type QuantizationComparisonSample,
  type RealQuantSweepEvidence,
} from './khala-quantization-eval-gate'

// Build a deterministic EXECUTED acceptance verdict fixture. `verified` is the
// quality signal the gate counts; we vary it per-sample to set accepted rates.
const verdict = (verified: boolean): AcceptanceVerdict => ({
  kind: 'crossy_road_single_html',
  executed: true,
  rubricRef: 'rubric.khala_code.crossy_road.single_html.v1',
  checks: [
    {
      id: 'loads_without_errors',
      passed: verified,
      detail: verified ? 'loaded clean' : 'console error on load',
    },
  ],
  passedChecks: verified ? ['loads_without_errors'] : [],
  failedChecks: verified ? [] : ['loads_without_errors'],
  scalarReward: verified ? 1 : 0,
  verified,
  consoleErrors: verified ? [] : ['boom'],
  pageErrors: [],
})

// Build a paired comparison sample: same task, original vs quantized verdict +
// cost. The quantized cost is typically lower (the throughput win).
const sample = (input: {
  taskId: string
  originalVerified: boolean
  quantizedVerified: boolean
  originalCostBasisMsat: number
  quantizedCostBasisMsat: number
}): QuantizationComparisonSample => ({
  taskId: input.taskId,
  originalVerdict: verdict(input.originalVerified),
  quantizedVerdict: verdict(input.quantizedVerified),
  originalCostBasisMsat: input.originalCostBasisMsat,
  quantizedCostBasisMsat: input.quantizedCostBasisMsat,
})

// A 4-task baseline where the original precision passes 4/4 and is more expensive.
const heldQualitySet: ReadonlyArray<QuantizationComparisonSample> = [
  sample({ taskId: 't1', originalVerified: true, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 600 }),
  sample({ taskId: 't2', originalVerified: true, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 600 }),
  sample({ taskId: 't3', originalVerified: true, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 600 }),
  sample({ taskId: 't4', originalVerified: true, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 600 }),
]

const realSweepEvidence = (
  overrides: Partial<RealQuantSweepEvidence> = {},
): RealQuantSweepEvidence => ({
  schemaVersion: 'openagents.khala.real-quant-sweep-evidence.v1',
  evidenceRef: 'evidence:khala:quant:fp8:2026-06-23',
  ownerApprovalRef: 'owner-approval:khala-quant-fp8:cap-001',
  workloadRef: 'workload:khala-code:realistic:launch-p1-7',
  originalModelId: 'openagents/khala-code:original',
  quantizedModelId: 'openagents/khala-code:fp8',
  originalPrecision: 'unquantized',
  quantizedPrecision: 'fp8',
  quantizationBackend: 'vllm',
  quantizationBackendVersion: 'vllm:real-sweep-fixture-ref',
  sampleCount: heldQualitySet.length,
  acceptanceVerifierRef: 'verifier:khala-code:executed:receipt-001',
  latencyEvidenceRef: 'latency:khala-quant:receipt-001',
  costEvidenceRef: 'cost:khala-quant:receipt-001',
  publicSafeEvidenceRefs: ['receipt:khala-quant:public-closeout-001'],
  ...overrides,
})

describe('quantization eval gate — quality holds (book P1-7 / #6090)', () => {
  it('PASSES when the quantized lane holds the accepted-outcome rate', () => {
    const result = runQuantizationEvalGate({
      samples: heldQualitySet,
      scope: 'weights_only',
    })
    expect(result.passed).toBe(true)
    expect(result.reason).toBe('accepted_rate_held')
    expect(result.originalAcceptedRate).toBe(1)
    expect(result.quantizedAcceptedRate).toBe(1)
    expect(result.acceptedRateDeltaAbs).toBe(0)
    // Decision-grade defaults OFF — the fixture gate proves the LOGIC.
    expect(result.decisionGrade).toBe(false)
  })

  it('PASSES when quality improves', () => {
    const set = [
      sample({ taskId: 't1', originalVerified: false, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 600 }),
      sample({ taskId: 't2', originalVerified: true, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 600 }),
    ]
    const result = runQuantizationEvalGate({ samples: set, scope: 'weights_only' })
    expect(result.passed).toBe(true)
    expect(result.reason).toBe('accepted_rate_held')
    expect(result.acceptedRateDeltaAbs).toBeGreaterThan(0)
  })
})

describe('quantization eval gate — quality drops (the LOSS rule)', () => {
  it('FAILS when the accepted-outcome rate drops beyond the bound, no matter the cost win', () => {
    // Original 4/4 accepted; quantized 2/4 => a 0.5 absolute drop, way past the
    // 0.02 bound. Even though quantized is far cheaper, this is a LOSS.
    const set = [
      sample({ taskId: 't1', originalVerified: true, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 100 }),
      sample({ taskId: 't2', originalVerified: true, quantizedVerified: true, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 100 }),
      sample({ taskId: 't3', originalVerified: true, quantizedVerified: false, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 100 }),
      sample({ taskId: 't4', originalVerified: true, quantizedVerified: false, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 100 }),
    ]
    const result = runQuantizationEvalGate({ samples: set, scope: 'weights_only' })
    expect(result.passed).toBe(false)
    expect(result.reason).toBe('accepted_rate_dropped_beyond_bound')
    expect(result.acceptedRateDeltaAbs).toBeLessThan(0)
  })

  it('FAILS a small drop with NO sufficient cost-per-accepted improvement', () => {
    // Build a large set so a single-task drop is a SMALL absolute drop (1/100 =
    // 0.01, within the 0.02 bound) but with NO cost win (costs are equal).
    const set: Array<QuantizationComparisonSample> = []
    for (let i = 0; i < 100; i += 1) {
      const quantizedVerified = i !== 0 // task 0 fails on quantized only
      set.push(
        sample({
          taskId: `t${i}`,
          originalVerified: true,
          quantizedVerified,
          originalCostBasisMsat: 1000,
          quantizedCostBasisMsat: 1000, // no cost win
        }),
      )
    }
    const result = runQuantizationEvalGate({ samples: set, scope: 'weights_only' })
    expect(result.passed).toBe(false)
    expect(result.reason).toBe('accepted_rate_dropped_without_cost_win')
    // The drop is within the bound...
    expect(-(result.acceptedRateDeltaAbs ?? 0)).toBeLessThanOrEqual(
      DEFAULT_QUANT_GATE_POLICY.maxAcceptedRateDropAbs,
    )
  })

  it('PASSES a small drop that IS bought back by a cost-per-accepted improvement', () => {
    // Same small drop (1/100) but the quantized lane is much cheaper, so
    // cost-per-accepted-outcome improves past the 0.15 fractional threshold.
    const set: Array<QuantizationComparisonSample> = []
    for (let i = 0; i < 100; i += 1) {
      const quantizedVerified = i !== 0
      set.push(
        sample({
          taskId: `t${i}`,
          originalVerified: true,
          quantizedVerified,
          originalCostBasisMsat: 1000,
          quantizedCostBasisMsat: 400, // big cost win
        }),
      )
    }
    const result = runQuantizationEvalGate({ samples: set, scope: 'weights_only' })
    expect(result.passed).toBe(true)
    expect(result.reason).toBe('cost_per_accepted_improved_offsets_drop')
    expect(result.costPerAcceptedImprovementFrac).toBeGreaterThanOrEqual(
      DEFAULT_QUANT_GATE_POLICY.minCostPerAcceptedImprovementFrac,
    )
  })
})

describe('quantization eval gate — policy and edge cases', () => {
  it('FAILS an aggressive scope without the owner ack even when metrics hold', () => {
    const result = runQuantizationEvalGate({
      samples: heldQualitySet,
      scope: 'kv_cache',
    })
    expect(result.passed).toBe(false)
    expect(result.reason).toBe('aggressive_scope_requires_ack')
    expect(result.aggressiveScope).toBe(true)
  })

  it('PASSES an aggressive scope WITH the owner ack when metrics hold', () => {
    const result = runQuantizationEvalGate({
      samples: heldQualitySet,
      scope: 'kv_cache',
      aggressiveScopeAck: true,
    })
    expect(result.passed).toBe(true)
    expect(result.reason).toBe('accepted_rate_held')
  })

  it('FAILS with no comparison samples', () => {
    const result = runQuantizationEvalGate({ samples: [], scope: 'weights_only' })
    expect(result.passed).toBe(false)
    expect(result.reason).toBe('no_comparison_samples')
  })

  it('FAILS when the original precision produced no accepted outcomes (no baseline)', () => {
    const set = [
      sample({ taskId: 't1', originalVerified: false, quantizedVerified: false, originalCostBasisMsat: 1000, quantizedCostBasisMsat: 600 }),
    ]
    const result = runQuantizationEvalGate({ samples: set, scope: 'weights_only' })
    expect(result.passed).toBe(false)
    expect(result.reason).toBe('no_baseline_accepted_outcomes')
    // cost-per-accepted is null (zero accepted) — never a fabricated 0.
    expect(result.originalCostPerAcceptedMsat).toBeNull()
  })
})

describe('quantization eval gate — decision-grade real-sweep evidence', () => {
  it('downgrades a passing gate when decision-grade evidence is missing', () => {
    const result = runQuantizationEvalGate({
      samples: heldQualitySet,
      scope: 'weights_only',
      decisionGrade: true,
    })
    expect(result.passed).toBe(true)
    expect(result.decisionGrade).toBe(false)
    expect(result.realSweepEvidenceRef).toBeNull()
    expect(result.decisionGradeBlockers).toEqual([
      'real_sweep_evidence_missing',
    ])
  })

  it('marks a passing gate decision-grade only with complete real-sweep evidence', () => {
    const evidence = realSweepEvidence()
    const result = runQuantizationEvalGate({
      samples: heldQualitySet,
      scope: 'weights_only',
      decisionGrade: true,
      realSweepEvidence: evidence,
    })
    expect(result.passed).toBe(true)
    expect(result.decisionGrade).toBe(true)
    expect(result.realSweepEvidenceRef).toBe(evidence.evidenceRef)
    expect(result.decisionGradeBlockers).toEqual([])
  })

  it('blocks decision-grade promotion when evidence is not public-safe and matching', () => {
    const blockers = decisionGradeBlockersForRealQuantSweepEvidence({
      sampleCount: heldQualitySet.length,
      evidence: realSweepEvidence({
        ownerApprovalRef: '',
        originalPrecision: 'fp8',
        quantizedPrecision: 'fp8',
        publicSafeEvidenceRefs: [''],
        sampleCount: heldQualitySet.length + 1,
      }),
    })
    expect(blockers).toEqual([
      'owner_approval_ref_missing',
      'original_precision_not_full',
      'same_precision_compared',
      'public_safe_evidence_refs_missing',
      'sample_count_mismatch',
    ])
  })
})

describe('real quantized-vs-original sweep is FLAG/OWNER/COMPUTE-GATED off', () => {
  it('throws RealQuantSweepNotArmedError when not armed', () => {
    expect(() =>
      collectRealQuantSweepSamples({ armRealQuantSweep: false }),
    ).toThrow(RealQuantSweepNotArmedError)
  })

  it('throws even if armRealQuantSweep:true but no executor is provided', () => {
    expect(() =>
      collectRealQuantSweepSamples({ armRealQuantSweep: true }),
    ).toThrow(RealQuantSweepNotArmedError)
  })

  it('delegates to the executor only when explicitly owner-armed', () => {
    const samples = collectRealQuantSweepSamples({
      armRealQuantSweep: true,
      executor: () => heldQualitySet,
    })
    expect(samples).toHaveLength(heldQualitySet.length)
  })
})
