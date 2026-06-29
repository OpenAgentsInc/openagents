import { describe, expect, it } from 'vitest'

import {
  Cs336A4DataRefineryJobKind,
  Cs336A4DataRefineryUnsafeProjectionError,
  Cs336A4HomeworkStages,
  Cs336A4PsionicLaneRef,
  buildCs336A4HomeworkPayload,
  cs336A4EvalDeltaPaymentPolicy,
  cs336A4NoSpendReadiness,
  cs336A4VerificationChallengeRequest,
} from './cs336-a4-data-refinery'

describe('CS336 A4 data-refinery homework', () => {
  it('builds public-safe deterministic payloads for the landed Psionic stages', () => {
    const payloads = Cs336A4HomeworkStages.map(stage =>
      buildCs336A4HomeworkPayload({
        assignmentRef: `assignment.cs336_a4.${stage}.1`,
        inputShardRef: `shard.public.cs336_a4.${stage}.1`,
        stage,
      }),
    )

    expect(payloads.map(payload => payload.stage)).toEqual([
      'pii_masking',
      'gopher_rules',
      'exact_line_dedup',
      'minhash_dedup',
    ])
    expect(
      payloads.every(payload => payload.jobKind === Cs336A4DataRefineryJobKind),
    ).toBe(true)
    expect(
      payloads.every(
        payload => payload.psionicLaneRef === Cs336A4PsionicLaneRef,
      ),
    ).toBe(true)
    expect(
      payloads.every(
        payload => payload.verificationClass === 'deterministic_recompute',
      ),
    ).toBe(true)
    expect(JSON.stringify(payloads)).not.toMatch(
      /mnemonic|raw[_-]?shard|secret|\/Users\//i,
    )
  })

  it('rejects unsafe public payload material', () => {
    expect(() =>
      buildCs336A4HomeworkPayload({
        assignmentRef: 'assignment.cs336_a4.bad',
        inputShardRef: '/Users/operator/private/raw-shard.warc',
        stage: 'pii_masking',
      }),
    ).toThrow(Cs336A4DataRefineryUnsafeProjectionError)
  })

  it('binds closeout evidence to deterministic recompute verification', () => {
    const request = cs336A4VerificationChallengeRequest({
      closeout: {
        assignmentRef: 'assignment.cs336_a4.gopher.1',
        inputShardRef: 'shard.public.cs336_a4.gopher.1',
        outputDigestRef: 'digest.cs336_a4.gopher.output',
        recomputedDigestRef: 'digest.cs336_a4.gopher.output',
        stage: 'gopher_rules',
        workerReceiptRef: 'receipt.cs336_a4.gopher.worker',
      },
      trainingRunRef: 'training.run.cs336.a4.data_refinery',
      windowRef: 'training.window.cs336.a4.gopher.1',
    })

    expect(request).toMatchObject({
      homeworkKind: 'admin_dispatched_homework',
      payload: {
        expectedDigestRef: 'digest.cs336_a4.gopher.output',
        recomputedDigestRef: 'digest.cs336_a4.gopher.output',
        stage: 'gopher_rules',
      },
      samplingPolicy: 'per_contribution',
      trainingRunRef: 'training.run.cs336.a4.data_refinery',
      verificationClass: 'deterministic_recompute',
    })
  })

  it('keeps eval-delta bonuses blocked until fixed-trainer eval and funding exist', () => {
    const readiness = cs336A4NoSpendReadiness()
    const policy = cs336A4EvalDeltaPaymentPolicy()

    expect(readiness.dispatchableStages.length).toBeGreaterThanOrEqual(3)
    expect(policy).toMatchObject({
      baseRatePolicyRef: 'policy.cs336_a4.pay_per_verified_shard_processed',
      bonusPolicyRef: 'policy.cs336_a4.eval_delta_quality_bonus_pending',
      qualityMeasurementRef:
        'measurement.cs336_a4.downstream_eval_delta_fixed_reference_model',
    })
    expect(policy.blockerRefs).toEqual([
      'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
      'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
      'blocker.cs336_a4.psionic_classifier_adapters_partial',
    ])
  })
})
