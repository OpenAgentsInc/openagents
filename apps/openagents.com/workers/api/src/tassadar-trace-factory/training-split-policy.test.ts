import { describe, expect, test } from 'vitest'

import {
  splitAssignmentForRecord,
  splitPolicyViolations,
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
})
