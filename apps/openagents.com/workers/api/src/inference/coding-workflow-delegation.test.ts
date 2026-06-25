import { describe, expect, test } from 'vitest'

import type {
  PylonApiAssignmentRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from '../pylon-api'
import { delegateCodingWorkflow } from './coding-workflow-delegation'

const nowIso = '2026-06-25T12:00:00.000Z'

const registration = (
  overrides: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: ['capability.pylon.local_codex'],
  clientProtocolVersion: '0.3.0',
  clientVersion: '0.3.0',
  createdAt: nowIso,
  displayName: 'Linked Codex Pylon',
  id: 'pylon_api_registration_1',
  latestCapacityRefs: [
    'capacity.coding.codex.ready=1',
    'capacity.coding.codex.available=1',
  ],
  latestHeartbeatAt: nowIso,
  latestHeartbeatStatus: 'online',
  latestHealthRefs: ['health.public.pylon_cli.ok'],
  latestLoadRefs: ['load.coding.codex.busy=0', 'load.coding.codex.queued=0'],
  latestResourceMode: 'background_20',
  ownerAgentCredentialId: 'agent_credential_owner',
  ownerAgentTokenPrefix: 'oa_agent_owner',
  ownerAgentUserId: 'agent_owner',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef: 'pylon.owner.codex',
  resourceMode: 'background_20',
  status: 'active',
  updatedAt: nowIso,
  walletReady: true,
  walletRef: null,
  ...overrides,
})

const assignment = (
  overrides: Partial<PylonApiAssignmentRecord> = {},
): PylonApiAssignmentRecord => ({
  acceptanceCriteriaRefs: ['acceptance.public.test'],
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: 'assignment.public.test.active',
  closeoutRefs: [],
  codingAssignment: null,
  createdAt: nowIso,
  id: 'pylon_api_assignment_active',
  idempotencyKeyHash: 'active',
  jobKind: 'codex_agent_task',
  leaseExpiresAt: '2026-06-25T12:30:00.000Z',
  ownerAgentUserId: 'agent_owner',
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef: 'pylon.owner.codex',
  rejectionRefs: [],
  resultExpectationRefs: ['result.public.test'],
  state: 'offered',
  taskRefs: ['task.public.test'],
  updatedAt: nowIso,
  ...overrides,
})

const makeStore = (input: {
  activeAssignments?: ReadonlyArray<PylonApiAssignmentRecord>
  registrations: ReadonlyArray<PylonApiRegistrationRecord>
}): PylonApiStore => {
  const assignments: Array<PylonApiAssignmentRecord> = [
    ...(input.activeAssignments ?? []),
  ]

  return {
    createAssignment: async record => {
      assignments.push(record)
      return { idempotent: false, record }
    },
    createEvent: async () => {
      throw new Error('not used')
    },
    listAssignmentsForPylon: async pylonRef =>
      assignments.filter(item => item.pylonRef === pylonRef),
    listEventsForAssignment: async () => [],
    listEventsForPylon: async () => [],
    listProviderJobLifecycleForPylons: async () => [],
    listRegistrations: async () => input.registrations,
    listRegistrationsForOwnerAgentUserIds: async ownerAgentUserIds =>
      input.registrations.filter(item =>
        ownerAgentUserIds.includes(item.ownerAgentUserId),
      ),
    readAssignment: async () => undefined,
    readAssignmentByIdempotencyKeyHash: async () => undefined,
    readEventByIdempotencyKeyHash: async () => undefined,
    readRegistration: async pylonRef =>
      input.registrations.find(item => item.pylonRef === pylonRef),
    updateAssignment: async record => record,
    upsertProviderJobLifecycle: async record => record,
    upsertRegistration: async record => record,
  }
}

const classification = {
  confidence: 1,
  evidenceRefs: ['evidence.coding_workflow.structured_body'],
  workflowClass: 'codex_agent_task',
} as const

describe('coding workflow delegation', () => {
  test('creates a controlled assignment on caller-owned Codex capacity', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [
        {
          agentUserId: 'agent_owner',
          credentialId: 'agent_credential_owner',
          displayName: 'Owner agent',
          linkKind: 'credential_anchor',
          openauthUserId: 'user_owner',
          tokenPrefix: 'oa_agent_owner',
        },
      ],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({ registrations: [registration()] }),
      rawBody: {},
      requestId: 'chatcmpl_coding_1',
    })

    expect(result?.assignment.ownerAgentUserId).toBe('agent_owner')
    expect(result?.assignment.pylonRef).toBe('pylon.owner.codex')
    expect(result?.assignment.jobKind).toBe('codex_agent_task')
    expect(result?.assignment.codingAssignment?.codex).toMatchObject({
      agentKind: 'codex_sdk',
      schema: 'openagents.pylon.codex_agent_task.v0.3',
    })
  })

  test('does not use another OpenAuth user account capacity', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [
        {
          agentUserId: 'agent_elsewhere',
          credentialId: 'agent_credential_elsewhere',
          displayName: 'Other agent',
          linkKind: 'credential_anchor',
          openauthUserId: 'user_owner',
          tokenPrefix: 'oa_agent_other',
        },
      ],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({ registrations: [registration()] }),
      rawBody: {},
      requestId: 'chatcmpl_coding_2',
    })

    expect(result).toBeNull()
  })

  test('inherits duplicate active assignment blocking from the shared gate', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [
        {
          agentUserId: 'agent_owner',
          credentialId: 'agent_credential_owner',
          displayName: 'Owner agent',
          linkKind: 'credential_anchor',
          openauthUserId: 'user_owner',
          tokenPrefix: 'oa_agent_owner',
        },
      ],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({
        activeAssignments: [assignment()],
        registrations: [registration()],
      }),
      rawBody: {},
      requestId: 'chatcmpl_coding_3',
    })

    expect(result).toBeNull()
  })
})
