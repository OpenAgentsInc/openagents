import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_ACCEPTED_WORK_PAYOUT_SLO_CONFORMANCE_FIXTURES,
  PYLON_ACCEPTED_WORK_PAYOUT_SLO_READ_ONLY_AUTHORITY,
  PylonAcceptedWorkPayoutSloProjection,
  PylonAcceptedWorkPayoutSloRecord,
  PylonAcceptedWorkPayoutSloUnsafe,
  projectPylonAcceptedWorkPayoutSlo,
  pylonAcceptedWorkPayoutSloCanDispatchPayout,
  pylonAcceptedWorkPayoutSloHasNoMutationAuthority,
  pylonAcceptedWorkPayoutSloProjectionHasPrivateMaterial,
} from './pylon-accepted-work-payout-slo'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T08:10:00.000Z'

const payoutSloRecord = (
  overrides: Partial<PylonAcceptedWorkPayoutSloRecord> = {},
): PylonAcceptedWorkPayoutSloRecord =>
  S.decodeUnknownSync(PylonAcceptedWorkPayoutSloRecord)({
    ...PYLON_ACCEPTED_WORK_PAYOUT_SLO_CONFORMANCE_FIXTURES[0]!,
    ...overrides,
  })

describe('Pylon accepted-work payout SLO projection', () => {
  test('decodes fixtures and projects settled SLO state without mutation authority', () => {
    const record = payoutSloRecord()
    const projection = projectPylonAcceptedWorkPayoutSlo(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonAcceptedWorkPayoutSloRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(PylonAcceptedWorkPayoutSloProjection)(
      projection,
    )).toEqual(projection)
    expect(pylonAcceptedWorkPayoutSloHasNoMutationAuthority(record.authority))
      .toBe(true)
    expect(pylonAcceptedWorkPayoutSloCanDispatchPayout(record)).toBe(false)
    expect(projection.buyerChargeMutationAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.payoutTargetMutationAllowed).toBe(false)
    expect(projection.providerEligibilityMutationAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.dispatchLatencyMs).toBe(120_000)
    expect(projection.dispatchLatencyDisplay).toBe('2 min')
    expect(projection.confirmationLatencyMs).toBe(150_000)
    expect(projection.confirmationLatencyDisplay).toBe('3 min')
    expect(projection.dispatchRequestedClaimAllowed).toBe(true)
    expect(projection.dispatchRecordedClaimAllowed).toBe(true)
    expect(projection.confirmationObservedClaimAllowed).toBe(true)
    expect(projection.verificationCompleteClaimAllowed).toBe(true)
    expect(projection.settlementClaimAllowed).toBe(true)
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(pylonAcceptedWorkPayoutSloProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('redacts private provider, dispatch, confirmation, verification, settlement, and workroom refs from public projection', () => {
    const projection = projectPylonAcceptedWorkPayoutSlo(
      payoutSloRecord(),
      'public',
      nowIso,
    )

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.dispatchRecordRefs).toEqual([
      'dispatch.public.trace_summary',
    ])
    expect(projection.confirmationRefs).toEqual([
      'confirmation.public.trace_summary',
    ])
    expect(projection.verificationRefs).toEqual([
      'verification.public.trace_summary',
    ])
    expect(projection.workroomRefs).toEqual([])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('keeps accepted-work, payout progress, and settlement claims separate', () => {
    const base = payoutSloRecord()
    const dispatchRequested = projectPylonAcceptedWorkPayoutSlo({
      ...base,
      confirmationObservedAtIso: null,
      confirmationRefs: [],
      dispatchRecordedAtIso: null,
      dispatchRecordRefs: [],
      settledAtIso: null,
      settlementRefs: [],
      state: 'dispatch_requested',
      verificationCompletedAtIso: null,
      verificationRefs: [],
    }, 'customer', nowIso)
    const confirmationObserved = projectPylonAcceptedWorkPayoutSlo({
      ...base,
      settledAtIso: null,
      settlementRefs: [],
      state: 'confirmation_observed',
      verificationCompletedAtIso: null,
      verificationRefs: [],
    }, 'customer', nowIso)
    const failed = projectPylonAcceptedWorkPayoutSlo({
      ...base,
      confirmationObservedAtIso: null,
      confirmationRefs: [],
      failureRefs: ['failure.public.no_route'],
      freshness: 'stale',
      freshnessRefs: ['freshness.public.retry_needed'],
      settledAtIso: null,
      settlementRefs: [],
      state: 'failed',
      verificationCompletedAtIso: null,
      verificationRefs: [],
    }, 'customer', nowIso)

    expect(dispatchRequested.acceptedWorkClaimAllowed).toBe(true)
    expect(dispatchRequested.dispatchRequestedClaimAllowed).toBe(true)
    expect(dispatchRequested.dispatchRecordedClaimAllowed).toBe(false)
    expect(dispatchRequested.settlementClaimAllowed).toBe(false)
    expect(confirmationObserved.confirmationObservedClaimAllowed).toBe(true)
    expect(confirmationObserved.verificationCompleteClaimAllowed).toBe(false)
    expect(confirmationObserved.settlementClaimAllowed).toBe(false)
    expect(failed.attentionRequired).toBe(true)
    expect(failed.dispatchRequestedClaimAllowed).toBe(false)
    expect(failed.settlementClaimAllowed).toBe(false)
  })

  test('requires matching evidence for each terminal or attention state', () => {
    const base = payoutSloRecord()

    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        dispatchRequestRefs: [],
        state: 'dispatch_requested',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        dispatchRecordRefs: [],
        state: 'dispatch_recorded',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        confirmationRefs: [],
        state: 'confirmation_observed',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        verificationRefs: [],
        state: 'verification_complete',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        settlementRefs: [],
        state: 'settled',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        failureRefs: [],
        state: 'failed',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        skippedRefs: [],
        state: 'skipped',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        blockerRefs: [],
        state: 'blocked',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
  })

  test('rejects mutable authority, negative counts, negative latency, and unsafe payment material', () => {
    const base = payoutSloRecord()

    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        authority: {
          ...PYLON_ACCEPTED_WORK_PAYOUT_SLO_READ_ONLY_AUTHORITY,
          noPayoutDispatch: false,
        },
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        failedAttemptCount: -1,
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    expect(() =>
      projectPylonAcceptedWorkPayoutSlo({
        ...base,
        dispatchRecordedAtIso: '2026-06-07T07:59:00.000Z',
      }, 'operator', nowIso),
    ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'payment id', value: 'payment_id.raw_internal' },
      { label: 'payout target', value: 'payout_target.raw_destination' },
      { label: 'invoice', value: 'invoice.lnbc123' },
      { label: 'preimage', value: 'payment_preimage.raw_secret' },
      { label: 'wallet material', value: 'wallet.secret.seed' },
      { label: 'channel monitor', value: 'channel_monitor.raw_state' },
    ]) {
      expect(() =>
        projectPylonAcceptedWorkPayoutSlo({
          ...base,
          evidenceRefs: [fixture.value],
        }, 'operator', nowIso),
      ).toThrow(PylonAcceptedWorkPayoutSloUnsafe)
    }
  })
})
