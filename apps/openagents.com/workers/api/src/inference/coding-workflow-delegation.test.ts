import { describe, expect, test } from 'vitest'

import type {
  PylonApiAssignmentRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from '../pylon-api'
import {
  type CodingDelegationAssignmentResult,
  delegateCodingWorkflow,
} from './coding-workflow-delegation'

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
    updateAssignmentIfState: async (record, expectedState) => {
      const index = assignments.findIndex(
        item => item.assignmentRef === record.assignmentRef,
      )

      if (index < 0 || assignments[index]?.state !== expectedState) {
        return undefined
      }

      assignments[index] = record
      return record
    },
    upsertProviderJobLifecycle: async record => record,
    upsertRegistration: async record => record,
  }
}

const classification = {
  confidence: 1,
  evidenceRefs: ['evidence.coding_workflow.structured_body'],
  workflowClass: 'codex_agent_task',
} as const

const linkedOwner = {
  agentUserId: 'agent_owner',
  credentialId: 'agent_credential_owner',
  displayName: 'Owner agent',
  linkKind: 'credential_anchor',
  openauthUserId: 'user_owner',
  tokenPrefix: 'oa_agent_owner',
} as const

const expectAssigned = (
  result: Awaited<ReturnType<typeof delegateCodingWorkflow>>,
): CodingDelegationAssignmentResult => {
  expect(result?.kind).toBe('assigned')
  return result as CodingDelegationAssignmentResult
}

describe('coding workflow delegation', () => {
  test('creates a controlled assignment on caller-owned Codex capacity', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({ registrations: [registration()] }),
      rawBody: {},
      requestId: 'chatcmpl_coding_1',
    })
    const assigned = expectAssigned(result)

    expect(assigned.assignment.ownerAgentUserId).toBe('agent_owner')
    expect(assigned.assignment.pylonRef).toBe('pylon.owner.codex')
    expect(assigned.assignment.jobKind).toBe('codex_agent_task')
    expect(assigned.assignment.codingAssignment?.codex).toMatchObject({
      agentKind: 'codex_sdk',
      schema: 'openagents.pylon.codex_agent_task.v0.3',
    })
  })

  test('carries a public-safe workspace and objective into the Codex assignment', async () => {
    const workspace = {
      kind: 'git_checkout',
      repository: {
        branch: 'main',
        commitSha: '7ab7cb401803f6e04a6c93b7aa9102405de66419',
        fullName: 'OpenAgentsInc/openagents',
        provider: 'github',
        visibility: 'public',
      },
      verificationCommand: {
        args: [
          'bun',
          'run',
          '--cwd',
          'apps/openagents.com/workers/api',
          'test',
          '--',
          'src/inference/coding-workflow-delegation.test.ts',
        ],
        commandRef: 'command.public.pylon_khala.delegation_test',
      },
    }
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({ registrations: [registration()] }),
      rawBody: {
        openagents: {
          coding: {
            objectiveSummary:
              'Implement the public-safe issue slice and run the named verification command.',
            targetPylonRef: 'pylon.owner.codex',
            workspace,
          },
        },
      },
      requestId: 'chatcmpl_coding_workspace',
    })
    const assigned = expectAssigned(result)

    expect(assigned.assignment.codingAssignment).toMatchObject({
      codex: {
        agentKind: 'codex_sdk',
        schema: 'openagents.pylon.codex_agent_task.v0.3',
      },
      objective: {
        publicSummary:
          'Implement the public-safe issue slice and run the named verification command.',
      },
      workspace,
    })
    expect(assigned.assignment.codingAssignment?.codex).not.toHaveProperty(
      'fixtureRef',
    )
  })

  test('rejects unsafe objective summaries before assignment creation', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({ registrations: [registration()] }),
      rawBody: {
        openagents: {
          coding: {
            objectiveSummary: 'Use Bearer secret from /Users/example/.env',
            targetPylonRef: 'pylon.owner.codex',
          },
        },
      },
      requestId: 'chatcmpl_coding_unsafe_objective',
    })

    expect(result).toMatchObject({
      error: 'invalid_coding_objective_summary',
      kind: 'rejected',
      statusCode: 400,
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

  test('rejects an explicit attempt to target another account Pylon', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({
        registrations: [
          registration(),
          registration({
            ownerAgentCredentialId: 'agent_credential_other',
            ownerAgentTokenPrefix: 'oa_agent_other',
            ownerAgentUserId: 'agent_other',
            pylonRef: 'pylon.other.codex',
          }),
        ],
      }),
      rawBody: {
        openagents: {
          coding: {
            targetPylonRef: 'pylon.other.codex',
          },
        },
      },
      requestId: 'chatcmpl_coding_cross_account',
    })

    expect(result).toMatchObject({
      error: 'target_pylon_not_authorized',
      kind: 'rejected',
      requestedPylonRef: 'pylon.other.codex',
      statusCode: 403,
    })
  })

  test('explicit target authorization is origin-independent for local and remote issuers', async () => {
    const issue = (origin: 'local' | 'remote') =>
      delegateCodingWorkflow({
        classification,
        linkedAgents: [linkedOwner],
        makeId: () => `id-${origin}`,
        nowIso,
        pylonStore: makeStore({ registrations: [registration()] }),
        rawBody: {
          openagents: {
            coding: {
              issuerOrigin: origin,
              pylonRef: 'pylon.owner.codex',
            },
          },
        },
        requestId: `chatcmpl_coding_${origin}`,
      })

    const local = expectAssigned(await issue('local'))
    const remote = expectAssigned(await issue('remote'))

    expect(local.pylon.pylonRef).toBe('pylon.owner.codex')
    expect(remote.pylon.pylonRef).toBe(local.pylon.pylonRef)
    expect(remote.assignment.ownerAgentUserId).toBe(
      local.assignment.ownerAgentUserId,
    )
  })

  test('inherits duplicate active assignment blocking from the shared gate', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
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

  test('allows same-account parallel coding delegation up to advertised Codex slots', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({
        activeAssignments: [assignment()],
        registrations: [
          registration({
            latestCapacityRefs: [
              'capacity.coding.codex.ready=2',
              'capacity.coding.codex.available=2',
            ],
            latestLoadRefs: [
              'load.coding.codex.busy=0',
              'load.coding.codex.queued=0',
            ],
          }),
        ],
      }),
      rawBody: {},
      requestId: 'chatcmpl_coding_parallel_2',
    })

    expect(result?.kind).toBe('assigned')
  })

  test('blocks same-account coding delegation once advertised Codex slots are reserved', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({
        activeAssignments: [
          assignment({
            assignmentRef: 'assignment.public.test.active_one',
            id: 'pylon_api_assignment_active_one',
          }),
          assignment({
            assignmentRef: 'assignment.public.test.active_two',
            id: 'pylon_api_assignment_active_two',
          }),
        ],
        registrations: [
          registration({
            latestCapacityRefs: [
              'capacity.coding.codex.ready=2',
              'capacity.coding.codex.available=2',
            ],
            latestLoadRefs: [
              'load.coding.codex.busy=0',
              'load.coding.codex.queued=0',
            ],
          }),
        ],
      }),
      rawBody: {},
      requestId: 'chatcmpl_coding_parallel_full',
    })

    expect(result).toBeNull()
  })

  test('returns typed refusal when a targeted linked Pylon is blocked by active assignment capacity', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({
        activeAssignments: [
          assignment({
            assignmentRef: 'assignment.public.test.stale_active_slot',
            id: 'pylon_api_assignment_stale_active_slot',
            updatedAt: '2026-06-25T11:00:00.000Z',
          }),
        ],
        registrations: [registration()],
      }),
      rawBody: {
        openagents: {
          coding: {
            targetPylonRef: 'pylon.owner.codex',
          },
        },
      },
      requestId: 'chatcmpl_coding_target_capacity_full',
    })

    expect(result).toMatchObject({
      error: 'target_pylon_unavailable',
      evidenceRefs: expect.arrayContaining([
        'evidence.khala_coding.target_pylon_ref.dispatch_gate_blocked',
        'blocker.public.pylon_dispatch.duplicate_active_assignment',
      ]),
      kind: 'rejected',
      requestedPylonRef: 'pylon.owner.codex',
      statusCode: 409,
    })
  })

  test('does not treat submitted closeout evidence as active Codex capacity', async () => {
    const result = await delegateCodingWorkflow({
      classification,
      linkedAgents: [linkedOwner],
      makeId: () => 'id1',
      nowIso,
      pylonStore: makeStore({
        activeAssignments: [
          assignment({
            artifactRefs: ['artifact.public.previous_patch'],
            closeoutRefs: ['closeout.public.previous_summary'],
            proofRefs: ['proof.public.previous_test'],
            state: 'closeout_submitted',
          }),
        ],
        registrations: [registration()],
      }),
      rawBody: {
        openagents: {
          coding: {
            targetPylonRef: 'pylon.owner.codex',
          },
        },
      },
      requestId: 'chatcmpl_coding_after_closeout',
    })

    expect(result?.kind).toBe('assigned')
    if (result?.kind !== 'assigned') {
      throw new Error('expected delegated coding assignment')
    }
    expect(result.pylon.pylonRef).toBe('pylon.owner.codex')
  })
})
