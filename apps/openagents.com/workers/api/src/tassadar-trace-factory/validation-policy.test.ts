import { describe, expect, test } from 'vitest'

import { buildTraceRecordFromExecution, withValidatorReceipts } from './record-factory'
import {
  admissionDecision,
  adversarialReplaySelection,
  generationAssignmentDigestViolations,
  receiptFromVerdict,
  requiredAdmissionTier,
  runTierOneFullReplay,
  runTierThreeAdversarialReplay,
  runTierTwoWindowSpotCheck,
  runTierZeroValidation,
  TASSADAR_VALIDATION_TIER_LADDER,
  trainingEligibility,
  type TassadarValidationContext,
  type TassadarWorkerStanding,
} from './validation-policy'
import { buildFamilyWorkload, type TassadarFamilyWorkload } from './workload-families'

const context: TassadarValidationContext = {
  validatedAtIso: '2026-06-11T00:00:00.000Z',
  validatorDeviceRef: 'device.local_pilot.validator',
}

const workloadFor = async (
  familyId: string,
  inputSeed = 'a1b2c3d4e5f60718',
  stepCount = 32,
): Promise<TassadarFamilyWorkload> => {
  const built = await buildFamilyWorkload({ familyId, inputSeed, stepCount })
  if (!built.ok) throw new globalThis.Error(built.failure.detail)

  return built.workload
}

describe('validator verdict v0.1 tier ladder', () => {
  test('the frozen ladder names all four tiers in escalation order', () => {
    expect(TASSADAR_VALIDATION_TIER_LADDER.map(policy => policy.tier)).toEqual([
      0, 1, 2, 3,
    ])
    expect(TASSADAR_VALIDATION_TIER_LADDER.map(policy => policy.method)).toEqual([
      'schema_hash',
      'full_replay',
      'window_spot_check',
      'adversarial_replay',
    ])
  })

  test('tier 0 verifies a real record and rejects a tampered token stream with a typed kind', async () => {
    const workload = await workloadFor('family.arithmetic_carry.v1')
    const record = await buildTraceRecordFromExecution(workload)
    const verdict = await runTierZeroValidation(record, context)
    expect(verdict.outcome).toBe('verified')
    expect(verdict.tier).toBe(0)

    const tamperedTokens = new Uint16Array(record.traceTokenIds)
    const original = tamperedTokens[10] ?? 0
    tamperedTokens[10] = (original + 1) & 0xffff
    const tampered = { ...record, traceTokenIds: tamperedTokens }
    const tamperedVerdict = await runTierZeroValidation(tampered, context)
    expect(tamperedVerdict.outcome).toBe('rejected')
    expect(tamperedVerdict.rejection?.kind).toBe('token_digest_mismatch')
  })

  test('tier 0 rejects an unknown profile with a typed kind', async () => {
    const workload = await workloadFor('family.memory_load_store.v1')
    const record = await buildTraceRecordFromExecution(workload)
    const verdict = await runTierZeroValidation(
      { ...record, profileVersion: 'profile.unknown.v9' },
      context,
    )
    expect(verdict.outcome).toBe('rejected')
    expect(verdict.rejection?.kind).toBe('profile_unknown')
  })

  test('tier 1 full replay verifies an honest record and rejects a forged digest', async () => {
    const workload = await workloadFor('family.application_state_machine.v1')
    const record = await buildTraceRecordFromExecution(workload)
    const verdict = await runTierOneFullReplay(record, workload, context)
    expect(verdict.outcome).toBe('verified')
    expect(verdict.replayedSteps).toBe(record.stepCount)

    const forged = {
      ...record,
      fullTraceDigest: record.fullTraceDigest.replace(/^./, character =>
        character === '0' ? '1' : '0',
      ),
    }
    const forgedVerdict = await runTierOneFullReplay(forged, workload, context)
    expect(forgedVerdict.outcome).toBe('rejected')
    expect(forgedVerdict.rejection?.kind).toBe('trace_digest_mismatch')
  })

  test('tier 1 rejects a workload whose program hash does not match the record', async () => {
    const workload = await workloadFor('family.branch_gated_control.v1')
    const other = await workloadFor('family.arithmetic_carry.v1')
    const record = await buildTraceRecordFromExecution(workload)
    const verdict = await runTierOneFullReplay(record, other, context)
    expect(verdict.outcome).toBe('rejected')
    expect(verdict.rejection?.kind).toBe('program_hash_mismatch')
  })

  test('tier 2 window spot-check and tier 3 adversarial replay carry their own tier labels', async () => {
    const workload = await workloadFor('family.near_miss_lookup.v1')
    const record = await buildTraceRecordFromExecution(workload)
    const tierTwo = await runTierTwoWindowSpotCheck(
      record,
      workload,
      { endStep: 16, startStep: 8 },
      context,
    )
    expect(tierTwo.tier).toBe(2)
    expect(tierTwo.outcome).toBe('verified')
    expect(tierTwo.comparedSteps).toBe(8)
    const tierThree = await runTierThreeAdversarialReplay(
      record,
      workload,
      context,
    )
    expect(tierThree.tier).toBe(3)
    expect(tierThree.method).toBe('adversarial_replay')
    expect(tierThree.outcome).toBe('verified')
  })

  test('adversarial selection is deterministic for a fixed seed', () => {
    const recordIds = Array.from({ length: 50 }, (_, index) => `trace_${index}`)
    const first = adversarialReplaySelection(recordIds, 'feedbeef', 7)
    const second = adversarialReplaySelection(recordIds, 'feedbeef', 7)
    expect(first).toEqual(second)
    expect(first.length).toBeGreaterThan(0)
    expect(first.length).toBeLessThan(recordIds.length)
  })
})

describe('iron rules as enforced invariants', () => {
  const newWorker: TassadarWorkerStanding = {
    isNewFamily: false,
    isNewProfile: false,
    isNewWorker: true,
    verifiedRecordCount: 0,
    workerRef: 'worker.new',
  }
  const establishedWorker: TassadarWorkerStanding = {
    isNewFamily: false,
    isNewProfile: false,
    isNewWorker: false,
    verifiedRecordCount: 250,
    workerRef: 'worker.established',
  }

  test('never train from unverified artifacts: a freshly built record is ineligible', async () => {
    const workload = await workloadFor('family.arithmetic_carry.v1')
    const record = await buildTraceRecordFromExecution(workload)
    expect(record.validatorReceipts).toHaveLength(0)
    const eligibility = trainingEligibility(record)
    expect(eligibility.eligible).toBe(false)
  })

  test('a tier 0 receipt alone never reaches training; tier 0 plus tier 1 does', async () => {
    const workload = await workloadFor('family.memory_load_store.v1')
    const record = await buildTraceRecordFromExecution(workload)
    const tierZero = receiptFromVerdict(
      await runTierZeroValidation(record, context),
    )
    const onlySchema = withValidatorReceipts(record, [tierZero])
    expect(trainingEligibility(onlySchema).eligible).toBe(false)

    const tierOne = receiptFromVerdict(
      await runTierOneFullReplay(record, workload, context),
    )
    const fullyVerified = withValidatorReceipts(record, [tierZero, tierOne])
    const eligibility = trainingEligibility(fullyVerified)
    expect(eligibility.eligible).toBe(true)
  })

  test('quarantine-before-admission: a new worker record stays quarantined until tier 1, an established worker needs tier >= 2', async () => {
    const workload = await workloadFor('family.branch_gated_control.v1')
    const record = await buildTraceRecordFromExecution(workload)
    expect(requiredAdmissionTier(newWorker)).toBe(1)
    expect(requiredAdmissionTier(establishedWorker)).toBe(2)

    expect(admissionDecision(record, newWorker).status).toBe('quarantined')

    const tierZero = receiptFromVerdict(
      await runTierZeroValidation(record, context),
    )
    const schemaOnly = withValidatorReceipts(record, [tierZero])
    expect(admissionDecision(schemaOnly, newWorker).status).toBe('quarantined')

    const tierOne = receiptFromVerdict(
      await runTierOneFullReplay(record, workload, context),
    )
    const replayVerified = withValidatorReceipts(record, [tierZero, tierOne])
    expect(admissionDecision(replayVerified, newWorker).status).toBe('admitted')
    expect(admissionDecision(replayVerified, establishedWorker).status).toBe(
      'quarantined',
    )
  })

  test('a rejected receipt rejects admission with typed kinds', async () => {
    const workload = await workloadFor('family.arithmetic_carry.v1')
    const record = await buildTraceRecordFromExecution(workload)
    const forged = {
      ...record,
      fullTraceDigest: record.fullTraceDigest.replace(/^./, character =>
        character === '0' ? '1' : '0',
      ),
    }
    const rejection = receiptFromVerdict(
      await runTierOneFullReplay(forged, workload, context),
    )
    const decided = admissionDecision(
      withValidatorReceipts(forged, [rejection]),
      newWorker,
    )
    expect(decided.status).toBe('rejected')
    if (decided.status === 'rejected') {
      expect(decided.rejectionKinds).toEqual(['trace_digest_mismatch'])
    }
  })

  test('expected digests never ship in generation assignments', () => {
    const clean = {
      assignmentRef: 'assignment.trace_factory.1',
      tassadar: {
        familyId: 'family.arithmetic_carry.v1',
        inputSeed: 'a1b2c3d4e5f60718',
        stepCount: 256,
      },
    }
    expect(generationAssignmentDigestViolations(clean)).toEqual([])

    const leaking = {
      assignmentRef: 'assignment.trace_factory.2',
      tassadar: {
        expectedTraceDigest: 'deadbeef',
        familyId: 'family.arithmetic_carry.v1',
        nested: [{ full_trace_digest: 'cafe' }],
      },
    }
    const violations = generationAssignmentDigestViolations(leaking)
    expect(violations.map(violation => violation.key).sort()).toEqual([
      'expectedTraceDigest',
      'full_trace_digest',
    ])
  })
})
