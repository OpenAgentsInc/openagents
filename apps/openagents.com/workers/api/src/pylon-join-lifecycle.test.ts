import { describe, expect, test } from 'vitest'

import {
  examplePylonCapacityFunnelRecords,
  type PylonCapacityFunnelStage,
} from './pylon-capacity-funnel'
import {
  PYLON_JOIN_LIFECYCLE_TRANSITIONS,
  PylonJoinLifecycleReasonCodes,
  type PylonJoinLifecycleReasonCode,
  type PylonJoinLifecycleRecord,
  type PylonJoinLifecycleState,
  buildPylonJoinLifecycleRecord,
  joinLifecycleStateForFunnel,
  projectPylonJoinLifecycleRecord,
  pylonJoinLifecycleLadderForFunnel,
  pylonJoinLifecycleProjectionHasPrivateMaterial,
  transitionPylonJoinLifecycleRecord,
} from './pylon-join-lifecycle'

const nowIso = '2026-06-12T15:00:00.000Z'
const laterIso = '2026-06-12T15:05:00.000Z'

const climb = (
  record: PylonJoinLifecycleRecord,
  steps: ReadonlyArray<
    Readonly<{
      reasonCode: PylonJoinLifecycleReasonCode
      toState: PylonJoinLifecycleState
    }>
  >,
): PylonJoinLifecycleRecord =>
  steps.reduce(
    (current, step, index) =>
      transitionPylonJoinLifecycleRecord({
        eventId: `step_${index + 1}`,
        nowIso: laterIso,
        reasonCode: step.reasonCode,
        receiptRef: `receipt.public.pylon_join.step_${index + 1}`,
        record: current,
        toState: step.toState,
      }).record,
    record,
  )

const fullForwardClimb = [
  {
    reasonCode:
      'join_lifecycle.public.qualification_gate_passed' as const,
    toState: 'qualified' as const,
  },
  {
    reasonCode:
      'join_lifecycle.public.durable_seal_digest_synced' as const,
    toState: 'state_synced' as const,
  },
  {
    reasonCode: 'join_lifecycle.public.warmup_started' as const,
    toState: 'warmup' as const,
  },
  {
    reasonCode: 'join_lifecycle.public.shadow_work_verified' as const,
    toState: 'active' as const,
  },
]

describe('Pylon join lifecycle ladder', () => {
  test('every transition carries exactly one reason code from the closed set', () => {
    for (const transition of PYLON_JOIN_LIFECYCLE_TRANSITIONS) {
      expect(PylonJoinLifecycleReasonCodes).toContain(transition.reasonCode)
    }

    const reasonCodes = PYLON_JOIN_LIFECYCLE_TRANSITIONS.map(
      transition => transition.reasonCode,
    )

    expect(new Set(reasonCodes).size).toBe(reasonCodes.length)
  })

  test('new records start at registered with a rebuilt projection', () => {
    const record = buildPylonJoinLifecycleRecord({
      capacityRef: 'capacity.public.pylon_live.entry_1',
      nowIso,
    })

    expect(record.state).toBe('registered')
    expect(record.reasonCode).toBeNull()
    expect(JSON.parse(record.publicProjectionJson)).toMatchObject({
      capacityRef: 'capacity.public.pylon_live.entry_1',
      ladderRank: 0,
      state: 'registered',
      stateLabel: 'Registered',
    })
  })

  test('climbs the full forward ladder, emitting a receipt-compatible event per transition', () => {
    let record = buildPylonJoinLifecycleRecord({
      capacityRef: 'capacity.public.pylon_live.entry_2',
      nowIso,
    })

    const firstTransition = transitionPylonJoinLifecycleRecord({
      eventId: 'qualify_1',
      nowIso: laterIso,
      reasonCode: 'join_lifecycle.public.qualification_gate_passed',
      receiptRef: 'receipt.public.pylon_join.qualify_1',
      record,
      toState: 'qualified',
    })

    expect(firstTransition.event).toEqual({
      capacityRef: 'capacity.public.pylon_live.entry_2',
      fromState: 'registered',
      id: 'pylon_join_lifecycle_event_qualify_1',
      occurredAtIso: laterIso,
      reasonCode: 'join_lifecycle.public.qualification_gate_passed',
      receiptRef: 'receipt.public.pylon_join.qualify_1',
      toState: 'qualified',
    })
    expect(firstTransition.record.updatedAtIso).toBe(laterIso)

    record = climb(record, fullForwardClimb)

    expect(record.state).toBe('active')
    expect(record.receiptRefs).toEqual([
      'receipt.public.pylon_join.step_1',
      'receipt.public.pylon_join.step_2',
      'receipt.public.pylon_join.step_3',
      'receipt.public.pylon_join.step_4',
    ])
    expect(JSON.parse(record.publicProjectionJson)).toMatchObject({
      ladderRank: 4,
      state: 'active',
      stateLabel: 'Active',
    })
  })

  test('takes the back edge active -> lagged -> sync_reentry -> state_synced', () => {
    const active = climb(
      buildPylonJoinLifecycleRecord({
        capacityRef: 'capacity.public.pylon_live.entry_3',
        nowIso,
      }),
      fullForwardClimb,
    )

    const reentered = climb(active, [
      {
        reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
        toState: 'lagged',
      },
      {
        reasonCode: 'join_lifecycle.public.sync_reentry_started',
        toState: 'sync_reentry',
      },
      {
        reasonCode: 'join_lifecycle.public.reentry_seal_digest_synced',
        toState: 'state_synced',
      },
    ])

    expect(reentered.state).toBe('state_synced')

    // A re-entered device re-ramps through the same warmup path.
    const reactivated = climb(reentered, [
      {
        reasonCode: 'join_lifecycle.public.warmup_started',
        toState: 'warmup',
      },
      {
        reasonCode: 'join_lifecycle.public.shadow_work_verified',
        toState: 'active',
      },
    ])

    expect(reactivated.state).toBe('active')
  })

  test('rejects illegal transitions with a typed error', () => {
    const record = buildPylonJoinLifecycleRecord({
      capacityRef: 'capacity.public.pylon_live.entry_4',
      nowIso,
    })

    expect(() =>
      transitionPylonJoinLifecycleRecord({
        eventId: 'skip_1',
        nowIso: laterIso,
        reasonCode: 'join_lifecycle.public.shadow_work_verified',
        receiptRef: 'receipt.public.pylon_join.skip_1',
        record,
        toState: 'active',
      }),
    ).toThrowError(
      expect.objectContaining({
        _tag: 'PylonJoinLifecycleTransitionError',
        kind: 'illegal_transition',
      }),
    )

    // registered cannot lapse straight to lagged either.
    expect(() =>
      transitionPylonJoinLifecycleRecord({
        eventId: 'skip_2',
        nowIso: laterIso,
        reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
        receiptRef: 'receipt.public.pylon_join.skip_2',
        record,
        toState: 'lagged',
      }),
    ).toThrowError(
      expect.objectContaining({
        _tag: 'PylonJoinLifecycleTransitionError',
        kind: 'illegal_transition',
      }),
    )
  })

  test('rejects a legal edge carrying the wrong reason code', () => {
    const record = buildPylonJoinLifecycleRecord({
      capacityRef: 'capacity.public.pylon_live.entry_5',
      nowIso,
    })

    expect(() =>
      transitionPylonJoinLifecycleRecord({
        eventId: 'mismatch_1',
        nowIso: laterIso,
        reasonCode: 'join_lifecycle.public.warmup_started',
        receiptRef: 'receipt.public.pylon_join.mismatch_1',
        record,
        toState: 'qualified',
      }),
    ).toThrowError(
      expect.objectContaining({
        _tag: 'PylonJoinLifecycleTransitionError',
        kind: 'reason_code_mismatch',
      }),
    )
  })

  test('rejects unsafe capacity and receipt refs', () => {
    expect(() =>
      buildPylonJoinLifecycleRecord({
        capacityRef: 'capacity.wallet_mnemonic_abandon',
        nowIso,
      }),
    ).toThrowError(
      expect.objectContaining({ _tag: 'PylonJoinLifecycleUnsafe' }),
    )

    const record = buildPylonJoinLifecycleRecord({
      capacityRef: 'capacity.public.pylon_live.entry_6',
      nowIso,
    })

    expect(() =>
      transitionPylonJoinLifecycleRecord({
        eventId: 'unsafe_1',
        nowIso: laterIso,
        reasonCode: 'join_lifecycle.public.qualification_gate_passed',
        receiptRef: 'receipt.payment_hash.abc123',
        record,
        toState: 'qualified',
      }),
    ).toThrowError(
      expect.objectContaining({ _tag: 'PylonJoinLifecycleUnsafe' }),
    )
  })

  test('projection is public-safe and never carries raw timestamps', () => {
    const record = climb(
      buildPylonJoinLifecycleRecord({
        capacityRef: 'capacity.public.pylon_live.entry_7',
        nowIso,
      }),
      fullForwardClimb,
    )
    const projection = projectPylonJoinLifecycleRecord(record, laterIso)

    expect(projection).toMatchObject({
      capacityRef: 'capacity.public.pylon_live.entry_7',
      ladderRank: 4,
      reasonCode: 'join_lifecycle.public.shadow_work_verified',
      state: 'active',
      stateLabel: 'Active',
    })
    expect(pylonJoinLifecycleProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
    expect(JSON.stringify(projection)).not.toContain(nowIso)
    expect(JSON.stringify(projection)).not.toContain(laterIso)
  })

  test('maps funnel stages and dark reason codes onto ladder states', () => {
    const stageExpectations: ReadonlyArray<
      readonly [PylonCapacityFunnelStage, PylonJoinLifecycleState]
    > = [
      ['registered', 'registered'],
      ['benchmarked', 'qualified'],
      ['eligible', 'qualified'],
      ['assigned', 'state_synced'],
      ['running', 'warmup'],
      ['artifact_producing', 'warmup'],
      ['accepted', 'active'],
      ['paid', 'active'],
      ['settled', 'active'],
    ]

    for (const [stage, state] of stageExpectations) {
      expect(
        joinLifecycleStateForFunnel({ darkCapacityReasonRefs: [], stage }),
      ).toBe(state)
    }

    expect(
      joinLifecycleStateForFunnel({
        darkCapacityReasonRefs: ['dark_capacity.public.stale_heartbeat'],
        stage: 'dark',
      }),
    ).toBe('lagged')
    expect(
      joinLifecycleStateForFunnel({
        darkCapacityReasonRefs: ['dark_capacity.public.assignment_declined'],
        stage: 'dark',
      }),
    ).toBe('qualified')

    // Multiple reasons claim the weakest supported rung.
    expect(
      joinLifecycleStateForFunnel({
        darkCapacityReasonRefs: [
          'dark_capacity.public.stale_heartbeat',
          'dark_capacity.public.wallet_not_ready',
        ],
        stage: 'dark',
      }),
    ).toBe('registered')

    // Unknown reason refs and reasonless dark rows prove nothing
    // beyond registration; sync_reentry is never inferred from
    // funnel snapshots.
    expect(
      joinLifecycleStateForFunnel({
        darkCapacityReasonRefs: ['dark_reason.unmapped_legacy'],
        stage: 'dark',
      }),
    ).toBe('registered')
    expect(
      joinLifecycleStateForFunnel({
        darkCapacityReasonRefs: [],
        stage: 'dark',
      }),
    ).toBe('registered')
  })

  test('renders public-safe ladder positions per funnel record', () => {
    const ladder = pylonJoinLifecycleLadderForFunnel(
      examplePylonCapacityFunnelRecords(),
      'public',
      nowIso,
    )

    expect(ladder.schemaVersion).toBe(
      'openagents.pylon.join_lifecycle_ladder.v1',
    )
    expect(ladder.totalCount).toBe(2)
    expect(ladder.entries).toEqual([
      {
        capacityRef: 'capacity.pylon_demo_1',
        ladderRank: 4,
        state: 'active',
        stateLabel: 'Active',
      },
      {
        capacityRef: 'capacity.pylon_demo_2',
        ladderRank: 0,
        state: 'registered',
        stateLabel: 'Registered',
      },
    ])
    expect(ladder.byState).toEqual([
      { count: 1, key: 'active' },
      { count: 1, key: 'registered' },
    ])

    const text = JSON.stringify(ladder)

    expect(text).not.toMatch(/node\.private|provider\.private/)
    expect(text).not.toMatch(/wallet|mnemonic|payment|invoice|lnbc/i)
    expect(text).not.toContain('2026-06-06T21:35:00.000Z')
  })
})
