import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY,
  PylonFlexibleLoadEventProjection,
  PylonFlexibleLoadEventUnsafe,
  examplePylonFlexibleLoadEvent,
  projectPylonFlexibleLoadEvent,
  pylonFlexibleLoadEventProjectionHasPrivateMaterial,
} from './pylon-flexible-load-events'

const nowIso = '2026-06-06T23:10:00.000Z'

describe('Pylon flexible-load events', () => {
  test('projects settled event telemetry with response and lost-work summaries', () => {
    const projection = projectPylonFlexibleLoadEvent(
      examplePylonFlexibleLoadEvent(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonFlexibleLoadEventProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      acceptedWorkImpactClaimAllowed: true,
      actualResponseWatts: 180000,
      acknowledgementClaimAllowed: true,
      capacityDispatchAllowed: false,
      compensationClaimAllowed: true,
      createdAtDisplay: '10 minutes ago',
      executedClaimAllowed: true,
      gridServiceClaimUpgradeAllowed: false,
      liveWalletSpendAllowed: false,
      lostWorkCostCents: 250,
      measuredClaimAllowed: true,
      payoutDispatchAllowed: false,
      requestedClaimAllowed: true,
      requestedResponseWatts: 200000,
      responseRatioBps: 9000,
      settlementClaimAllowed: true,
      settlementMutationAllowed: false,
      state: 'settled',
      stateLabel: 'Settled',
      updatedAtDisplay: '5 minutes ago',
      verifiedClaimAllowed: true,
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(pylonFlexibleLoadEventProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps requested, acknowledged, executed, measured, verified, compensated, and settled states separate', () => {
    const base = examplePylonFlexibleLoadEvent()
    const requested = projectPylonFlexibleLoadEvent({
      ...base,
      acceptedWorkImpactRefs: [],
      acknowledgementRefs: [],
      actualResponseWatts: null,
      checkpointRefs: [],
      compensationRefs: [],
      evidenceRefs: [],
      executionRefs: [],
      interruptedWorkRefs: [],
      lostWorkCostCents: 0,
      measurementRefs: [],
      resumeRefs: [],
      settlementRefs: [],
      state: 'requested',
    }, 'team', nowIso)
    const measured = projectPylonFlexibleLoadEvent({
      ...base,
      compensationRefs: [],
      settlementRefs: [],
      state: 'measured',
    }, 'team', nowIso)
    const compensated = projectPylonFlexibleLoadEvent({
      ...base,
      settlementRefs: [],
      state: 'compensated',
    }, 'team', nowIso)

    expect(requested.requestedClaimAllowed).toBe(true)
    expect(requested.acknowledgementClaimAllowed).toBe(false)
    expect(requested.executedClaimAllowed).toBe(false)
    expect(requested.measuredClaimAllowed).toBe(false)
    expect(requested.settlementClaimAllowed).toBe(false)
    expect(measured.executedClaimAllowed).toBe(true)
    expect(measured.measuredClaimAllowed).toBe(true)
    expect(measured.verifiedClaimAllowed).toBe(false)
    expect(measured.compensationClaimAllowed).toBe(false)
    expect(compensated.compensationClaimAllowed).toBe(true)
    expect(compensated.settlementClaimAllowed).toBe(false)
  })

  test('redacts private provider, runner, settlement, measurement, and accepted-work refs', () => {
    const projection = projectPylonFlexibleLoadEvent({
      ...examplePylonFlexibleLoadEvent(),
      acceptedWorkImpactRefs: [
        'accepted_work.public.site_revision_5',
        'accepted_work.private.operator_note',
      ],
      measurementRefs: [
        'measurement.public.flex_event_1',
        'measurement.private.operator_meter',
      ],
      providerRef: 'provider.private_demo_1',
      settlementRefs: [
        'settlement.public.flex_event_1',
        'settlement.private.operator_receipt',
      ],
    }, 'public', nowIso)

    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.acceptedWorkImpactRefs).toEqual([
      'accepted_work.public.site_revision_5',
    ])
    expect(projection.measurementRefs).toEqual([
      'measurement.public.flex_event_1',
    ])
    expect(projection.settlementRefs).toEqual([
      'settlement.public.flex_event_1',
    ])
    expect(pylonFlexibleLoadEventProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('rejects missing lifecycle evidence, false settlement, lost-work overclaim, and mutation authority', () => {
    const base = examplePylonFlexibleLoadEvent()

    for (const record of [
      { ...base, requestRefs: [] },
      { ...base, acknowledgementRefs: [], state: 'acknowledged' as const },
      { ...base, executionRefs: [], state: 'executed' as const },
      { ...base, actualResponseWatts: null, state: 'measured' as const },
      { ...base, measurementRefs: [], state: 'measured' as const },
      { ...base, evidenceRefs: [], state: 'verified' as const },
      { ...base, compensationRefs: [], state: 'compensated' as const },
      { ...base, settlementRefs: [], state: 'settled' as const },
      { ...base, interruptedWorkRefs: [], lostWorkCostCents: 1 },
      { ...base, checkpointRefs: [], resumeRefs: ['resume.public.orphan'] },
      {
        ...base,
        authority: {
          ...PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY,
          noGridServiceClaimUpgrade: false,
        },
      },
      {
        ...base,
        authority: {
          ...PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY,
          noCapacityDispatch: false,
        },
      },
      {
        ...base,
        authority: {
          ...PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY,
          noSettlementMutation: false,
        },
      },
    ]) {
      expect(() =>
        projectPylonFlexibleLoadEvent(record, 'operator', nowIso),
      ).toThrow(PylonFlexibleLoadEventUnsafe)
    }
  })

  test('rejects private provider, runner, wallet, payment, payout target, raw telemetry, and raw timestamps', () => {
    const base = examplePylonFlexibleLoadEvent()

    for (const record of [
      { ...base, evidenceRefs: ['provider_telemetry.power_curve'] },
      { ...base, measurementRefs: ['raw_meter.telemetry.panel'] },
      { ...base, executionRefs: ['raw_runner_log.flex_event'] },
      { ...base, caveatRefs: ['wallet_state.local_node'] },
      { ...base, compensationRefs: ['payment_id.raw_123'] },
      { ...base, settlementRefs: ['payout_target.raw_node'] },
      { ...base, requestRefs: ['request.2026-06-06T23:00:00Z'] },
    ]) {
      expect(() =>
        projectPylonFlexibleLoadEvent(record, 'operator', nowIso),
      ).toThrow(PylonFlexibleLoadEventUnsafe)
    }
  })
})
