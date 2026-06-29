import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_ACCEPTED_WORK_PAYOUT_ROW_CONFORMANCE_FIXTURES,
  PYLON_ACCEPTED_WORK_PAYOUT_ROW_READ_ONLY_AUTHORITY,
  PylonAcceptedWorkPayoutRowProjection,
  PylonAcceptedWorkPayoutRowRecord,
  PylonAcceptedWorkPayoutRowUnsafe,
  projectPylonAcceptedWorkPayoutRow,
  pylonAcceptedWorkPayoutRowCanUpgradePublicClaim,
  pylonAcceptedWorkPayoutRowHasNoMutationAuthority,
  pylonAcceptedWorkPayoutRowProjectionHasPrivateMaterial,
} from './pylon-accepted-work-payout-rows'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T09:10:00.000Z'

const payoutRowRecord = (
  overrides: Partial<PylonAcceptedWorkPayoutRowRecord> = {},
): PylonAcceptedWorkPayoutRowRecord =>
  S.decodeUnknownSync(PylonAcceptedWorkPayoutRowRecord)({
    ...PYLON_ACCEPTED_WORK_PAYOUT_ROW_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon accepted-work payout rows', () => {
  test('decodes and projects settled public-safe payout rows without authority', () => {
    const record = payoutRowRecord()
    const projection = projectPylonAcceptedWorkPayoutRow(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonAcceptedWorkPayoutRowRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(PylonAcceptedWorkPayoutRowProjection)(
      projection,
    )).toEqual(projection)
    expect(pylonAcceptedWorkPayoutRowHasNoMutationAuthority(record.authority))
      .toBe(true)
    expect(pylonAcceptedWorkPayoutRowCanUpgradePublicClaim(record)).toBe(false)
    expect(projection.buyerChargeMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.payoutTargetMutationAllowed).toBe(false)
    expect(projection.publicClaimUpgradeAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection).toMatchObject({
      acceptedWorkClaimAllowed: true,
      confirmationClaimAllowed: true,
      dispatchClaimAllowed: true,
      eligibilityClaimAllowed: true,
      payoutBasisLabel: 'Accepted-work reward',
      payoutClassLabel: 'Settled payout',
      progressClassLabel: 'Settled',
      rewardIntentClaimAllowed: true,
      settlementClaimAllowed: true,
      settlementStateLabel: 'Settled',
      updatedAtDisplay: '5 minutes ago',
      verificationClaimAllowed: true,
      workClassLabel: 'Site build',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonAcceptedWorkPayoutRowProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('redacts private refs while keeping public links usable', () => {
    const projection = projectPylonAcceptedWorkPayoutRow(
      payoutRowRecord(),
      'public',
      nowIso,
    )

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.dispatchRefs).toEqual([
      'dispatch.public.site_otc_revision_3',
    ])
    expect(projection.confirmationRefs).toEqual([
      'confirmation.public.site_otc_revision_3',
    ])
    expect(projection.verificationRefs).toEqual([
      'verification.public.site_otc_revision_3',
    ])
    expect(projection.linkRefs).toEqual([
      'link.public.proof.otc',
      'link.public.site_order.otc',
    ])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('keeps modeled reward, eligibility, dispatch, verification, and settlement separate', () => {
    const base = payoutRowRecord()
    const modeled = projectPylonAcceptedWorkPayoutRow({
      ...base,
      confirmationRefs: [],
      dispatchRefs: [],
      eligibilityRefs: [],
      payoutClass: 'modeled_reward',
      progressClass: 'modeled',
      settlementRefs: [],
      settlementState: 'not_settled',
      verificationRefs: [],
    }, 'customer', nowIso)
    const eligible = projectPylonAcceptedWorkPayoutRow({
      ...base,
      confirmationRefs: [],
      dispatchRefs: [],
      payoutClass: 'payout_eligibility',
      progressClass: 'eligible',
      settlementRefs: [],
      settlementState: 'not_settled',
      verificationRefs: [],
    }, 'customer', nowIso)
    const dispatched = projectPylonAcceptedWorkPayoutRow({
      ...base,
      confirmationRefs: [],
      payoutClass: 'payout_dispatch',
      progressClass: 'dispatch_recorded',
      settlementRefs: [],
      settlementState: 'pending',
      verificationRefs: [],
    }, 'customer', nowIso)
    const verified = projectPylonAcceptedWorkPayoutRow({
      ...base,
      payoutClass: 'payout_verification',
      progressClass: 'verified',
      settlementRefs: [],
      settlementState: 'verified',
    }, 'customer', nowIso)

    expect(modeled.rewardIntentClaimAllowed).toBe(true)
    expect(modeled.eligibilityClaimAllowed).toBe(false)
    expect(modeled.settlementClaimAllowed).toBe(false)
    expect(eligible.eligibilityClaimAllowed).toBe(true)
    expect(eligible.dispatchClaimAllowed).toBe(false)
    expect(dispatched.dispatchClaimAllowed).toBe(true)
    expect(dispatched.verificationClaimAllowed).toBe(false)
    expect(verified.verificationClaimAllowed).toBe(true)
    expect(verified.settlementClaimAllowed).toBe(false)
  })

  test('requires evidence for progress and settlement states', () => {
    const base = payoutRowRecord()

    expect(() =>
      projectPylonAcceptedWorkPayoutRow({
        ...base,
        rewardIntentRefs: [],
        progressClass: 'modeled',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutRowUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutRow({
        ...base,
        eligibilityRefs: [],
        progressClass: 'eligible',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutRowUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutRow({
        ...base,
        dispatchRefs: [],
        progressClass: 'dispatch_recorded',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutRowUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutRow({
        ...base,
        verificationRefs: [],
        progressClass: 'verified',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutRowUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutRow({
        ...base,
        settlementRefs: [],
        progressClass: 'settled',
        settlementState: 'settled',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutRowUnsafe)
  })

  test('rejects mutable authority and unsafe raw payment or payout material', () => {
    const base = payoutRowRecord()

    expect(() =>
      projectPylonAcceptedWorkPayoutRow({
        ...base,
        authority: {
          ...PYLON_ACCEPTED_WORK_PAYOUT_ROW_READ_ONLY_AUTHORITY,
          noPublicClaimUpgrade: false,
        },
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutRowUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'payment id', value: 'payment_id.raw_internal' },
      { label: 'raw payout target', value: 'payout_target.raw_destination' },
      { label: 'invoice', value: 'invoice.lnbc123' },
      { label: 'preimage', value: 'payment_preimage.raw_secret' },
      { label: 'wallet material', value: 'wallet.secret.seed' },
      { label: 'provider token', value: 'provider_token.local' },
    ]) {
      expect(() =>
        projectPylonAcceptedWorkPayoutRow({
          ...base,
          evidenceRefs: [fixture.value],
        }, 'operator', nowIso),
      ).toThrow(PylonAcceptedWorkPayoutRowUnsafe)
    }
  })
})
