import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_PAYOUT_TARGET_ADMISSION_CONFORMANCE_FIXTURES,
  PYLON_PAYOUT_TARGET_ADMISSION_READ_ONLY_AUTHORITY,
  PylonPayoutTargetAdmissionProjection,
  PylonPayoutTargetAdmissionRecord,
  PylonPayoutTargetAdmissionUnsafe,
  projectPylonPayoutTargetAdmission,
  pylonPayoutTargetAdmissionCanMutateProviderEligibility,
  pylonPayoutTargetAdmissionClaimAllowed,
  pylonPayoutTargetAdmissionHasNoSettlementAuthority,
  pylonPayoutTargetAdmissionProjectionHasPrivateMaterial,
} from './pylon-payout-target-admission'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T06:00:00.000Z'

const admissionRecord = (
  overrides: Partial<PylonPayoutTargetAdmissionRecord> = {},
): PylonPayoutTargetAdmissionRecord =>
  S.decodeUnknownSync(PylonPayoutTargetAdmissionRecord)({
    ...PYLON_PAYOUT_TARGET_ADMISSION_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon payout target admission projection', () => {
  test('decodes registered target fixtures and projects read-only operator state', () => {
    const record = admissionRecord()
    const projection = projectPylonPayoutTargetAdmission(
      record,
      'operator',
      nowIso,
    )

    expect(PYLON_PAYOUT_TARGET_ADMISSION_CONFORMANCE_FIXTURES.map(
      fixture => fixture.id,
    )).toEqual([
      'pylon_payout_target_admission.provider_1',
      'pylon_payout_target_admission.provider_2',
    ])
    expect(S.decodeUnknownSync(PylonPayoutTargetAdmissionRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(PylonPayoutTargetAdmissionProjection)(
      projection,
    )).toEqual(projection)
    expect(pylonPayoutTargetAdmissionHasNoSettlementAuthority(
      record.authority,
    )).toBe(true)
    expect(pylonPayoutTargetAdmissionCanMutateProviderEligibility(record))
      .toBe(false)
    expect(pylonPayoutTargetAdmissionClaimAllowed(record)).toBe(true)
    expect(projection.registeredPayoutTargetClaimAllowed).toBe(true)
    expect(projection.heartbeatHintOnly).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.payoutTargetDisclosureAllowed).toBe(false)
    expect(projection.providerEligibilityMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.createdAtDisplay).toBe('1 hour ago')
    expect(projection.updatedAtDisplay).toBe('15 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonPayoutTargetAdmissionProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps heartbeat-only hints separate from registered payout target claims', () => {
    const heartbeatOnly =
      PYLON_PAYOUT_TARGET_ADMISSION_CONFORMANCE_FIXTURES[1]!
    const projection = projectPylonPayoutTargetAdmission(
      heartbeatOnly,
      'operator',
      nowIso,
    )

    expect(pylonPayoutTargetAdmissionClaimAllowed(heartbeatOnly)).toBe(false)
    expect(projection.heartbeatHintOnly).toBe(true)
    expect(projection.registeredPayoutTargetClaimAllowed).toBe(false)
    expect(projection.targetFingerprintRef).toBeNull()
    expect(projection.blockerRefs).toEqual([
      'blocker.public.requires_ldk_v0_2_target',
    ])
  })

  test('redacts private owner, provider, registration, and verification refs from public projection', () => {
    const projection = projectPylonPayoutTargetAdmission(
      admissionRecord(),
      'public',
      nowIso,
    )

    expect(projection.ownerRef).toBe('owner.redacted')
    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.registrationRefs).toEqual([
      'registration.public.ldk_target_1',
    ])
    expect(projection.targetVerificationRefs).toEqual([
      'verification.public.ownership_signature_1',
    ])
    expect(projection.targetFingerprintRef).toBe(
      'target_hash.public.bolt12_abc123',
    )
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('requires state-specific evidence for pending, registered, rejected, revoked, stale, and missing states', () => {
    expect(() =>
      projectPylonPayoutTargetAdmission(
        admissionRecord({
          admissionState: 'pending_registration',
          registrationRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonPayoutTargetAdmissionUnsafe)

    expect(() =>
      projectPylonPayoutTargetAdmission(
        admissionRecord({
          admissionState: 'registered',
          targetVerificationRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonPayoutTargetAdmissionUnsafe)

    expect(() =>
      projectPylonPayoutTargetAdmission(
        admissionRecord({
          admissionState: 'rejected',
          rejectionRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonPayoutTargetAdmissionUnsafe)

    expect(() =>
      projectPylonPayoutTargetAdmission(
        admissionRecord({
          admissionState: 'revoked',
          revocationRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonPayoutTargetAdmissionUnsafe)

    expect(() =>
      projectPylonPayoutTargetAdmission(
        admissionRecord({
          admissionState: 'stale',
          staleRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonPayoutTargetAdmissionUnsafe)

    expect(() =>
      projectPylonPayoutTargetAdmission(
        admissionRecord({
          admissionState: 'missing',
          targetFingerprintRef: 'target_hash.public.should_not_exist',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonPayoutTargetAdmissionUnsafe)
  })

  test('rejects mutable authority and raw payout target, wallet, channel, provider, and credential material', () => {
    expect(() =>
      projectPylonPayoutTargetAdmission(
        admissionRecord({
          authority: {
            ...PYLON_PAYOUT_TARGET_ADMISSION_READ_ONLY_AUTHORITY,
            noPayoutTargetMutation: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonPayoutTargetAdmissionUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'bolt12 offer', value: 'lno1rawboltoffer' },
      { label: 'bolt11 invoice', value: 'lnbc1rawinvoice' },
      { label: 'lnurl', value: 'lnurl1rawtarget' },
      { label: 'payout address', value: 'payout_address.bc1qtest' },
      { label: 'channel monitor', value: 'channel_monitor.raw_state' },
      { label: 'provider secret', value: 'provider_secret.local_node' },
    ]) {
      expect(() =>
        projectPylonPayoutTargetAdmission(
          admissionRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(PylonPayoutTargetAdmissionUnsafe)
    }
  })
})
