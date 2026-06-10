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
  darkCapacityReasonRefForPylon,
  handlePylonCapacityFunnelApi,
  pylonCapacityFunnelRecordsFromStore,
} from './pylon-capacity-funnel-live-routes'

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
})
