import { describe, expect, test } from 'vitest'

import {
  makeArtanisPylonAssignmentsLister,
  makeArtanisPylonJobStatusReader,
} from './artanis-operator-pylon-job-status'
import type {
  PylonApiAssignmentRecord,
  PylonApiEventRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'

const OWNER = 'github:14167547'
const LINKED_AGENT = 'agent:linked-1'

const baseAssignment: PylonApiAssignmentRecord = {
  acceptanceCriteriaRefs: [],
  acceptedWorkRefs: ['accepted_work.public.pylon_assignment.001'],
  artifactRefs: ['artifact.public.pylon_assignment.001'],
  assignmentRef: 'assignment.public.pylon_api.known_001',
  closeoutRefs: ['closeout.public.pylon_assignment.001'],
  codingAssignment: null,
  createdAt: '2026-06-26T00:00:00.000Z',
  id: 'pylon_api_assignment_x',
  idempotencyKeyHash: 'hash',
  jobKind: 'codex_agent_task',
  leaseExpiresAt: '2026-06-26T00:15:00.000Z',
  ownerAgentUserId: LINKED_AGENT,
  proofRefs: ['proof.public.pylon_assignment.001'],
  publicProjectionJson: '{}',
  pylonRef: 'pylon.public.edge.alpha',
  rejectionRefs: [],
  resultExpectationRefs: [],
  state: 'closeout_submitted',
  taskRefs: [],
  updatedAt: '2026-06-26T00:10:00.000Z',
}

const closeoutEvent: PylonApiEventRecord = {
  assignmentRef: baseAssignment.assignmentRef,
  createdAt: '2026-06-26T00:10:00.000Z',
  eventBody: {},
  eventKind: 'worker_closeout',
  eventRef: 'pylon_event.worker_closeout.001',
  id: 'pylon_api_event_1',
  idempotencyKeyHash: 'h1',
  ownerAgentUserId: LINKED_AGENT,
  publicProjectionJson: '{}',
  pylonRef: baseAssignment.pylonRef,
  status: 'accepted',
}

const fakeStore = (
  overrides: Partial<{
    assignment: PylonApiAssignmentRecord | undefined
    events: ReadonlyArray<PylonApiEventRecord>
  }> = {},
): PylonApiStore =>
  ({
    listEventsForAssignment: async () => overrides.events ?? [closeoutEvent],
    readAssignment: async () =>
      'assignment' in overrides ? overrides.assignment : baseAssignment,
  }) as unknown as PylonApiStore

const reader = (
  store: PylonApiStore,
  linked: ReadonlyArray<string> = [LINKED_AGENT],
) =>
  makeArtanisPylonJobStatusReader({
    listLinkedAgentUserIds: async () => linked,
    nowIso: () => '2026-06-26T00:12:00.000Z',
    ownerOpenAuthUserId: OWNER,
    pylonStore: store,
  })

describe('makeArtanisPylonJobStatusReader', () => {
  test('resolves an owned assignment to a public-safe PASS status', async () => {
    const status = await reader(fakeStore())(baseAssignment.assignmentRef)
    expect(status).not.toBeNull()
    expect(status?.assignmentRef).toBe(baseAssignment.assignmentRef)
    expect(status?.jobKind).toBe('codex_agent_task')
    expect(status?.state).toBe('closeout_submitted')
    expect(status?.leaseState).toBe('active')
    expect(status?.closeoutSubmitted).toBe(true)
    expect(status?.proofObserved).toBe(true)
    expect(status?.verifyResult).toBe('pass')
    expect(status?.failureSummary).toBeNull()
    expect(status?.proofRefs).toContain('proof.public.pylon_assignment.001')
  })

  test('a rejected closeout is a FAIL with a redacted failure summary', async () => {
    const store = fakeStore({
      assignment: {
        ...baseAssignment,
        rejectionRefs: ['rejection.public.pylon_assignment.verify_failed'],
        state: 'closeout_submitted',
      },
    })
    const status = await reader(store)(baseAssignment.assignmentRef)
    expect(status?.verifyResult).toBe('fail')
    expect(status?.failureSummary).toContain('rejected')
    expect(status?.failureSummary).toContain('verify_failed')
    expect(status?.rejectionRefs).toContain(
      'rejection.public.pylon_assignment.verify_failed',
    )
  })

  test('a blocker in an event body drives a FAIL verdict', async () => {
    const store = fakeStore({
      assignment: { ...baseAssignment, state: 'running' },
      events: [
        {
          ...closeoutEvent,
          eventBody: {
            blockerRefs: ['blocker.public.pylon_assignment.network'],
          },
          eventKind: 'assignment_progress',
          status: 'blocked',
        },
      ],
    })
    const status = await reader(store)(baseAssignment.assignmentRef)
    expect(status?.verifyResult).toBe('fail')
    expect(status?.blockerRefs).toContain(
      'blocker.public.pylon_assignment.network',
    )
  })

  test('an in-progress assignment with no proof reads as unknown', async () => {
    const store = fakeStore({
      assignment: {
        ...baseAssignment,
        artifactRefs: [],
        proofRefs: [],
        state: 'running',
      },
      events: [],
    })
    const status = await reader(store)(baseAssignment.assignmentRef)
    expect(status?.verifyResult).toBe('unknown')
    expect(status?.closeoutSubmitted).toBe(false)
    expect(status?.proofObserved).toBe(false)
  })

  test('owner scoping: an assignment owned by a different owner reads as null', async () => {
    // The owner has NO linked agents that match the assignment owner.
    const status = await reader(fakeStore(), ['agent:someone-else'])(
      baseAssignment.assignmentRef,
    )
    expect(status).toBeNull()
  })

  test('a missing assignment reads as null (honest absence)', async () => {
    const status = await reader(fakeStore({ assignment: undefined }))(
      baseAssignment.assignmentRef,
    )
    expect(status).toBeNull()
  })
})

const fakeRegistration = (
  pylonRef: string,
): PylonApiRegistrationRecord =>
  ({
    ownerAgentUserId: LINKED_AGENT,
    pylonRef,
  }) as unknown as PylonApiRegistrationRecord

const listerStore = (
  assignments: ReadonlyArray<PylonApiAssignmentRecord>,
  registrations: ReadonlyArray<PylonApiRegistrationRecord> = [
    fakeRegistration(baseAssignment.pylonRef),
  ],
): PylonApiStore =>
  ({
    listAssignmentsForPylons: async () => assignments,
    listRegistrationsForOwnerAgentUserIds: async () => registrations,
  }) as unknown as PylonApiStore

describe('makeArtanisPylonAssignmentsLister', () => {
  const lister = (
    store: PylonApiStore,
    linked: ReadonlyArray<string> = [LINKED_AGENT],
  ) =>
    makeArtanisPylonAssignmentsLister({
      listLinkedAgentUserIds: async () => linked,
      nowIso: () => '2026-06-26T00:12:00.000Z',
      ownerOpenAuthUserId: OWNER,
      pylonStore: store,
    })

  test('summarizes the owner own assignments newest-first with derived phase/verdict', async () => {
    const rows: ReadonlyArray<PylonApiAssignmentRecord> = [
      {
        ...baseAssignment,
        assignmentRef: 'assignment.public.pylon_api.accepted_001',
        artifactRefs: [],
        proofRefs: [],
        state: 'accepted',
        updatedAt: '2026-06-26T00:01:00.000Z',
      },
      {
        ...baseAssignment,
        assignmentRef: 'assignment.public.pylon_api.closeout_003',
        state: 'closeout_submitted',
        updatedAt: '2026-06-26T00:09:00.000Z',
      },
      {
        ...baseAssignment,
        assignmentRef: 'assignment.public.pylon_api.rejected_004',
        rejectionRefs: ['rejection.public.pylon_assignment.verify_failed'],
        state: 'rejected',
        updatedAt: '2026-06-26T00:05:00.000Z',
      },
    ]
    const summaries = await lister(listerStore(rows))(25)

    // Newest-first by updatedAt.
    expect(summaries.map(s => s.assignmentRef)).toEqual([
      'assignment.public.pylon_api.closeout_003',
      'assignment.public.pylon_api.rejected_004',
      'assignment.public.pylon_api.accepted_001',
    ])
    const byRef = new Map(summaries.map(s => [s.assignmentRef, s]))
    expect(byRef.get('assignment.public.pylon_api.closeout_003')?.verifyResult).toBe(
      'pass',
    )
    expect(byRef.get('assignment.public.pylon_api.closeout_003')?.phase).toBe(
      'proof-ready',
    )
    expect(byRef.get('assignment.public.pylon_api.rejected_004')?.verifyResult).toBe(
      'fail',
    )
    expect(byRef.get('assignment.public.pylon_api.accepted_001')?.verifyResult).toBe(
      'unknown',
    )
    expect(byRef.get('assignment.public.pylon_api.accepted_001')?.phase).toBe(
      'accepted',
    )
  })

  test('owner scoping: drops an assignment not owned by a linked credential', async () => {
    const rows: ReadonlyArray<PylonApiAssignmentRecord> = [
      { ...baseAssignment, ownerAgentUserId: 'agent:someone-else' },
    ]
    const summaries = await lister(listerStore(rows))(25)
    expect(summaries).toEqual([])
  })

  test('an owner with no linked agents lists nothing (honest absence)', async () => {
    const summaries = await lister(listerStore([baseAssignment]), [])(25)
    expect(summaries).toEqual([])
  })

  test('no registrations -> no pylons -> empty list', async () => {
    const summaries = await lister(listerStore([baseAssignment], []))(25)
    expect(summaries).toEqual([])
  })
})
