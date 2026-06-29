/**
 * Training split policy v0.1 for the Tassadar verified trace factory
 * (issue #4748). Splits are designed against memorization:
 *
 *   - held-out program FAMILIES, never seeds — the economic-workload
 *     family and the psionic-compiled anchor never appear in training;
 *   - train-short / evaluate-long — training records are bounded at
 *     `trainMaxSteps`; evaluation runs the same families at 2x/4x/8x;
 *   - branch and memory stress suites name the train families whose
 *     long-horizon records form the stress evaluations;
 *   - near-miss lookup adversaries are a dedicated eval-only family.
 */
import type { TassadarTraceRecord } from './trace-record'
import { TASSADAR_TRACE_FAMILY_IDS, type TassadarTraceFamilyId } from './workload-families'

export const TASSADAR_TRAINING_SPLIT_POLICY_VERSION = 'training_split.v0.1'

export type TassadarTrainingSplitPolicy = Readonly<{
  policyVersion: typeof TASSADAR_TRAINING_SPLIT_POLICY_VERSION
  splitUnit: 'program_family'
  trainFamilies: ReadonlyArray<TassadarTraceFamilyId>
  heldOutFamilies: ReadonlyArray<TassadarTraceFamilyId>
  adversarialFamilies: ReadonlyArray<TassadarTraceFamilyId>
  economicFamily: TassadarTraceFamilyId
  trainMaxSteps: number
  evalLengthFactors: ReadonlyArray<number>
  stressSuites: ReadonlyArray<{
    suiteId: string
    familyId: TassadarTraceFamilyId
  }>
}>

export const TASSADAR_TRAINING_SPLIT_POLICY_V0_1: TassadarTrainingSplitPolicy =
  {
    adversarialFamilies: ['family.near_miss_lookup.v1'],
    economicFamily: 'family.application_state_machine.v1',
    evalLengthFactors: [2, 4, 8],
    heldOutFamilies: [
      'family.application_state_machine.v1',
      'family.stack_loop_sum.compiled.v1',
    ],
    policyVersion: TASSADAR_TRAINING_SPLIT_POLICY_VERSION,
    splitUnit: 'program_family',
    stressSuites: [
      { familyId: 'family.branch_gated_control.v1', suiteId: 'stress.branch.v1' },
      { familyId: 'family.memory_load_store.v1', suiteId: 'stress.memory.v1' },
    ],
    trainFamilies: [
      'family.arithmetic_carry.v1',
      'family.memory_load_store.v1',
      'family.branch_gated_control.v1',
    ],
    trainMaxSteps: 512,
  }

export type TassadarSplitAssignment =
  | 'train'
  | 'eval_heldout_family'
  | 'eval_long_horizon'
  | 'eval_adversarial'

export type TassadarSplitPolicyViolation = Readonly<
  | { kind: 'family_in_multiple_buckets'; familyId: string }
  | { kind: 'family_unassigned'; familyId: string }
  | { kind: 'economic_family_not_held_out'; familyId: string }
  | { kind: 'eval_factors_not_ascending'; detail: string }
>

/**
 * Structural invariants of any v0.1 split policy instance: every family
 * is assigned to exactly one bucket, the economic family is held out,
 * and the long-horizon factors escalate.
 */
export const splitPolicyViolations = (
  policy: TassadarTrainingSplitPolicy,
): ReadonlyArray<TassadarSplitPolicyViolation> => {
  const violations: Array<TassadarSplitPolicyViolation> = []
  const buckets: ReadonlyArray<ReadonlyArray<TassadarTraceFamilyId>> = [
    policy.trainFamilies,
    policy.heldOutFamilies,
    policy.adversarialFamilies,
  ]
  for (const familyId of TASSADAR_TRACE_FAMILY_IDS) {
    const memberships = buckets.filter(bucket =>
      bucket.includes(familyId),
    ).length
    if (memberships > 1) {
      violations.push({ familyId, kind: 'family_in_multiple_buckets' })
    }
    if (memberships === 0) {
      violations.push({ familyId, kind: 'family_unassigned' })
    }
  }
  if (!policy.heldOutFamilies.includes(policy.economicFamily)) {
    violations.push({
      familyId: policy.economicFamily,
      kind: 'economic_family_not_held_out',
    })
  }
  const ascending = policy.evalLengthFactors.every(
    (factor, index, factors) =>
      factor > 1 && (index === 0 || factor > (factors[index - 1] ?? 0)),
  )
  if (!ascending) {
    violations.push({
      detail: `eval length factors ${policy.evalLengthFactors.join(',')} must be > 1 and strictly ascending`,
      kind: 'eval_factors_not_ascending',
    })
  }

  return violations
}

/**
 * Routes one record to its split. Held-out family membership wins over
 * everything; adversarial families are eval-only; long-horizon records
 * of train families become the stress evaluations.
 */
export const splitAssignmentForRecord = (
  record: Pick<TassadarTraceRecord, 'familyId' | 'stepCount'>,
  policy: TassadarTrainingSplitPolicy = TASSADAR_TRAINING_SPLIT_POLICY_V0_1,
): TassadarSplitAssignment => {
  if (
    policy.heldOutFamilies.includes(record.familyId as TassadarTraceFamilyId)
  ) {
    return 'eval_heldout_family'
  }
  if (
    policy.adversarialFamilies.includes(
      record.familyId as TassadarTraceFamilyId,
    )
  ) {
    return 'eval_adversarial'
  }
  if (record.stepCount > policy.trainMaxSteps) {
    return 'eval_long_horizon'
  }

  return 'train'
}
