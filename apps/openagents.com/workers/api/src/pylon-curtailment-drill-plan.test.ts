import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CURTAILMENT_DRILL_EVENT_STATE_REQUIREMENTS,
  CURTAILMENT_DRILL_EVENT_STATE_SEQUENCE,
  buildCurtailmentDrillRequestedEvent,
  exampleCurtailmentDrillRequestedEvent,
} from './pylon-curtailment-drill-plan'
import {
  PylonFlexibleLoadEventProjection,
  PylonFlexibleLoadEventUnsafe,
  projectPylonFlexibleLoadEvent,
} from './pylon-flexible-load-events'

const nowIso = '2026-06-20T00:05:00.000Z'

describe('Pylon curtailment drill plan', () => {
  test('seed drill event is a valid, public-safe requested-state record', () => {
    const projection = projectPylonFlexibleLoadEvent(
      exampleCurtailmentDrillRequestedEvent(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonFlexibleLoadEventProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      state: 'requested',
      stateLabel: 'Requested',
      requestedClaimAllowed: true,
      requestedResponseWatts: 250000,
      // Honest: nothing measured, verified, compensated, or settled yet.
      acknowledgementClaimAllowed: false,
      executedClaimAllowed: false,
      measuredClaimAllowed: false,
      verifiedClaimAllowed: false,
      compensationClaimAllowed: false,
      settlementClaimAllowed: false,
      // Authority stays read-only.
      capacityDispatchAllowed: false,
      gridServiceClaimUpgradeAllowed: false,
      liveWalletSpendAllowed: false,
      payoutDispatchAllowed: false,
      settlementMutationAllowed: false,
    })
    expect(projection.actualResponseWatts).toBeNull()
    expect(projection.responseRatioBps).toBeNull()
    expect(projection.lostWorkCostCents).toBe(0)
    expect(JSON.stringify(projection)).not.toContain('2026-06-20T')
  })

  test('cannot mark the drill measured without real telemetry', () => {
    const seed = exampleCurtailmentDrillRequestedEvent()

    // Attempting to advance to `measured` while fabricating nothing must fail.
    expect(() =>
      projectPylonFlexibleLoadEvent(
        { ...seed, state: 'measured' },
        'operator',
        nowIso,
      )
    ).toThrow(PylonFlexibleLoadEventUnsafe)
  })

  test('builder rejects mutated authority (stays read-only)', () => {
    const seed = buildCurtailmentDrillRequestedEvent({
      createdAtIso: '2026-06-20T00:00:00.000Z',
      drillRef: 'event.flex.drill_x',
      id: 'flex_event.drill_x',
      profileRefs: ['profile.flex.psion_pretraining_window'],
      providerRef: 'provider.cohort_x',
      requestRefs: ['request.public.drill_x'],
      requestedResponseWatts: 100000,
      workClassRefs: ['work_class.psion_pretraining_window'],
    })

    expect(() =>
      projectPylonFlexibleLoadEvent(
        {
          ...seed,
          authority: { ...seed.authority, noCapacityDispatch: false },
        },
        'operator',
        nowIso,
      )
    ).toThrow(PylonFlexibleLoadEventUnsafe)
  })

  test('state requirements cover the full happy-path lifecycle in order', () => {
    expect(
      CURTAILMENT_DRILL_EVENT_STATE_REQUIREMENTS.map(r => r.state),
    ).toEqual([...CURTAILMENT_DRILL_EVENT_STATE_SEQUENCE])

    const requested = CURTAILMENT_DRILL_EVENT_STATE_REQUIREMENTS[0]
    expect(requested?.requiredRefFields).toContain('requestRefs')
    expect(requested?.requiredValueFields).toContain('requestedResponseWatts')

    const measured = CURTAILMENT_DRILL_EVENT_STATE_REQUIREMENTS.find(
      r => r.state === 'measured',
    )
    expect(measured?.requiredRefFields).toContain('measurementRefs')
    expect(measured?.requiredValueFields).toContain('actualResponseWatts')

    const settled = CURTAILMENT_DRILL_EVENT_STATE_REQUIREMENTS.find(
      r => r.state === 'settled',
    )
    expect(settled?.requiredRefFields).toContain('settlementRefs')
  })
})
