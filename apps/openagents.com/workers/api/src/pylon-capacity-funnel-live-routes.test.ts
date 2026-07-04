import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  PylonApiAssignmentRecord,
  PylonApiProviderJobLifecycleRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'
import { providerJobLifecycleRecordFromAssignment } from './pylon-api'
import {
  type PylonCapacityFunnelSnapshotBucketKind,
  type PylonCapacityFunnelSnapshotRecord,
  type PylonCapacityFunnelSnapshotStore,
  buildPylonCapacityFunnelSnapshotRecord,
  darkCapacityReasonRefForPylon,
  handlePylonCapacityFunnelApi,
  handlePylonCapacityFunnelHistoryApi,
  makeDualWritePylonCapacityFunnelSnapshotStore,
  pylonCapacityFunnelRecordsFromStore,
  readPylonCapacityFunnelAggregate,
  recordPylonCapacityFunnelSnapshots,
} from './pylon-capacity-funnel-live-routes'
import { aggregatePylonCapacityFunnel } from './pylon-capacity-funnel'
import type {
  PylonDispatchDiagnostic,
  PylonDispatchDiagnosticEvent,
} from './pylon-dispatch-store'

const nowIso = '2026-06-09T20:00:00.000Z'

const registration = (
  overrides: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: [
    'capability.pylon.assignment_ready',
    'capability.pylon.local_coding_agent',
  ],
  clientProtocolVersion: '1',
  clientVersion: '0.3.0',
  createdAt: '2026-06-09T18:00:00.000Z',
  displayName: 'Test Pylon',
  id: 'pylon_row_1',
  latestHeartbeatAt: '2026-06-09T19:58:00.000Z',
  latestHeartbeatStatus: 'online',
  latestCapacityRefs: [],
  latestHealthRefs: [],
  latestLoadRefs: [],
  latestResourceMode: null,
  ownerAgentCredentialId: 'agent_credential_test',
  ownerAgentTokenPrefix: 'oa_agent_test',
  ownerAgentUserId: 'user_test',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef: 'pylon.test.alpha',
  resourceMode: 'balanced',
  status: 'active',
  updatedAt: '2026-06-09T19:58:00.000Z',
  walletReady: true,
  walletRef: 'wallet.public.test.redacted',
  ...overrides,
})

const assignment = (
  overrides: Partial<PylonApiAssignmentRecord> = {},
): PylonApiAssignmentRecord => ({
  acceptanceCriteriaRefs: [],
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: 'pylon_assignment.test.1',
  closeoutRefs: [],
  codingAssignment: null,
  createdAt: '2026-06-09T19:00:00.000Z',
  id: 'assignment_row_1',
  idempotencyKeyHash: 'hash',
  jobKind: 'inference',
  leaseExpiresAt: '2026-06-09T21:00:00.000Z',
  ownerAgentUserId: 'user_test',
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef: 'pylon.test.alpha',
  rejectionRefs: [],
  resultExpectationRefs: [],
  state: 'running',
  taskRefs: [],
  updatedAt: '2026-06-09T19:30:00.000Z',
  ...overrides,
})

class MemorySnapshotStore implements PylonCapacityFunnelSnapshotStore {
  readonly records = new Map<string, PylonCapacityFunnelSnapshotRecord>()
  readonly pruned: Array<
    Readonly<{
      beforeIso: string
      bucketKind: PylonCapacityFunnelSnapshotBucketKind
    }>
  > = []

  listSnapshots = async (
    input: Readonly<{
      bucketKind: PylonCapacityFunnelSnapshotBucketKind
      limit: number
    }>,
  ) =>
    Array.from(this.records.values())
      .filter(record => record.bucketKind === input.bucketKind)
      .sort((left, right) =>
        right.bucketStartAt.localeCompare(left.bucketStartAt),
      )
      .slice(0, input.limit)

  pruneSnapshotsBefore = async (
    input: Readonly<{
      beforeIso: string
      bucketKind: PylonCapacityFunnelSnapshotBucketKind
    }>,
  ) => {
    this.pruned.push(input)
  }

  upsertSnapshot = async (record: PylonCapacityFunnelSnapshotRecord) => {
    this.records.set(`${record.bucketKind}:${record.bucketStartAt}`, record)

    return record
  }
}

const makeSnapshotLogSink = () => {
  const events: Array<{
    event: PylonDispatchDiagnosticEvent
    fields: PylonDispatchDiagnostic
  }> = []
  return {
    events,
    log: (
      event: PylonDispatchDiagnosticEvent,
      fields: PylonDispatchDiagnostic,
    ) => {
      events.push({ event, fields })
    },
  }
}

describe('pylon capacity funnel snapshot dual-write', () => {
  test('snapshot writes mirror to Postgres while reads stay D1-authoritative', async () => {
    const d1 = new MemorySnapshotStore()
    const pg = new MemorySnapshotStore()
    const snapshot = buildPylonCapacityFunnelSnapshotRecord({
      aggregate: aggregatePylonCapacityFunnel([], 'public', nowIso),
      bucketKind: 'hourly',
      nowIso,
    })
    const store = makeDualWritePylonCapacityFunnelSnapshotStore({
      d1,
      flags: { dualWrite: true },
      postgres: pg,
    })

    await store.upsertSnapshot(snapshot)
    await store.pruneSnapshotsBefore({
      beforeIso: '2026-05-26T20:00:00.000Z',
      bucketKind: 'hourly',
    })
    const read = await store.listSnapshots({ bucketKind: 'hourly', limit: 10 })

    expect(read).toEqual([snapshot])
    expect(pg.records.get('hourly:2026-06-09T20:00:00.000Z')).toEqual(
      snapshot,
    )
    expect(pg.pruned).toEqual([
      {
        beforeIso: '2026-05-26T20:00:00.000Z',
        bucketKind: 'hourly',
      },
    ])
  })

  test('snapshot mirror failures are fail-soft diagnostics', async () => {
    const d1 = new MemorySnapshotStore()
    const snapshot = buildPylonCapacityFunnelSnapshotRecord({
      aggregate: aggregatePylonCapacityFunnel([], 'public', nowIso),
      bucketKind: 'daily',
      nowIso,
    })
    const sink = makeSnapshotLogSink()
    const store = makeDualWritePylonCapacityFunnelSnapshotStore({
      d1,
      flags: { dualWrite: true },
      log: sink.log,
      postgres: {
        listSnapshots: () => Promise.resolve([]),
        pruneSnapshotsBefore: () => Promise.reject(new Error('pg down')),
        upsertSnapshot: () => Promise.reject(new Error('pg down')),
      },
    })

    await expect(store.upsertSnapshot(snapshot)).resolves.toEqual(snapshot)
    await expect(
      store.pruneSnapshotsBefore({
        beforeIso: '2025-12-11T20:00:00.000Z',
        bucketKind: 'daily',
      }),
    ).resolves.toBeUndefined()
    expect(sink.events.map(event => event.fields.op)).toEqual([
      'upsertPylonCapacityFunnelSnapshot',
      'prunePylonCapacityFunnelSnapshots',
    ])
    expect(sink.events[0]?.fields.refs).toEqual([
      'daily',
      '2026-06-09T00:00:00.000Z',
    ])
  })
})

describe('pylon capacity funnel live bridge', () => {
  test('classifies every dark-capacity reason in the taxonomy', () => {
    const cases: ReadonlyArray<
      Readonly<{
        assignments: ReadonlyArray<PylonApiAssignmentRecord>
        expected: string
        registration: PylonApiRegistrationRecord
      }>
    > = [
      {
        assignments: [],
        expected: 'dark_capacity.public.never_heartbeated',
        registration: registration({ latestHeartbeatAt: null }),
      },
      {
        assignments: [],
        expected: 'dark_capacity.public.stale_heartbeat',
        registration: registration({
          latestHeartbeatAt: '2026-06-09T10:00:00.000Z',
        }),
      },
      {
        assignments: [],
        expected: 'dark_capacity.public.version_incompatible',
        registration: registration({ clientVersion: '0.1.0' }),
      },
      {
        assignments: [],
        expected: 'dark_capacity.public.capability_missing',
        registration: registration({ capabilityRefs: [] }),
      },
      {
        assignments: [],
        expected: 'dark_capacity.public.wallet_not_ready',
        registration: registration({ walletReady: false }),
      },
      {
        assignments: [assignment({ state: 'rejected' })],
        expected: 'dark_capacity.public.assignment_declined',
        registration: registration(),
      },
      {
        assignments: [assignment({ state: 'stale' })],
        expected: 'dark_capacity.public.assignment_expired',
        registration: registration(),
      },
      {
        assignments: [
          assignment({
            leaseExpiresAt: '2026-06-09T19:10:00.000Z',
            state: 'accepted',
          }),
        ],
        expected: 'dark_capacity.public.closeout_missing',
        registration: registration(),
      },
      {
        assignments: [],
        expected: 'dark_capacity.public.no_assignments_offered',
        registration: registration(),
      },
    ]

    for (const candidate of cases) {
      expect(
        darkCapacityReasonRefForPylon({
          assignments: candidate.assignments,
          nowIso,
          registration: candidate.registration,
        }),
      ).toBe(candidate.expected)
    }

    expect(
      darkCapacityReasonRefForPylon({
        assignments: [assignment()],
        nowIso,
        registration: registration(),
      }),
    ).toBeNull()
  })

  test('aggregates funnel stages from live store records', () => {
    const registrations = [
      registration({ pylonRef: 'pylon.test.running' }),
      registration({
        latestHeartbeatAt: null,
        pylonRef: 'pylon.test.dark',
      }),
      registration({ pylonRef: 'pylon.test.idle' }),
      registration({
        capabilityRefs: ['capability.pylon.assignment_ready'],
        pylonRef: 'pylon.test.artifact',
      }),
    ]
    const assignmentsByPylonRef = new Map([
      ['pylon.test.running', [assignment()]],
      [
        'pylon.test.artifact',
        [
          assignment({
            artifactRefs: ['artifact.public.test.delivered'],
            assignmentRef: 'pylon_assignment.test.artifact',
            state: 'closeout_submitted',
          }),
        ],
      ],
    ])
    const lifecycleByPylonRef = new Map([
      [
        'pylon.test.running',
        [providerJobLifecycleRecordFromAssignment(assignment())],
      ],
      [
        'pylon.test.artifact',
        [
          providerJobLifecycleRecordFromAssignment(
            assignment({
              artifactRefs: ['artifact.public.test.delivered'],
              assignmentRef: 'pylon_assignment.test.artifact',
              state: 'closeout_submitted',
            }),
          ),
        ],
      ],
    ])
    const records = pylonCapacityFunnelRecordsFromStore({
      assignmentsByPylonRef,
      lifecycleByPylonRef,
      nowIso,
      registrations,
    })

    expect(records.map(record => record.stage)).toEqual([
      'running',
      'dark',
      'eligible',
      'artifact_producing',
    ])
    expect(records[1]?.darkCapacityReasonRefs).toEqual([
      'dark_capacity.public.never_heartbeated',
    ])
    expect(records[2]?.darkCapacityReasonRefs).toEqual([
      'dark_capacity.public.no_assignments_offered',
    ])
    expect(
      JSON.stringify(records),
    ).not.toMatch(/pylon\.test\.|user_test|oa_agent|wallet\.public\.test/)
  })

  test('prefers provider job lifecycle records over assignment inference for job stages', () => {
    const registrations = [
      registration({ pylonRef: 'pylon.test.accepted' }),
      registration({ pylonRef: 'pylon.test.artifact' }),
      registration({ pylonRef: 'pylon.test.running' }),
      registration({ pylonRef: 'pylon.test.assigned' }),
    ]
    const lifecycleByPylonRef = new Map<
      string,
      ReadonlyArray<PylonApiProviderJobLifecycleRecord>
    >([
      [
        'pylon.test.accepted',
        [
          providerJobLifecycleRecordFromAssignment(
            assignment({
              acceptedWorkRefs: ['accepted_work.public.test'],
              pylonRef: 'pylon.test.accepted',
              state: 'accepted_work',
            }),
          ),
        ],
      ],
      [
        'pylon.test.artifact',
        [
          providerJobLifecycleRecordFromAssignment(
            assignment({
              artifactRefs: ['artifact.public.test'],
              pylonRef: 'pylon.test.artifact',
              state: 'proof_submitted',
            }),
          ),
        ],
      ],
      [
        'pylon.test.running',
        [
          providerJobLifecycleRecordFromAssignment(
            assignment({
              pylonRef: 'pylon.test.running',
              state: 'running',
            }),
          ),
        ],
      ],
      [
        'pylon.test.assigned',
        [
          providerJobLifecycleRecordFromAssignment(
            assignment({
              pylonRef: 'pylon.test.assigned',
              state: 'offered',
            }),
          ),
        ],
      ],
    ])
    const records = pylonCapacityFunnelRecordsFromStore({
      assignmentsByPylonRef: new Map(),
      lifecycleByPylonRef,
      nowIso,
      registrations,
    })

    expect(records.map(record => record.stage)).toEqual([
      'accepted',
      'artifact_producing',
      'running',
      'assigned',
    ])
  })

  test('counts accepted-work assignment rows when lifecycle rows lag', async () => {
    const store: PylonApiStore = {
      createAssignment: () => Promise.reject(new Error('unused')),
      createEvent: () => Promise.reject(new Error('unused')),
      listAssignmentsForPylon: () => Promise.resolve([]),
      listAssignmentsForPylons: () =>
        Promise.resolve([
          assignment({
            acceptedWorkRefs: ['accepted_work.public.assignment_lag'],
            closeoutRefs: ['closeout.public.assignment_lag'],
            pylonRef: 'pylon.test.accepted_lag',
            state: 'accepted_work',
          }),
        ]),
      listEventsForPylon: () => Promise.resolve([]),
      listEventsForAssignment: () => Promise.resolve([]),
      listRegistrations: () =>
        Promise.resolve([
          registration({ pylonRef: 'pylon.test.accepted_lag' }),
        ]),
      listProviderJobLifecycleForPylons: () => Promise.resolve([]),
      readEventByIdempotencyKeyHash: () => Promise.resolve(undefined),
      readAssignment: () => Promise.resolve(undefined),
      readAssignmentByIdempotencyKeyHash: () => Promise.resolve(undefined),
      readRegistrationByPylonRef: () => Promise.resolve(undefined),
      updateAssignment: () => Promise.reject(new Error('unused')),
      upsertProviderJobLifecycle: () => Promise.reject(new Error('unused')),
      upsertRegistration: () => Promise.reject(new Error('unused')),
    } as unknown as PylonApiStore

    const response = await Effect.runPromise(
      handlePylonCapacityFunnelApi(
        new Request('https://openagents.com/api/public/pylon-capacity-funnel'),
        { nowIso: () => nowIso, store },
      ),
    )
    const body = (await response.json()) as Readonly<{
      funnel: Readonly<{
        acceptedCount: number
        byStage: ReadonlyArray<Readonly<{ count: number; key: string }>>
        totalCount: number
      }>
    }>

    expect(response.status).toBe(200)
    expect(body.funnel.totalCount).toBe(1)
    expect(body.funnel.acceptedCount).toBe(1)
    expect(body.funnel.byStage).toEqual([
      {
        count: 1,
        key: 'accepted',
      },
    ])
    expect(JSON.stringify(body)).not.toMatch(
      /pylon\.test\.|user_test|oa_agent|accepted_lag/,
    )
  })

  test('serves the public funnel route with counts only', async () => {
    const store: PylonApiStore = {
      createAssignment: () => Promise.reject(new Error('unused')),
      createEvent: () => Promise.reject(new Error('unused')),
      listAssignmentsForPylon: (pylonRef: string) =>
        Promise.resolve(
          pylonRef === 'pylon.test.running' ? [assignment()] : [],
        ),
      listEventsForPylon: () => Promise.resolve([]),
      listEventsForAssignment: () => Promise.resolve([]),
      listRegistrations: () =>
        Promise.resolve([
          registration({ pylonRef: 'pylon.test.running' }),
          registration({
            latestHeartbeatAt: null,
            pylonRef: 'pylon.test.dark',
          }),
        ]),
      listProviderJobLifecycleForPylons: (pylonRefs: ReadonlyArray<string>) =>
        Promise.resolve(
          pylonRefs.includes('pylon.test.running')
            ? [
                providerJobLifecycleRecordFromAssignment(
                  assignment({ pylonRef: 'pylon.test.running' }),
                ),
              ]
            : [],
        ),
      readEventByIdempotencyKeyHash: () => Promise.resolve(undefined),
      readAssignment: () => Promise.resolve(undefined),
      readAssignmentByIdempotencyKeyHash: () => Promise.resolve(undefined),
      readRegistrationByPylonRef: () => Promise.resolve(undefined),
      updateAssignment: () => Promise.reject(new Error('unused')),
      upsertProviderJobLifecycle: () => Promise.reject(new Error('unused')),
      upsertRegistration: () => Promise.reject(new Error('unused')),
    } as unknown as PylonApiStore
    const response = await Effect.runPromise(
      handlePylonCapacityFunnelApi(
        new Request('https://openagents.com/api/public/pylon-capacity-funnel'),
        { nowIso: () => nowIso, store },
      ),
    )
    const body = (await response.json()) as Readonly<{
      funnel: Readonly<{
        byDarkCapacityReason: ReadonlyArray<
          Readonly<{ count: number; key: string }>
        >
        darkCount: number
        registeredCount: number
        runningCount: number
        totalCount: number
      }>
      joinLifecycleLadder: Readonly<{
        byState: ReadonlyArray<Readonly<{ count: number; key: string }>>
        entries: ReadonlyArray<
          Readonly<{
            capacityRef: string
            ladderRank: number
            state: string
            stateLabel: string
          }>
        >
        totalCount: number
      }>
      kind: string
      publicSafe: boolean
    }>

    expect(response.status).toBe(200)
    expect(body.kind).toBe('pylon_capacity_funnel_live')
    expect(body.publicSafe).toBe(true)
    expect(body.funnel.totalCount).toBe(2)
    expect(body.funnel.runningCount).toBe(1)
    expect(body.funnel.darkCount).toBe(1)
    expect(body.funnel.byDarkCapacityReason).toEqual([
      expect.objectContaining({
        count: 1,
        key: 'dark_capacity.public.never_heartbeated',
      }),
    ])
    expect(body.joinLifecycleLadder.totalCount).toBe(2)
    expect(body.joinLifecycleLadder.entries).toEqual([
      {
        capacityRef: 'capacity.public.pylon_live.entry_1',
        ladderRank: 3,
        state: 'warmup',
        stateLabel: 'Warmup',
      },
      {
        capacityRef: 'capacity.public.pylon_live.entry_2',
        ladderRank: 0,
        state: 'registered',
        stateLabel: 'Registered',
      },
    ])
    expect(body.joinLifecycleLadder.byState).toEqual([
      { count: 1, key: 'registered' },
      { count: 1, key: 'warmup' },
    ])
    expect(JSON.stringify(body)).not.toMatch(
      /pylon\.test\.|user_test|oa_agent/,
    )

    const wrongMethod = await Effect.runPromise(
      handlePylonCapacityFunnelApi(
        new Request(
          'https://openagents.com/api/public/pylon-capacity-funnel',
          { method: 'POST' },
        ),
        { nowIso: () => nowIso, store },
      ),
    )

    expect(wrongMethod.status).toBe(405)
  })

  test('records hourly and daily retained funnel snapshots', async () => {
    const store: PylonApiStore = {
      createAssignment: () => Promise.reject(new Error('unused')),
      createEvent: () => Promise.reject(new Error('unused')),
      listAssignmentsForPylon: () => Promise.resolve([assignment()]),
      listEventsForPylon: () => Promise.resolve([]),
      listEventsForAssignment: () => Promise.resolve([]),
      listRegistrations: () =>
        Promise.resolve([
          registration({ latestHeartbeatAt: '2026-06-09T20:41:00.000Z' }),
        ]),
      listProviderJobLifecycleForPylons: () =>
        Promise.resolve([providerJobLifecycleRecordFromAssignment(assignment())]),
      readEventByIdempotencyKeyHash: () => Promise.resolve(undefined),
      readAssignment: () => Promise.resolve(undefined),
      readAssignmentByIdempotencyKeyHash: () => Promise.resolve(undefined),
      readRegistrationByPylonRef: () => Promise.resolve(undefined),
      updateAssignment: () => Promise.reject(new Error('unused')),
      upsertProviderJobLifecycle: () => Promise.reject(new Error('unused')),
      upsertRegistration: () => Promise.reject(new Error('unused')),
    } as unknown as PylonApiStore
    const snapshotStore = new MemorySnapshotStore()

    const snapshots = await recordPylonCapacityFunnelSnapshots({
      nowIso: '2026-06-09T20:42:00.000Z',
      snapshotStore,
      store,
    })

    expect(snapshots.map(snapshot => snapshot.bucketKind).sort()).toEqual([
      'daily',
      'hourly',
    ])
    expect(
      snapshots.find(snapshot => snapshot.bucketKind === 'hourly')
        ?.bucketStartAt,
    ).toBe('2026-06-09T20:00:00.000Z')
    expect(
      snapshots.find(snapshot => snapshot.bucketKind === 'daily')
        ?.bucketStartAt,
    ).toBe('2026-06-09T00:00:00.000Z')
    expect(snapshots[0]?.aggregate.runningCount).toBe(1)
    expect(snapshotStore.pruned).toEqual([
      {
        beforeIso: '2026-05-26T20:42:00.000Z',
        bucketKind: 'hourly',
      },
      {
        beforeIso: '2025-12-11T20:42:00.000Z',
        bucketKind: 'daily',
      },
    ])
  })

  test('serves retained public funnel history as counts only', async () => {
    const snapshotStore = new MemorySnapshotStore()
    const records = pylonCapacityFunnelRecordsFromStore({
      assignmentsByPylonRef: new Map([['pylon.test.alpha', [assignment()]]]),
      lifecycleByPylonRef: new Map([
        [
          'pylon.test.alpha',
          [providerJobLifecycleRecordFromAssignment(assignment())],
        ],
      ]),
      nowIso,
      registrations: [registration()],
    })
    const aggregate = aggregatePylonCapacityFunnel(records, 'public', nowIso)

    await snapshotStore.upsertSnapshot(
      buildPylonCapacityFunnelSnapshotRecord({
        aggregate,
        bucketKind: 'hourly',
        nowIso: '2026-06-09T20:42:00.000Z',
      }),
    )
    await snapshotStore.upsertSnapshot(
      buildPylonCapacityFunnelSnapshotRecord({
        aggregate,
        bucketKind: 'daily',
        nowIso: '2026-06-09T20:42:00.000Z',
      }),
    )

    const response = await Effect.runPromise(
      handlePylonCapacityFunnelHistoryApi(
        new Request(
          'https://openagents.com/api/public/pylon-capacity-funnel/history',
        ),
        { nowIso: () => nowIso, snapshotStore },
      ),
    )
    const body = (await response.json()) as Readonly<{
      history: Readonly<{
        daily: ReadonlyArray<Record<string, unknown>>
        hourly: ReadonlyArray<Record<string, unknown>>
        retentionPolicyRef: string
      }>
      kind: string
      publicSafe: boolean
    }>

    expect(response.status).toBe(200)
    expect(body.kind).toBe('pylon_capacity_funnel_history')
    expect(body.publicSafe).toBe(true)
    expect(body.history.retentionPolicyRef).toBe(
      'retention.public.pylon_capacity_funnel.hourly_14d_daily_180d',
    )
    expect(body.history.hourly).toHaveLength(1)
    expect(body.history.daily).toHaveLength(1)
    expect(body.history.hourly[0]?.funnel).toMatchObject({
      runningCount: 1,
      totalCount: 1,
    })
    expect(JSON.stringify(body)).not.toMatch(
      /pylon\\.test\\.|user_test|oa_agent|wallet\\.public\\.test/,
    )

    const wrongMethod = await Effect.runPromise(
      handlePylonCapacityFunnelHistoryApi(
        new Request(
          'https://openagents.com/api/public/pylon-capacity-funnel/history',
          { method: 'POST' },
        ),
        { nowIso: () => nowIso, snapshotStore },
      ),
    )

    expect(wrongMethod.status).toBe(405)
  })
})

describe('funnel aggregate subrequest discipline', () => {
  test('uses one batched assignments query when the store supports it (no N+1)', async () => {
    let batchedCalls = 0
    let perPylonCalls = 0
    const registrations = Array.from({ length: 60 }, (_, index) =>
      registration({ pylonRef: `pylon.test.batch_${index}` }),
    )
    const store = {
      listAssignmentsForPylon: () => {
        perPylonCalls += 1
        return Promise.resolve([])
      },
      listAssignmentsForPylons: (pylonRefs: ReadonlyArray<string>) => {
        batchedCalls += 1
        expect(pylonRefs).toHaveLength(60)
        return Promise.resolve([
          assignment({ pylonRef: 'pylon.test.batch_0' }),
        ])
      },
      listProviderJobLifecycleForPylons: () => Promise.resolve([]),
      listRegistrations: () => Promise.resolve(registrations),
    } as unknown as PylonApiStore

    const aggregate = await readPylonCapacityFunnelAggregate({
      nowIso,
      store,
    })

    expect(batchedCalls).toBe(1)
    expect(perPylonCalls).toBe(0)
    expect(aggregate.totalCount).toBe(60)
  })
})

describe('dark-reason taxonomy survives its own scanner', () => {
  test('a wallet_not_ready pylon projects instead of poisoning the surface', async () => {
    const store = {
      listAssignmentsForPylon: () => Promise.resolve([]),
      listProviderJobLifecycleForPylons: () => Promise.resolve([]),
      listRegistrations: () =>
        Promise.resolve([
          registration({
            pylonRef: 'pylon.test.no_wallet',
            walletReady: false,
          }),
        ]),
    } as unknown as PylonApiStore

    const aggregate = await readPylonCapacityFunnelAggregate({
      nowIso,
      store,
    })

    expect(aggregate.totalCount).toBe(1)
    expect(aggregate.byDarkCapacityReason).toEqual([
      { count: 1, key: 'dark_capacity.public.wallet_not_ready' },
    ])
  })
})
