import { describe, expect, test } from 'vitest'

import type {
  AgentRegistrationStore,
  ProgrammaticAgentSession,
} from './agent-registration'
import {
  emptyCrmMcpCatalog,
  type CrmMcpCatalog,
} from './crm-mcp-routes'
import {
  combineMcpCatalogs,
  khalaMcpAgentPrincipal,
  makeKhalaMcpCatalog,
} from './khala-mcp'
import { khalaCodingRequestIdRef } from './inference/coding-workflow-delegation'
import type { ServedTokensRecorderInput } from './inference/served-tokens-recorder'
import type {
  PylonApiAssignmentRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'

const nowIso = '2026-06-25T12:00:00.000Z'
const request = new Request('https://openagents.com/api/mcp', {
  headers: { authorization: 'Bearer oa_agent_test' },
  method: 'POST',
})
type TestEnv = Readonly<Record<string, never>>
const env: TestEnv = {}

const session: ProgrammaticAgentSession = {
  credential: {
    id: 'agent_credential_owner',
    lastUsedAt: nowIso,
    openauthUserId: 'user_owner',
    profileMetadataJson: '{}',
    tokenPrefix: 'oa_agent_owner',
  },
  user: {
    avatarUrl: null,
    createdAt: nowIso,
    displayName: 'Owner Agent',
    id: 'agent_owner',
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: nowIso,
  },
}

const principal = khalaMcpAgentPrincipal(session, nowIso)

const privateOnlyPrincipal = {
  ...principal,
  grants: principal.grants.filter(
    grant => grant.authorityClass === 'private_account_read',
  ),
}

const agentStore = (): AgentRegistrationStore => ({
  createAgentRegistration: async () => {},
  findAgentByTokenHash: async () => undefined,
  listLinkedAgentsForOpenAuthUser: async () => [
    {
      agentUserId: 'agent_owner',
      credentialId: 'agent_credential_owner',
      displayName: 'Owner Agent',
      linkKind: 'credential_anchor',
      openauthUserId: 'user_owner',
      tokenPrefix: 'oa_agent_owner',
    },
  ],
  touchAgentCredential: async () => {},
  updateAgentDisplayName: async () => 1,
})

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
  acceptanceCriteriaRefs: ['acceptance.public.khala_coding.owner_requested'],
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: 'assignment.public.khala_coding.assignment_id',
  closeoutRefs: [],
  codingAssignment: null,
  createdAt: nowIso,
  id: 'pylon_api_assignment_1',
  idempotencyKeyHash: 'khala-coding:chatcmpl_mcp',
  jobKind: 'codex_agent_task',
  leaseExpiresAt: '2026-06-25T13:00:00.000Z',
  ownerAgentUserId: 'agent_owner',
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef: 'pylon.owner.codex',
  rejectionRefs: [],
  resultExpectationRefs: ['result.public.khala_coding.worker_closeout'],
  state: 'offered',
  taskRefs: [khalaCodingRequestIdRef('chatcmpl_mcp')],
  updatedAt: nowIso,
  ...overrides,
})

const makeStore = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord> = [registration()],
  seededAssignments: ReadonlyArray<PylonApiAssignmentRecord> = [],
): PylonApiStore => {
  const assignments: Array<PylonApiAssignmentRecord> = [...seededAssignments]
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
    listRegistrations: async () => registrations,
    listRegistrationsForOwnerAgentUserIds: async ownerAgentUserIds =>
      registrations.filter(item =>
        ownerAgentUserIds.includes(item.ownerAgentUserId),
      ),
    readAssignment: async () => undefined,
    readAssignmentByIdempotencyKeyHash: async () => undefined,
    readEventByIdempotencyKeyHash: async () => undefined,
    readRegistration: async pylonRef =>
      registrations.find(item => item.pylonRef === pylonRef),
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

const catalogFor = (
  options: Readonly<{
    assignments?: ReadonlyArray<PylonApiAssignmentRecord>
    durableFetch?: typeof fetch
    ids?: string[]
    recordedTokens?: Array<ServedTokensRecorderInput>
    registrations?: ReadonlyArray<PylonApiRegistrationRecord>
  }> = {},
) => {
  const ids = [...(options.ids ?? ['chatcmpl_mcp', 'assignment_id'])]
  const store = makeStore(
    options.registrations ?? [registration()],
    options.assignments ?? [],
  )
  return makeKhalaMcpCatalog<TestEnv>({
    agentStore: () => agentStore(),
    durableFetch: options.durableFetch ?? fetch,
    makeId: () => ids.shift() ?? 'id_more',
    nowIso: () => nowIso,
    pylonStore: () => store,
    recordTokensServed: () => async input => {
      options.recordedTokens?.push(input)
    },
  })
}

describe('Khala MCP catalog', () => {
  test('tools/list exposes Khala tools and grant-filters request authority', async () => {
    const catalog = catalogFor()

    const tools = await catalog.listTools(env, request, principal)
    expect(tools.map(tool => tool.name).sort()).toEqual([
      'khala.capacity',
      'khala.request',
      'khala.resume',
      'khala.spawn',
      'khala.spawnStatus',
      'khala.status',
    ])
    expect(tools.find(tool => tool.name === 'khala.spawn')).toMatchObject({
      inputSchema: {
        properties: {
          count: {
            maximum: 20,
            minimum: 1,
            type: 'integer',
          },
          maxParallel: {
            maximum: 20,
            minimum: 1,
            type: 'integer',
          },
        },
        required: ['count'],
      },
    })
    expect(
      (await catalog.listTools(env, request, privateOnlyPrincipal)).map(
        tool => tool.name,
      ).sort(),
    ).toEqual([
      'khala.capacity',
      'khala.resume',
      'khala.spawnStatus',
      'khala.status',
    ])
  })

  test('khala.capacity projects only linked caller-owned Pylon capacity', async () => {
    const outcome = await catalogFor().callTool(
      env,
      request,
      principal,
      'khala.capacity',
      {},
    )

    expect(outcome.isError).toBeFalsy()
    const body = outcome.structuredContent as {
      pylons: Array<{
        codingCapacity: Array<{ available: number; service: string }>
        pylonRef: string
      }>
    }
    expect(body.pylons).toHaveLength(1)
    expect(body.pylons[0]?.pylonRef).toBe('pylon.owner.codex')
    expect(body.pylons[0]?.codingCapacity).toContainEqual({
      accounts: [],
      available: 1,
      busy: 0,
      queued: 0,
      ready: 1,
      service: 'codex',
    })
  })

  test('khala.request delegates through own linked Pylon capacity and returns a durable handle', async () => {
    const recordedTokens: Array<ServedTokensRecorderInput> = []
    const outcome = await catalogFor({ recordedTokens }).callTool(
      env,
      request,
      principal,
      'khala.request',
      {
        prompt: 'Fix the failing test',
        targetPylonRef: 'pylon.owner.codex',
        workflow: 'codex_agent_task',
      },
    )

    // #6325: MCP delegation must not meter a handoff estimate. The local
    // Pylon/Codex executor records exact downstream SDK turn usage through the
    // registered-agent ingest route after Codex actually runs.
    expect(recordedTokens).toHaveLength(0)
    expect(outcome.isError).toBeFalsy()
    expect(outcome.structuredContent).toMatchObject({
      assignmentRef: 'assignment.public.khala_coding.assignment_id',
      durableRequestId: 'chatcmpl_mcp',
      durableStreamUrl: '/v1/chat/completions/durable/chatcmpl_mcp',
      ok: true,
      pylonRef: 'pylon.owner.codex',
      schema: 'openagents.khala_mcp.request.v1',
      stream: true,
      workflow: 'codex_agent_task',
    })
  })

  test('khala.request returns isError when an explicit target is not caller-owned', async () => {
    const recordedTokens: Array<ServedTokensRecorderInput> = []
    const outcome = await catalogFor({
      recordedTokens,
      registrations: [
        registration(),
        registration({
          ownerAgentCredentialId: 'agent_credential_other',
          ownerAgentTokenPrefix: 'oa_agent_other',
          ownerAgentUserId: 'agent_other',
          pylonRef: 'pylon.other.codex',
        }),
      ],
    }).callTool(env, request, principal, 'khala.request', {
      prompt: 'Use another account capacity',
      targetPylonRef: 'pylon.other.codex',
      workflow: 'codex_agent_task',
    })

    expect(outcome.isError).toBe(true)
    expect(recordedTokens).toHaveLength(0)
    expect(outcome.structuredContent).toMatchObject({
      error: 'target_pylon_not_authorized',
      ok: false,
      requestedPylonRef: 'pylon.other.codex',
      statusCode: 403,
    })
  })

  test('khala.request returns typed gate refusal when linked Pylon capacity is consumed by an active assignment', async () => {
    const recordedTokens: Array<ServedTokensRecorderInput> = []
    const outcome = await catalogFor({
      assignments: [
        // a5ce0fc50c (#6386): an `offered` lease only consumes a slot inside the
        // fresh-offered claim window, so this seed must be freshly touched
        // (updatedAt = now) to represent capacity actually held by an active
        // assignment. A stale offered lease is intentionally NOT counted.
        assignment({
          assignmentRef: 'assignment.public.khala_coding.fresh_active_slot',
          id: 'pylon_api_assignment_fresh_active_slot',
          updatedAt: nowIso,
        }),
      ],
      recordedTokens,
    }).callTool(env, request, principal, 'khala.request', {
      prompt: 'Run the public issue task',
      targetPylonRef: 'pylon.owner.codex',
      workflow: 'codex_agent_task',
    })

    expect(outcome.isError).toBe(true)
    expect(recordedTokens).toHaveLength(0)
    expect(outcome.structuredContent).toMatchObject({
      error: 'target_pylon_unavailable',
      evidenceRefs: expect.arrayContaining([
        'evidence.khala_coding.target_pylon_ref.dispatch_gate_blocked',
        'blocker.public.pylon_dispatch.duplicate_active_assignment',
      ]),
      ok: false,
      requestedPylonRef: 'pylon.owner.codex',
      statusCode: 409,
    })
  })

  test('khala.spawn creates bounded child assignments and khala.spawnStatus aggregates them', async () => {
    const catalog = catalogFor({
      ids: [
        'spawn_mcp',
        'chatcmpl_spawn_one',
        'assignment_spawn_one',
        'record_one',
        'row_one',
        'chatcmpl_spawn_two',
        'assignment_spawn_two',
        'record_two',
        'row_two',
      ],
      registrations: [
        registration({
          latestCapacityRefs: [
            'capacity.coding.codex.ready=2',
            'capacity.coding.codex.available=2',
          ],
        }),
      ],
    })
    const spawned = await catalog.callTool(
      env,
      request,
      principal,
      'khala.spawn',
      {
        count: 2,
        fixture: true,
        objective: 'Audit the public fixture through two child workers',
        targetPylonRef: 'pylon.owner.codex',
      },
    )

    expect(spawned.isError).toBeFalsy()
    expect(spawned.structuredContent).toMatchObject({
      assignedCount: 2,
      ok: true,
      requestedCount: 2,
      schema: 'openagents.khala_mcp.spawn.v1',
      spawnRef: 'spawn.public.khala_coding.spawn_mcp',
      workflow: 'codex_agent_task',
    })
    const spawnBody = spawned.structuredContent as {
      children: Array<{ assignmentRef: string; durableRequestId: string }>
      spawnRef: string
    }
    expect(spawnBody.children).toMatchObject([
      {
        assignmentRef: 'assignment.public.khala_coding.assignment_spawn_one',
        durableRequestId: 'chatcmpl_spawn_one',
      },
      {
        assignmentRef: 'assignment.public.khala_coding.assignment_spawn_two',
        durableRequestId: 'chatcmpl_spawn_two',
      },
    ])

    const status = await catalog.callTool(
      env,
      request,
      principal,
      'khala.spawnStatus',
      { spawnRef: spawnBody.spawnRef },
    )
    expect(status.isError).toBeFalsy()
    expect(status.structuredContent).toMatchObject({
      childCount: 2,
      ok: true,
      schema: 'openagents.khala_mcp.spawn_status.v1',
      spawnRef: spawnBody.spawnRef,
      state: 'active',
    })
    expect(JSON.stringify(status.structuredContent)).not.toContain('rawEvents')
  })

  test('khala.spawn reports capacity detail when requested count exceeds availability', async () => {
    const outcome = await catalogFor({
      ids: [
        'spawn_shortfall',
        'chatcmpl_shortfall_one',
        'assignment_shortfall_one',
        'record_one',
        'row_one',
        'chatcmpl_shortfall_two',
        'assignment_shortfall_two',
        'record_two',
        'row_two',
      ],
      registrations: [
        registration({
          latestCapacityRefs: [
            'capacity.coding.codex.ready=2',
            'capacity.coding.codex.available=2',
          ],
        }),
      ],
    }).callTool(env, request, principal, 'khala.spawn', {
      count: 3,
      fixture: true,
      objective: 'Try to launch more public fixture workers than available',
      targetPylonRef: 'pylon.owner.codex',
    })

    expect(outcome.isError).toBeFalsy()
    expect(outcome.structuredContent).toMatchObject({
      assignedCount: 2,
      blockerRefs: expect.arrayContaining([
        'blocker.khala_mcp.spawn.capacity_shortfall',
      ]),
      capacity: {
        advertisedAvailableCount: 2,
        readyCount: 2,
      },
      ok: false,
      requestedCount: 3,
      spawnRef: 'spawn.public.khala_coding.spawn_shortfall',
    })
  })

  test('khala.spawn returns a typed auth error for a cross-owner target Pylon', async () => {
    const outcome = await catalogFor({
      registrations: [
        registration(),
        registration({
          ownerAgentCredentialId: 'agent_credential_other',
          ownerAgentTokenPrefix: 'oa_agent_other',
          ownerAgentUserId: 'agent_other',
          pylonRef: 'pylon.other.codex',
        }),
      ],
    }).callTool(env, request, principal, 'khala.spawn', {
      count: 2,
      objective: 'Try to use another account capacity',
      targetPylonRef: 'pylon.other.codex',
    })

    expect(outcome.isError).toBe(true)
    expect(outcome.structuredContent).toMatchObject({
      error: 'target_pylon_not_authorized',
      ok: false,
      requestedPylonRef: 'pylon.other.codex',
      statusCode: 403,
    })
  })

  test('khala.resume reads the durable stream route without metering', async () => {
    const seen: string[] = []
    const durableFetch: typeof fetch = async input => {
      seen.push(input instanceof Request ? input.url : String(input))
      return new Response('data: [DONE]\n\n', {
        headers: {
          'stream-closed': 'true',
          'stream-next-offset': '128',
          'stream-up-to-date': 'true',
        },
      })
    }
    const catalog = catalogFor({ durableFetch })
    const issued = await catalog.callTool(
      env,
      request,
      principal,
      'khala.request',
      {
        prompt: 'Fix the failing test',
        targetPylonRef: 'pylon.owner.codex',
        workflow: 'codex_agent_task',
      },
    )
    expect(issued.isError).toBeFalsy()

    const outcome = await catalog.callTool(
      env,
      request,
      principal,
      'khala.resume',
      { durableRequestId: 'chatcmpl_mcp', offset: 64 },
    )

    expect(outcome.isError).toBeFalsy()
    expect(seen[0]).toBe(
      'https://openagents.com/v1/chat/completions/durable/chatcmpl_mcp?offset=64',
    )
    expect(outcome.structuredContent).toMatchObject({
      durableRequestId: 'chatcmpl_mcp',
      nextOffset: '128',
      ok: true,
      streamClosed: true,
      streamUpToDate: true,
    })
  })

  test('khala.resume rejects durable handles not attached to the caller-owned Pylon assignments', async () => {
    const seen: string[] = []
    const durableFetch: typeof fetch = async input => {
      seen.push(input instanceof Request ? input.url : String(input))
      return new Response('data: [DONE]\n\n')
    }

    const outcome = await catalogFor({
      assignments: [
        assignment({
          ownerAgentUserId: 'agent_other',
          pylonRef: 'pylon.other.codex',
        }),
      ],
      durableFetch,
      registrations: [
        registration(),
        registration({
          ownerAgentCredentialId: 'agent_credential_other',
          ownerAgentTokenPrefix: 'oa_agent_other',
          ownerAgentUserId: 'agent_other',
          pylonRef: 'pylon.other.codex',
        }),
      ],
    }).callTool(
      env,
      request,
      principal,
      'khala.resume',
      { durableRequestId: 'chatcmpl_mcp' },
    )

    expect(outcome.isError).toBe(true)
    expect(seen).toEqual([])
    expect(outcome.structuredContent).toMatchObject({
      durableRequestId: 'chatcmpl_mcp',
      error: 'durable_request_not_authorized',
      ok: false,
      statusCode: 403,
    })
  })

  test('composed MCP catalog lists and routes both CRM and Khala tools', async () => {
    const crmCatalog: CrmMcpCatalog<TestEnv> = {
      ...emptyCrmMcpCatalog<TestEnv>(),
      callTool: (_env, _request, _principal, name) =>
        name === 'crm.fixture'
          ? Promise.resolve({
              content: [{ text: 'crm ok', type: 'text' as const }],
              structuredContent: { ok: true },
            })
          : Promise.reject(new Error('unknown_tool')),
      listTools: () =>
        Promise.resolve([
          {
            description: 'Fixture',
            inputSchema: { type: 'object' },
            name: 'crm.fixture',
            title: 'Fixture',
          },
        ]),
    }

    const composed = combineMcpCatalogs<TestEnv>([crmCatalog, catalogFor()])
    const tools = await composed.listTools(env, request, principal)
    expect(tools.map(tool => tool.name)).toContain('crm.fixture')
    expect(tools.map(tool => tool.name)).toContain('khala.request')
    expect(
      await composed.callTool(env, request, principal, 'crm.fixture', {}),
    ).toMatchObject({ structuredContent: { ok: true } })
    expect(
      await composed.callTool(env, request, principal, 'khala.capacity', {}),
    ).toMatchObject({ isError: false })
  })
})
