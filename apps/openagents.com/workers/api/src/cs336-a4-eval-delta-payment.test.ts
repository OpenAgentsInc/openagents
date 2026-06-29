import { describe, expect, it } from 'vitest'

import {
  Cs336A4EvalDeltaBonusPolicyRef,
  Cs336A4EvalDeltaMeasurementRef,
  Cs336A4EvalDeltaPaymentSchemaVersion,
  Cs336A4EvalDeltaPaymentValidationError,
  settleCs336A4EvalDeltaPayment,
  type Cs336A4EvalDeltaPaymentInput,
} from './cs336-a4-eval-delta-payment'

const measurement = {
  baselineScore: 0.4,
  filteredScore: 0.5,
  fixedReferenceModelRef: 'config.cs336_a4.fixed_reference_trainer.v1',
  heldOutEvalSetRef: 'eval.cs336_a4.held_out.v1',
  sourceRef: 'source.psion.bounded_synthetic_mixture.v1',
}

const verifiedInput: Cs336A4EvalDeltaPaymentInput = {
  assignmentRef: 'assignment.cs336_a4.shard.1',
  measurement,
  stageRecomputeVerified: true,
}

describe('CS336 A4 eval-delta payment settlement', () => {
  it('blocks payment when operator funding parameters are unset', () => {
    const settlement = settleCs336A4EvalDeltaPayment(verifiedInput)

    expect(settlement.payable).toBe(false)
    if (settlement.payable) throw new Error('unreachable')
    expect(settlement.reason).toBe('funding_parameters_unset')
    expect(settlement.bonusPolicyRef).toBe(Cs336A4EvalDeltaBonusPolicyRef)
    expect(settlement.blockerRefs).toContain(
      'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
    )
    expect(settlement.schemaVersion).toBe(Cs336A4EvalDeltaPaymentSchemaVersion)
    expect(settlement.measurementRef).toBe(Cs336A4EvalDeltaMeasurementRef)
    // Honest measured delta is still reported even when blocked.
    expect(settlement.measuredDelta).toBeCloseTo(0.1, 10)
  })

  it('blocks payment when the producing stage is not recompute-verified', () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      stageRecomputeVerified: false,
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })

    expect(settlement.payable).toBe(false)
    if (settlement.payable) throw new Error('unreachable')
    expect(settlement.reason).toBe('stage_recompute_unverified')
  })

  it('pays no bonus for neutral filtering (delta == 0)', () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      measurement: { ...measurement, filteredScore: 0.4 },
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })

    expect(settlement.payable).toBe(false)
    if (settlement.payable) throw new Error('unreachable')
    expect(settlement.reason).toBe('delta_not_positive')
    expect(settlement.measuredDelta).toBe(0)
  })

  it('pays no bonus for a quality regression (delta < 0)', () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      measurement: { ...measurement, filteredScore: 0.3 },
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })

    expect(settlement.payable).toBe(false)
    if (settlement.payable) throw new Error('unreachable')
    expect(settlement.reason).toBe('delta_not_positive')
    expect(settlement.measuredDelta).toBeCloseTo(-0.1, 10)
  })

  it('settles a positive delta with funding using the documented formula', () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })

    expect(settlement.payable).toBe(true)
    if (!settlement.payable) throw new Error('unreachable')
    // round(clamp(0.1, 0, 1) * 1000) == 100
    expect(settlement.settledBonusSats).toBe(100)
    expect(settlement.clampedDelta).toBeCloseTo(0.1, 10)
    expect(settlement.bonusPolicyRef).toBe(
      'policy.cs336_a4.eval_delta_quality_bonus_settled',
    )
  })

  it('clamps a large delta to the funding cap before pricing', () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      measurement: { ...measurement, filteredScore: 5 },
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 0.5 },
    })

    expect(settlement.payable).toBe(true)
    if (!settlement.payable) throw new Error('unreachable')
    expect(settlement.clampedDelta).toBe(0.5)
    expect(settlement.settledBonusSats).toBe(500)
  })

  it('is deterministic: same input yields the same settlement', () => {
    const input: Cs336A4EvalDeltaPaymentInput = {
      ...verifiedInput,
      fundingParameters: { bonusRateSatsPerUnit: 1234, deltaCap: 1 },
    }

    expect(settleCs336A4EvalDeltaPayment(input)).toEqual(
      settleCs336A4EvalDeltaPayment(input),
    )
  })

  it('blocks when funding parameters are present but non-positive', () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      fundingParameters: { bonusRateSatsPerUnit: 0, deltaCap: 1 },
    })

    expect(settlement.payable).toBe(false)
    if (settlement.payable) throw new Error('unreachable')
    expect(settlement.reason).toBe('funding_parameters_unset')
  })

  it('rejects an empty assignment ref', () => {
    expect(() =>
      settleCs336A4EvalDeltaPayment({ ...verifiedInput, assignmentRef: '  ' }),
    ).toThrow(Cs336A4EvalDeltaPaymentValidationError)
  })

  it('rejects a non-finite measured score', () => {
    expect(() =>
      settleCs336A4EvalDeltaPayment({
        ...verifiedInput,
        measurement: { ...measurement, filteredScore: Number.NaN },
      }),
    ).toThrow(Cs336A4EvalDeltaPaymentValidationError)
  })
})
