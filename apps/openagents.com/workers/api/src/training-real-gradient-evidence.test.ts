import { describe, expect, it } from 'vitest'

import {
  buildTrainingRunRecord,
  publicTrainingRunSummary,
} from './training-run-window-authority'
import {
  Cs336A1RealGradientJobKind,
  RealGradientEvidenceValidationError,
  RealGradientUnsafeProjectionError,
  admitCs336A1RealGradientEvidence,
  type Cs336A1RealGradientEvidenceRequest,
} from './training-real-gradient-evidence'

const nowIso = '2026-06-11T10:00:00.000Z'

const baseRun = () =>
  buildTrainingRunRecord({
    makeId: () => 'a1-real-grad',
    nowIso,
    request: {
      promiseRef: 'pylon.first_real_model_training_run.v1',
      trainingRunRef: 'training.run.cs336.a1.real_gradient',
    },
  })

const baseRequest: Cs336A1RealGradientEvidenceRequest = {
  budgetLabel:
    'CS336 A1 validation loss at or below the uniform baseline ln(32) under the bounded 6-step compute budget.',
  budgetRef: 'budget.cs336_a1.real_grad.bounded_6nd_steps',
  evalRef: 'eval.cs336_a1.real_grad.validation_loss.step_6',
  freivaldsCommitmentRefs: ['commitment.cs336_a1.real_grad.merkle_root.1'],
  gradientCloseoutRefs: ['closeout.cs336_a1.real_grad.step_1_shard_0'],
  lossCurve: [
    { sourceRefs: ['eval.cs336_a1.real_grad.step_0'], step: 0, validationLoss: 3.51 },
    { sourceRefs: ['eval.cs336_a1.real_grad.step_1'], step: 1, validationLoss: 2.93 },
    { sourceRefs: ['eval.cs336_a1.real_grad.step_2'], step: 2, validationLoss: 2.54 },
  ],
  lossSourceRefs: ['route:/api/training/runs/training.run.cs336.a1.real_gradient'],
  maxValidationLoss: 3.4657359027997265,
  mergeRef: 'merge.cs336_a1.real_grad.aggregated_state.step_6',
  receiptRefs: ['approval.operator.20260611.focus_cs336_issue4678'],
  shardContributions: [
    {
      dataUnitCount: 768,
      deviceClassRef: 'device_class.apple_silicon_macos',
      gradientCommitmentRef: 'commitment.cs336_a1.real_grad.step_1_shard_0.sha256_abc',
      pylonRef: 'pylon.cs336.a1.device1',
      receiptRefs: ['receipt.nexus_pylon.settlement.assignment_step_1_shard_0'],
      shardIndex: 0,
      shardLoss: 3.52,
      sourceRefs: ['assignment.cs336_a1.real_grad.step_1_shard_0'],
      stepIndex: 0,
      verificationRefs: ['training.verification.challenge.shard_0'],
    },
    {
      dataUnitCount: 768,
      deviceClassRef: 'device_class.x86_64_linux',
      gradientCommitmentRef: 'commitment.cs336_a1.real_grad.step_1_shard_1.sha256_def',
      pylonRef: 'pylon.cs336.a1.device2',
      receiptRefs: ['receipt.nexus_pylon.settlement.assignment_step_1_shard_1'],
      shardIndex: 1,
      shardLoss: 3.51,
      sourceRefs: ['assignment.cs336_a1.real_grad.step_1_shard_1'],
      stepIndex: 0,
      verificationRefs: [],
    },
  ],
  sourceRefs: ['issue.github.openagents.4678'],
}

describe('CS336 A1 real-gradient evidence admission', () => {
  it('admits receipted two-device evidence under the exact projection key the authority reads', () => {
    const admitted = admitCs336A1RealGradientEvidence({
      nowIso,
      request: baseRequest,
      run: baseRun(),
    })
    const projection = JSON.parse(admitted.publicProjectionJson) as Record<
      string,
      unknown
    >
    const evidence = projection.realGradient as Record<string, unknown>

    expect(evidence.jobKind).toBe(Cs336A1RealGradientJobKind)
    expect(evidence.maxValidationLoss).toBe(baseRequest.maxValidationLoss)
    expect(evidence.mergeRef).toBe(baseRequest.mergeRef)
    expect(evidence.psionicLaneRef).toBe(
      'psion_cs336_a1_real_gradient_reference_v1',
    )

    const summary = publicTrainingRunSummary({
      challenges: [],
      leases: [],
      nowIso,
      run: admitted,
      windows: [],
    })

    expect(summary.realGradient.closeoutRequirement.satisfied).toBe(true)
    expect(summary.realGradient.lossUnderBudget).toMatchObject({
      finalValidationLoss: 2.54,
      maxValidationLoss: baseRequest.maxValidationLoss,
      satisfied: true,
    })
    expect(summary.realGradient.lossCurve.map(point => point.step)).toEqual([
      0, 1, 2,
    ])
  })

  it('rejects evidence without two distinct contributor devices', () => {
    const request: Cs336A1RealGradientEvidenceRequest = {
      ...baseRequest,
      shardContributions: baseRequest.shardContributions.map(shard => ({
        ...shard,
        pylonRef: 'pylon.cs336.a1.device1',
      })),
    }

    expect(() =>
      admitCs336A1RealGradientEvidence({ nowIso, request, run: baseRun() }),
    ).toThrow(RealGradientEvidenceValidationError)
  })

  it('rejects unreceipted shard contributions', () => {
    const request: Cs336A1RealGradientEvidenceRequest = {
      ...baseRequest,
      shardContributions: [
        baseRequest.shardContributions[0]!,
        { ...baseRequest.shardContributions[1]!, receiptRefs: [] },
      ],
    }

    expect(() =>
      admitCs336A1RealGradientEvidence({ nowIso, request, run: baseRun() }),
    ).toThrow(RealGradientEvidenceValidationError)
  })

  it('rejects a final validation loss above the declared budget', () => {
    const request: Cs336A1RealGradientEvidenceRequest = {
      ...baseRequest,
      lossCurve: [
        ...baseRequest.lossCurve.slice(0, 2),
        { sourceRefs: [], step: 2, validationLoss: 3.9 },
      ],
    }

    expect(() =>
      admitCs336A1RealGradientEvidence({ nowIso, request, run: baseRun() }),
    ).toThrow(RealGradientEvidenceValidationError)
  })

  it('rejects non-increasing loss-curve steps and missing commitment refs', () => {
    expect(() =>
      admitCs336A1RealGradientEvidence({
        nowIso,
        request: {
          ...baseRequest,
          lossCurve: [
            { sourceRefs: [], step: 1, validationLoss: 3.0 },
            { sourceRefs: [], step: 1, validationLoss: 2.9 },
          ],
        },
        run: baseRun(),
      }),
    ).toThrow(RealGradientEvidenceValidationError)

    expect(() =>
      admitCs336A1RealGradientEvidence({
        nowIso,
        request: { ...baseRequest, freivaldsCommitmentRefs: [] },
        run: baseRun(),
      }),
    ).toThrow(RealGradientEvidenceValidationError)

    expect(() =>
      admitCs336A1RealGradientEvidence({
        nowIso,
        request: { ...baseRequest, gradientCloseoutRefs: [] },
        run: baseRun(),
      }),
    ).toThrow(RealGradientEvidenceValidationError)
  })

  it('rejects wallet or payment material at admission time', () => {
    const request: Cs336A1RealGradientEvidenceRequest = {
      ...baseRequest,
      budgetLabel: 'budget with lnbc1invoicematerial inside',
    }

    expect(() =>
      admitCs336A1RealGradientEvidence({ nowIso, request, run: baseRun() }),
    ).toThrow(RealGradientUnsafeProjectionError)
  })
})
