import { describe, expect, test } from 'vitest'

import {
  splitAssignmentForRecord,
  splitPolicyViolations,
  tassadarTraceCorpusUseDecision,
  TASSADAR_HELD_OUT_PARTITION_MANIFEST_SHA256,
  TASSADAR_HELD_OUT_PARTITION_REF,
  TASSADAR_TRAINING_SPLIT_POLICY_V0_1,
  type TassadarTrainingSplitPolicy,
} from './training-split-policy'

describe('training split policy v0.1', () => {
  test('the frozen policy has no structural violations', () => {
    expect(splitPolicyViolations(TASSADAR_TRAINING_SPLIT_POLICY_V0_1)).toEqual(
      [],
    )
  })

  test('the split unit is program families, never seeds', () => {
    expect(TASSADAR_TRAINING_SPLIT_POLICY_V0_1.splitUnit).toBe('program_family')
    expect(TASSADAR_TRAINING_SPLIT_POLICY_V0_1.heldOutFamilies.length).toBeGreaterThan(0)
  })

  test('held-out family records route to eval regardless of seed or length', () => {
    for (const stepCount of [16, 512, 4096]) {
      expect(
        splitAssignmentForRecord({
          familyId: 'family.application_state_machine.v1',
          stepCount,
        }),
      ).toBe('eval_heldout_family')
    }
    expect(
      splitAssignmentForRecord({
        familyId: 'family.stack_loop_sum.compiled.v1',
        stepCount: 80,
      }),
    ).toBe('eval_heldout_family')
  })

  test('the economic-workload family is held out by policy', () => {
    expect(TASSADAR_TRAINING_SPLIT_POLICY_V0_1.heldOutFamilies).toContain(
      TASSADAR_TRAINING_SPLIT_POLICY_V0_1.economicFamily,
    )
  })

  test('the GG partition is checksum-locked and exposed by ref only', () => {
    expect(
      TASSADAR_TRAINING_SPLIT_POLICY_V0_1.generalizationPartition,
    ).toMatchObject({
      exposure: 'checksum_only',
      manifestDigest: {
        algorithm: 'sha256',
        hex: TASSADAR_HELD_OUT_PARTITION_MANIFEST_SHA256,
      },
      partitionRef: TASSADAR_HELD_OUT_PARTITION_REF,
      purpose: 'generalization_gg_eval',
      rotation: 'append_new_partition_ref_never_rewrite',
    })
  })

  test('the GG partition is blocked from training, optimization, homework, and retrieval context', () => {
    for (const use of [
      'training',
      'optimization',
      'homework',
      'retrieval_context',
    ] as const) {
      expect(tassadarTraceCorpusUseDecision(use)).toMatchObject({
        allowed: false,
        blockerRef: 'blocker.tassadar_trace.gg_partition_isolated',
        partitionRef: TASSADAR_HELD_OUT_PARTITION_REF,
        use,
      })
    }
    expect(tassadarTraceCorpusUseDecision('generalization_eval')).toEqual({
      allowed: true,
      partitionRef: TASSADAR_HELD_OUT_PARTITION_REF,
      use: 'generalization_eval',
    })
  })

  test('adversarial near-miss records route to the adversarial eval', () => {
    expect(
      splitAssignmentForRecord({
        familyId: 'family.near_miss_lookup.v1',
        stepCount: 64,
      }),
    ).toBe('eval_adversarial')
  })

  test('train-short / evaluate-long: long-horizon train-family records become eval', () => {
    expect(
      splitAssignmentForRecord({
        familyId: 'family.arithmetic_carry.v1',
        stepCount: 512,
      }),
    ).toBe('train')
    expect(
      splitAssignmentForRecord({
        familyId: 'family.arithmetic_carry.v1',
        stepCount: 1024,
      }),
    ).toBe('eval_long_horizon')
  })

  test('stress suites name branch and memory families', () => {
    expect(
      TASSADAR_TRAINING_SPLIT_POLICY_V0_1.stressSuites.map(
        suite => suite.suiteId,
      ),
    ).toEqual(['stress.branch.v1', 'stress.memory.v1'])
  })

  test('a policy that holds out nothing or double-assigns a family is rejected with typed violations', () => {
    const broken: TassadarTrainingSplitPolicy = {
      ...TASSADAR_TRAINING_SPLIT_POLICY_V0_1,
      heldOutFamilies: [],
      trainFamilies: [
        ...TASSADAR_TRAINING_SPLIT_POLICY_V0_1.trainFamilies,
        'family.near_miss_lookup.v1',
      ],
    }
    const violations = splitPolicyViolations(broken)
    expect(
      violations.some(violation => violation.kind === 'family_unassigned'),
    ).toBe(true)
    expect(
      violations.some(
        violation => violation.kind === 'family_in_multiple_buckets',
      ),
    ).toBe(true)
    expect(
      violations.some(
        violation => violation.kind === 'economic_family_not_held_out',
      ),
    ).toBe(true)
  })

  test('a policy that exposes a mutable or non-checksummed GG partition is rejected', () => {
    const broken: TassadarTrainingSplitPolicy = {
      ...TASSADAR_TRAINING_SPLIT_POLICY_V0_1,
      generalizationPartition: {
        ...TASSADAR_TRAINING_SPLIT_POLICY_V0_1.generalizationPartition,
        blockedUses: ['homework'],
        exposure: 'checksum_only',
        manifestDigest: {
          algorithm: 'sha256',
          hex: 'not-a-digest',
        },
      },
    }
    const violations = splitPolicyViolations(broken)
    expect(
      violations.some(
        violation =>
          violation.kind === 'held_out_partition_not_checksum_locked',
      ),
    ).toBe(true)
    expect(
      violations.some(
        violation =>
          violation.kind === 'held_out_partition_training_use_allowed',
      ),
    ).toBe(true)
  })
})
