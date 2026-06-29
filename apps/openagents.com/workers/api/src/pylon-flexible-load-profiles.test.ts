import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
  PylonFlexibleLoadProfileProjection,
  PylonFlexibleLoadProfileUnsafe,
  examplePylonFlexibleLoadProfile,
  projectPylonFlexibleLoadProfile,
  pylonFlexibleLoadProjectionHasPrivateMaterial,
} from './pylon-flexible-load-profiles'

const nowIso = '2026-06-06T22:10:00.000Z'

describe('Pylon flexible-load profiles', () => {
  test('projects modeled work-class flexibility with public-safe refs and friendly times', () => {
    const publicProjection = projectPylonFlexibleLoadProfile(
      examplePylonFlexibleLoadProfile(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonFlexibleLoadProfileProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      acceptedOutcomeClaimAllowed: false,
      capacityAssignmentMutationAllowed: false,
      checkpointCadence: 'per_step',
      checkpointCadenceLabel: 'Per step',
      deadlineWindow: 'overnight',
      deadlineWindowLabel: 'Overnight',
      flexibilityClass: 'interruptible',
      flexibilityClassLabel: 'Interruptible',
      interruptionTolerance: 'checkpoint_required',
      interruptionToleranceLabel: 'Checkpoint required',
      measuredSuitabilityClaimAllowed: false,
      modeledSuitabilityClaimAllowed: true,
      powerEventEligibility: 'eligible_modeled',
      powerEventEligibilityLabel: 'Eligible by model',
      powerEventDispatchAllowed: false,
      replayCost: 'low',
      replayCostLabel: 'Low',
      resumeRequirement: 'any_eligible_provider',
      resumeRequirementLabel: 'Any eligible provider',
      revenueClaimAllowed: false,
      settlementClaimAllowed: false,
      settlementMutationAllowed: false,
      updatedAtDisplay: '5 minutes ago',
      workClassMutationAllowed: false,
      workKind: 'autopilot_sites',
      workKindLabel: 'Autopilot Sites',
    })
    expect(JSON.stringify(publicProjection)).not.toContain(
      '2026-06-06T22:05:00.000Z',
    )
    expect(pylonFlexibleLoadProjectionHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('keeps modeled suitability, measured response, accepted outcomes, revenue, and settlement separate', () => {
    const base = examplePylonFlexibleLoadProfile()
    const measured = projectPylonFlexibleLoadProfile({
      ...base,
      measuredResponseRefs: ['measured.flex_response.event_1'],
      powerEventEligibility: 'eligible_measured',
    }, 'team', nowIso)
    const settled = projectPylonFlexibleLoadProfile({
      ...base,
      acceptedOutcomeRefs: ['accepted.outcome.site_revision_4'],
      measuredResponseRefs: ['measured.flex_response.event_1'],
      powerEventEligibility: 'eligible_measured',
      revenueRefs: ['revenue.accepted_work.site_revision_4'],
      settlementRefs: ['settlement.pylon_receipt.site_revision_4'],
    }, 'team', nowIso)

    expect(measured.modeledSuitabilityClaimAllowed).toBe(true)
    expect(measured.measuredSuitabilityClaimAllowed).toBe(true)
    expect(measured.acceptedOutcomeClaimAllowed).toBe(false)
    expect(measured.revenueClaimAllowed).toBe(false)
    expect(measured.settlementClaimAllowed).toBe(false)
    expect(settled.modeledSuitabilityClaimAllowed).toBe(true)
    expect(settled.measuredSuitabilityClaimAllowed).toBe(true)
    expect(settled.acceptedOutcomeClaimAllowed).toBe(true)
    expect(settled.revenueClaimAllowed).toBe(true)
    expect(settled.settlementClaimAllowed).toBe(true)
  })

  test('redacts audience-private evidence while keeping operator projection intact', () => {
    const profile = {
      ...examplePylonFlexibleLoadProfile(),
      acceptedOutcomeRefs: [
        'accepted.outcome.public_site_revision',
        'accepted.private.operator_review',
      ],
      measuredResponseRefs: [
        'measured.flex_response.public_event',
        'measured.private.operator_event',
      ],
      powerEventEligibility: 'eligible_measured' as const,
      revenueRefs: [
        'revenue.accepted_work.public_site_revision',
        'revenue.private.operator_margin',
      ],
      settlementRefs: [
        'settlement.pylon_receipt.public_site_revision',
        'settlement.private.operator_receipt',
      ],
    }
    const publicProjection = projectPylonFlexibleLoadProfile(
      profile,
      'public',
      nowIso,
    )
    const operatorProjection = projectPylonFlexibleLoadProfile(
      profile,
      'operator',
      nowIso,
    )

    expect(publicProjection.acceptedOutcomeRefs).toEqual([
      'accepted.outcome.public_site_revision',
    ])
    expect(publicProjection.measuredResponseRefs).toEqual([
      'measured.flex_response.public_event',
    ])
    expect(publicProjection.revenueRefs).toEqual([
      'revenue.accepted_work.public_site_revision',
    ])
    expect(publicProjection.settlementRefs).toEqual([
      'settlement.pylon_receipt.public_site_revision',
    ])
    expect(operatorProjection.acceptedOutcomeRefs).toContain(
      'accepted.private.operator_review',
    )
    expect(operatorProjection.measuredResponseRefs).toContain(
      'measured.private.operator_event',
    )
  })

  test('requires coherent flexibility class, checkpoint, resume, verification, modeled, measured, revenue, and settlement evidence', () => {
    const base = examplePylonFlexibleLoadProfile()
    const unsafeRecords = [
      {
        ...base,
        flexibilityClass: 'fixed' as const,
      },
      {
        ...base,
        flexibilityClass: 'interruptible' as const,
        interruptionTolerance: 'none' as const,
      },
      {
        ...base,
        deadlineWindow: 'immediate' as const,
        flexibilityClass: 'deferrable' as const,
      },
      { ...base, checkpointCadence: 'none' as const },
      { ...base, checkpointPolicyRefs: [] },
      { ...base, resumePolicyRefs: [] },
      { ...base, verificationPolicyRefs: [] },
      { ...base, modeledSuitabilityRefs: [] },
      {
        ...base,
        measuredResponseRefs: [],
        powerEventEligibility: 'eligible_measured' as const,
      },
      {
        ...base,
        acceptedOutcomeRefs: [],
        revenueRefs: ['revenue.accepted_work.orphan'],
      },
      {
        ...base,
        acceptedOutcomeRefs: ['accepted.outcome.site_revision_4'],
        revenueRefs: [],
        settlementRefs: ['settlement.pylon_receipt.site_revision_4'],
      },
    ]

    unsafeRecords.forEach(record => {
      expect(() =>
        projectPylonFlexibleLoadProfile(record, 'operator', nowIso),
      ).toThrow(PylonFlexibleLoadProfileUnsafe)
    })
  })

  test('rejects profiles that imply dispatch, runner, settlement, work-class, or claim mutation authority', () => {
    const base = examplePylonFlexibleLoadProfile()

    for (const authority of [
      {
        ...PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
        noCapacityAssignmentMutation: false,
      },
      {
        ...PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
        noPowerEventDispatch: false,
      },
      {
        ...PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
        noRunnerLaunch: false,
      },
      {
        ...PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
        noSettlementMutation: false,
      },
      {
        ...PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
        noWorkClassMutation: false,
      },
      {
        ...PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
        noPublicClaimUpgrade: false,
      },
    ]) {
      expect(() =>
        projectPylonFlexibleLoadProfile({
          ...base,
          authority,
        }, 'operator', nowIso),
      ).toThrow(PylonFlexibleLoadProfileUnsafe)
    }
  })

  test('rejects provider telemetry, private hardware, raw runner logs, wallet, payment, payout target, and raw timestamps', () => {
    const base = examplePylonFlexibleLoadProfile()
    const unsafeRecords = [
      { ...base, evidenceRefs: ['provider_telemetry.power_curve'] },
      { ...base, evidenceRefs: ['private_hardware.gpu_serial'] },
      { ...base, evidenceRefs: ['raw_runner_log.checkpoint'] },
      { ...base, caveatRefs: ['wallet_state.local_node'] },
      { ...base, revenueRefs: ['payment_id.raw_123'] },
      { ...base, settlementRefs: ['payout_target.raw_node'] },
      { ...base, checkpointPolicyRefs: ['checkpoint.2026-06-06T22:00:00Z'] },
    ]

    unsafeRecords.forEach(record => {
      expect(() =>
        projectPylonFlexibleLoadProfile(record, 'operator', nowIso),
      ).toThrow(PylonFlexibleLoadProfileUnsafe)
    })
  })
})
