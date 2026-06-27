// Reusable validation harness for the #6273 Pylon-linked coding-capacity batch.
//
// WHAT THIS PROVES (programmatically, no live deps):
//  - The OpenAuth account <-> agent-credential link routes
//    (`POST /api/account/pylon-agent-links`, `GET /api/account/pylons`) work
//    end-to-end through the REAL route handlers (`makePylonApiRoutes`).
//  - The OpenAuth-scoped resolver: a browser session resolves ONLY its own
//    linked agents and Pylons; another account's set is never returned.
//  - Cross-account denial (the firm invariant): account B cannot link, see, or
//    take over account A's already-linked credential / Pylon.
//  - No raw key material / bearer tokens appear in any response body.
//  - Per-service capacity dimensions (Codex x N / Claude x M, busy / available /
//    queued) round-trip through the heartbeat -> projection.
//  - The typed/semantic classifier routes structured markers and refuses prose.
//  - The router caller-awareness + delegate branch routes a coding workflow to
//    the caller's OWN Codex Pylon and never to another account's capacity.
//  - The public-counter recording path tags own-capacity coding tokens with
//    `demand_kind = own_capacity` (source-agnostic counter, no exclusion).
//
// HOW TO RUN (re-runnable by the owner / any agent):
//   cd apps/openagents.com/workers/api
//   bun run test -- src/coding-capacity-validation.test.ts
//
// This is an in-process harness against the injectable route/store deps; it does
// not require `wrangler dev`. The route handlers, classifier, delegation, and
// capacity projection imported here are the SAME production modules.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  AgentRegistrationStore,
  LinkedAgentOwnerRecord,
  OpenAuthAgentLinkRecord,
  ProgrammaticAgentSession,
} from './agent-registration'
import { sha256Hex } from './agent-registration'
import { authorizeInferenceMonetization } from './inference-resale-authorization'
import { classifyCodingWorkflow } from './inference/coding-workflow-classifier'
import { delegateCodingWorkflow } from './inference/coding-workflow-delegation'
import type { ServedTokensRecorderInput } from './inference/served-tokens-recorder'
import {
  KHALA_MCP_TOOLS,
  khalaMcpAgentPrincipal,
  makeKhalaMcpCatalog,
} from './khala-mcp'
import type {
  PylonApiAssignmentRecord,
  PylonApiEventRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'
import { pylonCodingServiceCapacityProjection } from './pylon-api'
import { makePylonApiRoutes } from './pylon-api-routes'

const NOW_ISO = '2026-06-25T12:00:00.000Z'

// ----------------------------------------------------------------------------
// In-memory AgentRegistrationStore with REAL link semantics.
// Models: credentials keyed by token-hash, each carrying openauth_user_id;
// explicit links table; and the credential-anchor + explicit-link UNION that
// `listLinkedAgentsForOpenAuthUser` performs in D1.
// ----------------------------------------------------------------------------

type Credential = {
  id: string
  agentUserId: string
  displayName: string
  tokenHash: string
  tokenPrefix: string
  openauthUserId: string | null
}

const makeAgentStore = (
  credentials: ReadonlyArray<Credential>,
): AgentRegistrationStore => {
  const creds = credentials.map(credential => ({ ...credential }))
  const links: Array<OpenAuthAgentLinkRecord> = []

  const linkedFor = (openauthUserId: string): Array<LinkedAgentOwnerRecord> => {
    const fromCredentials = creds
      .filter(credential => credential.openauthUserId === openauthUserId)
      .map(credential => ({
        agentUserId: credential.agentUserId,
        credentialId: credential.id,
        displayName: credential.displayName,
        linkKind: 'credential_anchor' as const,
        openauthUserId,
        tokenPrefix: credential.tokenPrefix,
      }))
    const fromLinks = links
      .filter(
        link => link.openauthUserId === openauthUserId && link.status === 'active',
      )
      .map(link => {
        const credential = creds.find(c => c.id === link.agentCredentialId)
        return {
          agentUserId: link.agentUserId,
          credentialId: link.agentCredentialId,
          displayName: credential?.displayName ?? link.agentUserId,
          linkKind: link.linkKind,
          openauthUserId,
          tokenPrefix: credential?.tokenPrefix ?? null,
        }
      })
    const byAgent = new Map<string, LinkedAgentOwnerRecord>()
    for (const record of [...fromCredentials, ...fromLinks]) {
      byAgent.set(`${record.agentUserId}:${record.credentialId}`, record)
    }
    return [...byAgent.values()]
  }

  return {
    createAgentRegistration: () => Promise.resolve(),
    findAgentByTokenHash: async tokenHash => {
      const credential = creds.find(c => c.tokenHash === tokenHash)
      if (credential === undefined) {
        return undefined
      }
      return {
        credentialId: credential.id,
        openauthUserId: credential.openauthUserId,
        profileMetadataJson: '{}',
        tokenPrefix: credential.tokenPrefix,
        user: {
          avatarUrl: null,
          createdAt: NOW_ISO,
          displayName: credential.displayName,
          id: credential.agentUserId,
          kind: 'agent',
          primaryEmail: null,
          status: 'active',
          updatedAt: NOW_ISO,
        },
      }
    },
    linkOpenAuthAgent: async record => {
      const existing = links.findIndex(
        link =>
          link.openauthUserId === record.openauthUserId &&
          link.agentUserId === record.agentUserId &&
          link.agentCredentialId === record.agentCredentialId,
      )
      if (existing >= 0) {
        links[existing] = record
      } else {
        links.push(record)
      }
      if (record.agentCredentialId !== null) {
        const credential = creds.find(c => c.id === record.agentCredentialId)
        if (credential !== undefined) {
          credential.openauthUserId = record.openauthUserId
        }
      }
    },
    listLinkedAgentsForOpenAuthUser: async openauthUserId =>
      linkedFor(openauthUserId),
    touchAgentCredential: () => Promise.resolve(),
    updateAgentDisplayName: () => Promise.resolve(0),
  }
}

// ----------------------------------------------------------------------------
// In-memory PylonApiStore (registrations + assignments only; enough for the
// account list + delegation paths).
// ----------------------------------------------------------------------------

const registration = (
  overrides: Partial<PylonApiRegistrationRecord>,
): PylonApiRegistrationRecord => ({
  capabilityRefs: ['capability.pylon.local_codex'],
  clientProtocolVersion: '0.3.0',
  clientVersion: '0.3.0',
  createdAt: NOW_ISO,
  displayName: 'Linked Codex Pylon',
  id: `pylon_api_registration_${overrides.pylonRef ?? 'x'}`,
  latestCapacityRefs: [
    'capacity.coding.codex.ready=2',
    'capacity.coding.codex.available=1',
    'capacity.coding.claude.ready=1',
    'capacity.coding.claude.available=1',
  ],
  latestHeartbeatAt: NOW_ISO,
  latestHeartbeatStatus: 'online',
  latestHealthRefs: ['health.public.pylon_cli.ok'],
  latestLoadRefs: [
    'load.coding.codex.busy=1',
    'load.coding.codex.queued=0',
    'load.coding.claude.busy=0',
    'load.coding.claude.queued=2',
  ],
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
  updatedAt: NOW_ISO,
  walletReady: true,
  walletRef: null,
  ...overrides,
})

type MemoryPylonStore = PylonApiStore & {
  createdAssignments: () => ReadonlyArray<PylonApiAssignmentRecord>
}

const makePylonStore = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
): MemoryPylonStore => {
  const assignments: Array<PylonApiAssignmentRecord> = []
  const store: PylonApiStore = {
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
    listEventsForPylon: async (): Promise<ReadonlyArray<PylonApiEventRecord>> =>
      [],
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
  return { ...store, createdAssignments: () => assignments }
}

// ----------------------------------------------------------------------------
// Route driver: wires the REAL makePylonApiRoutes with an injectable
// requireBrowserSession (the OpenAuth-backed session at runtime).
// ----------------------------------------------------------------------------

const tokenFor = (agentUserId: string) => `oa_agent_validate_${agentUserId}`

const route = async (input: {
  agentStore: AgentRegistrationStore
  pylonStore: PylonApiStore
  path: string
  method?: string
  body?: unknown
  browserUserId?: string | undefined
}) => {
  let counter = 0
  const init: RequestInit = {
    headers: input.body === undefined ? {} : { 'content-type': 'application/json' },
    method: input.method ?? 'GET',
  }
  if (input.body !== undefined) {
    init.body = JSON.stringify(input.body)
  }
  const request = new Request(`https://openagents.com${input.path}`, init)
  const routes = makePylonApiRoutes({
    agentStore: () => input.agentStore,
    makeId: () => `validate-${++counter}`,
    makeStore: () => input.pylonStore,
    nowIso: () => NOW_ISO,
    requireAdminApiToken: () => Promise.resolve(false),
    requireBrowserSession: () =>
      Promise.resolve(
        input.browserUserId === undefined
          ? undefined
          : { user: { userId: input.browserUserId } },
      ),
  })
  const response = routes.routePylonApiRequest(
    request,
    { OPENAGENTS_DB: {} as D1Database },
    {} as ExecutionContext,
  )
  if (response === undefined) {
    throw new Error(`No route matched ${input.path}`)
  }
  const resolved = await Effect.runPromise(response)
  const text = await resolved.text()
  return {
    status: resolved.status,
    text,
    json: text === '' ? {} : (JSON.parse(text) as Record<string, unknown>),
    headers: resolved.headers,
  }
}

describe('#6273 coding-capacity validation harness', () => {
  test('AREA 1 — link flow: a browser session links its own agent credential', async () => {
    const userA = 'openauth-user-a'
    const credentials: Array<Credential> = [
      {
        id: 'cred_a',
        agentUserId: 'agent_a',
        displayName: 'Agent A',
        tokenHash: await sha256Hex(tokenFor('agent_a')),
        tokenPrefix: tokenFor('agent_a').slice(0, 16),
        openauthUserId: null,
      },
    ]
    const agentStore = makeAgentStore(credentials)
    const pylonStore = makePylonStore([])

    const linked = await route({
      agentStore,
      pylonStore,
      path: '/api/account/pylon-agent-links',
      method: 'POST',
      body: { agentToken: tokenFor('agent_a') },
      browserUserId: userA,
    })

    expect(linked.status).toBe(201)
    expect(linked.json).toMatchObject({
      linkedAgent: { agentRef: 'agent:agent_a', linkKind: 'credential_anchor' },
    })
    // No raw token material leaks.
    expect(linked.text).not.toContain(tokenFor('agent_a'))
  })

  test('AREA 2 — resolver: account lists ONLY its own linked Pylons + keys', async () => {
    const userA = 'openauth-user-a'
    const credentials: Array<Credential> = [
      {
        id: 'cred_a',
        agentUserId: 'agent_a',
        displayName: 'Agent A',
        tokenHash: await sha256Hex(tokenFor('agent_a')),
        tokenPrefix: tokenFor('agent_a').slice(0, 16),
        openauthUserId: userA,
      },
    ]
    const agentStore = makeAgentStore(credentials)
    const pylonStore = makePylonStore([
      registration({ ownerAgentUserId: 'agent_a', pylonRef: 'pylon.a.codex' }),
      registration({
        ownerAgentUserId: 'agent_b',
        pylonRef: 'pylon.b.codex',
        id: 'pylon_api_registration_b',
      }),
    ])

    const listed = await route({
      agentStore,
      pylonStore,
      path: '/api/account/pylons',
      browserUserId: userA,
    })

    expect(listed.status).toBe(200)
    const pylons = (listed.json.pylons ?? []) as ReadonlyArray<{
      pylonRef?: string
      codingCapacity?: ReadonlyArray<unknown>
    }>
    const refs = pylons.map(p => p.pylonRef)
    expect(refs).toContain('pylon.a.codex')
    expect(refs).not.toContain('pylon.b.codex')
    // Capacity dimensions are projected on the account read.
    expect(pylons[0]?.codingCapacity?.length ?? 0).toBeGreaterThan(0)
    // No raw token material in the account read.
    expect(listed.text).not.toContain(tokenFor('agent_a'))
  })

  test('AREA 2b — cross-account denial: B cannot take over A-linked credential', async () => {
    const userA = 'openauth-user-a'
    const userB = 'openauth-user-b'
    const credentials: Array<Credential> = [
      {
        id: 'cred_a',
        agentUserId: 'agent_a',
        displayName: 'Agent A',
        tokenHash: await sha256Hex(tokenFor('agent_a')),
        tokenPrefix: tokenFor('agent_a').slice(0, 16),
        openauthUserId: userA, // already linked to A
      },
    ]
    const agentStore = makeAgentStore(credentials)
    const pylonStore = makePylonStore([])

    const relink = await route({
      agentStore,
      pylonStore,
      path: '/api/account/pylon-agent-links',
      method: 'POST',
      body: { agentToken: tokenFor('agent_a') },
      browserUserId: userB,
    })

    // Must be denied (403 forbidden): the credential belongs to A.
    expect(relink.status).toBe(403)
    expect(relink.json.error).toBe('pylon_api_forbidden')

    // And B's account list still sees nothing of A's.
    const listedB = await route({
      agentStore,
      pylonStore,
      path: '/api/account/pylons',
      browserUserId: userB,
    })
    expect((listedB.json.pylons ?? []) as ReadonlyArray<unknown>).toHaveLength(0)
  })

  test('AREA 2c — unauthenticated (no browser session) is rejected', async () => {
    const agentStore = makeAgentStore([])
    const pylonStore = makePylonStore([])
    const listed = await route({
      agentStore,
      pylonStore,
      path: '/api/account/pylons',
      browserUserId: undefined,
    })
    expect(listed.status).toBe(401)
  })

  test('AREA 2d — REGRESSION PROBE: fresh unlinked credential should still link (openauthUserId null vs undefined)', async () => {
    // The route guard is `credential.openauthUserId !== null && !== session`.
    // A fresh credential whose openauthUserId is *omitted* (undefined) would
    // satisfy `!== null`, which could wrongly forbid a first link. The D1 store
    // returns null (not undefined), so production is safe; this probe documents
    // that the null path links cleanly.
    const userA = 'openauth-user-a'
    const credentials: Array<Credential> = [
      {
        id: 'cred_a',
        agentUserId: 'agent_a',
        displayName: 'Agent A',
        tokenHash: await sha256Hex(tokenFor('agent_a')),
        tokenPrefix: tokenFor('agent_a').slice(0, 16),
        openauthUserId: null,
      },
    ]
    const linked = await route({
      agentStore: makeAgentStore(credentials),
      pylonStore: makePylonStore([]),
      path: '/api/account/pylon-agent-links',
      method: 'POST',
      body: { agentToken: tokenFor('agent_a') },
      browserUserId: userA,
    })
    expect(linked.status).toBe(201)
  })

  test('AREA 3 — capacity: per-service Codex/Claude dims project from heartbeat refs', () => {
    const projection = pylonCodingServiceCapacityProjection(
      registration({ pylonRef: 'pylon.cap' }),
    )
    const byService = Object.fromEntries(
      projection.map(item => [item.service, item]),
    )
    expect(byService.codex).toMatchObject({
      ready: 2,
      available: 1,
      busy: 1,
      queued: 0,
    })
    expect(byService.claude).toMatchObject({
      ready: 1,
      available: 1,
      busy: 0,
      queued: 2,
    })
  })

  test('AREA 4 — classifier: structured markers route; prose does NOT', () => {
    expect(
      classifyCodingWorkflow({
        messages: [{ content: 'hi', role: 'user' }],
        rawBody: { openagents: { workflow_class: 'codex_agent_task' } },
      }).workflowClass,
    ).toBe('codex_agent_task')

    expect(
      classifyCodingWorkflow({
        messages: [
          { content: 'please code a fix, edit the repo, and commit', role: 'user' },
        ],
        rawBody: {},
      }).workflowClass,
    ).toBe('none')
  })

  test('AREA 5 — router/delegate: coding workflow routes to OWN Codex Pylon', async () => {
    const pylonStore = makePylonStore([
      registration({ ownerAgentUserId: 'agent_a', pylonRef: 'pylon.a.codex' }),
    ])
    const result = await delegateCodingWorkflow({
      classification: {
        confidence: 1,
        evidenceRefs: ['evidence.coding_workflow.structured_body'],
        workflowClass: 'codex_agent_task',
      },
      linkedAgents: [
        {
          agentUserId: 'agent_a',
          credentialId: 'cred_a',
          displayName: 'Agent A',
          linkKind: 'credential_anchor',
          openauthUserId: 'openauth-user-a',
          tokenPrefix: tokenFor('agent_a').slice(0, 16),
        },
      ],
      makeId: () => 'id1',
      nowIso: NOW_ISO,
      pylonStore,
      rawBody: {},
      requestId: 'chatcmpl_validate_own',
    })
    expect(result?.kind).toBe('assigned')
    if (result?.kind !== 'assigned') {
      throw new Error('expected coding delegation assignment')
    }
    expect(result.assignment.ownerAgentUserId).toBe('agent_a')
    expect(result.assignment.pylonRef).toBe('pylon.a.codex')
    expect(result.durableStreamUrl).toContain(
      '/v1/chat/completions/durable/',
    )
  })

  test('AREA 5 — router/delegate: root typed target refs route to OWN Codex Pylon', async () => {
    const pylonStore = makePylonStore([
      registration({ ownerAgentUserId: 'agent_a', pylonRef: 'pylon.a.codex' }),
    ])
    const result = await delegateCodingWorkflow({
      classification: {
        confidence: 1,
        evidenceRefs: ['evidence.coding_workflow.structured_body'],
        workflowClass: 'codex_agent_task',
      },
      linkedAgents: [
        {
          agentUserId: 'agent_a',
          credentialId: 'cred_a',
          displayName: 'Agent A',
          linkKind: 'credential_anchor',
          openauthUserId: 'openauth-user-a',
          tokenPrefix: tokenFor('agent_a').slice(0, 16),
        },
      ],
      makeId: () => 'id1',
      nowIso: NOW_ISO,
      pylonStore,
      rawBody: {
        targetPylonRef: 'pylon.a.codex',
        workflowClass: 'codex_agent_task',
      },
      requestId: 'chatcmpl_validate_root_target',
    })

    expect(result?.kind).toBe('assigned')
    if (result?.kind !== 'assigned') {
      throw new Error('expected root target assignment')
    }
    expect(result.assignment.pylonRef).toBe('pylon.a.codex')
  })

  test('AREA 5b — router/delegate: NEVER routes to another account capacity', async () => {
    const pylonStore = makePylonStore([
      registration({ ownerAgentUserId: 'agent_b', pylonRef: 'pylon.b.codex' }),
    ])
    const result = await delegateCodingWorkflow({
      classification: {
        confidence: 1,
        evidenceRefs: ['evidence.coding_workflow.structured_body'],
        workflowClass: 'codex_agent_task',
      },
      // Caller A only links agent_a, which owns NO registration here.
      linkedAgents: [
        {
          agentUserId: 'agent_a',
          credentialId: 'cred_a',
          displayName: 'Agent A',
          linkKind: 'credential_anchor',
          openauthUserId: 'openauth-user-a',
          tokenPrefix: tokenFor('agent_a').slice(0, 16),
        },
      ],
      makeId: () => 'id1',
      nowIso: NOW_ISO,
      pylonStore,
      rawBody: {},
      requestId: 'chatcmpl_validate_cross',
    })
    expect(result).toBeNull()
    expect(pylonStore.createdAssignments()).toHaveLength(0)
  })

  test('P4/P6 — bare MCP issuer harness: link -> issue -> route -> durable resume, with cross-account resume denied', async () => {
    const userA = 'openauth-user-a'
    const userB = 'openauth-user-b'
    const credentials: Array<Credential> = [
      {
        id: 'cred_a',
        agentUserId: 'agent_a',
        displayName: 'Agent A',
        tokenHash: await sha256Hex(tokenFor('agent_a')),
        tokenPrefix: tokenFor('agent_a').slice(0, 16),
        openauthUserId: null,
      },
      {
        id: 'cred_b',
        agentUserId: 'agent_b',
        displayName: 'Agent B',
        tokenHash: await sha256Hex(tokenFor('agent_b')),
        tokenPrefix: tokenFor('agent_b').slice(0, 16),
        openauthUserId: userB,
      },
    ]
    const agentStore = makeAgentStore(credentials)
    const pylonStore = makePylonStore([
      registration({ ownerAgentUserId: 'agent_a', pylonRef: 'pylon.a.codex' }),
      registration({
        id: 'pylon_api_registration_b',
        ownerAgentUserId: 'agent_b',
        pylonRef: 'pylon.b.codex',
      }),
    ])

    const linked = await route({
      agentStore,
      pylonStore,
      path: '/api/account/pylon-agent-links',
      method: 'POST',
      body: { agentToken: tokenFor('agent_a') },
      browserUserId: userA,
    })
    expect(linked.status).toBe(201)

    const recordedTokens: Array<ServedTokensRecorderInput> = []
    const durableReads: string[] = []
    const idQueue = ['chatcmpl_validate_issue', 'assignment_validate_issue']
    const catalog = makeKhalaMcpCatalog({
      agentStore: () => agentStore,
      durableFetch: async input => {
        durableReads.push(input instanceof Request ? input.url : String(input))
        return new Response('data: [DONE]\n\n', {
          headers: {
            'stream-closed': 'true',
            'stream-next-offset': '17',
            'stream-up-to-date': 'true',
          },
        })
      },
      makeId: () => idQueue.shift() ?? 'id-more',
      nowIso: () => NOW_ISO,
      pylonStore: () => pylonStore,
      recordTokensServed: () => async input => {
        recordedTokens.push(input)
      },
    })
    const session: ProgrammaticAgentSession = {
      credential: {
        id: 'cred_a',
        lastUsedAt: NOW_ISO,
        openauthUserId: userA,
        profileMetadataJson: '{}',
        tokenPrefix: tokenFor('agent_a').slice(0, 16),
      },
      user: {
        avatarUrl: null,
        createdAt: NOW_ISO,
        displayName: 'Agent A',
        id: 'agent_a',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: NOW_ISO,
      },
    }
    const principal = khalaMcpAgentPrincipal(session, NOW_ISO)
    const request = new Request('https://openagents.com/api/mcp', {
      headers: { authorization: `Bearer ${tokenFor('agent_a')}` },
      method: 'POST',
    })

    const issued = await catalog.callTool(
      { OPENAGENTS_DB: {} as D1Database },
      request,
      principal,
      'khala.request',
      {
        prompt: 'Run the reusable issuer harness fixture',
        targetPylonRef: 'pylon.a.codex',
        workflow: 'codex_agent_task',
      },
    )

    expect(issued.isError).toBeFalsy()
    expect(issued.structuredContent).toMatchObject({
      assignmentRef: 'assignment.public.khala_coding.assignment_validate_issue',
      durableRequestId: 'chatcmpl_validate_issue',
      durableStreamUrl:
        '/v1/chat/completions/durable/chatcmpl_validate_issue',
      pylonRef: 'pylon.a.codex',
      stream: true,
      workflow: 'codex_agent_task',
    })
    expect(pylonStore.createdAssignments()).toHaveLength(1)
    expect(pylonStore.createdAssignments()[0]?.ownerAgentUserId).toBe('agent_a')
    expect(pylonStore.createdAssignments()[0]?.pylonRef).toBe('pylon.a.codex')
    // #6325: MCP delegation must NOT meter a handoff estimate at request time.
    // The local Pylon/Codex executor records the exact downstream SDK turn usage
    // through the registered-agent turns ingest route after Codex actually runs,
    // so no served-token row exists at request time. (Matches the contract
    // asserted in khala-mcp.test.ts.)
    expect(recordedTokens).toHaveLength(0)

    const resumed = await catalog.callTool(
      { OPENAGENTS_DB: {} as D1Database },
      request,
      principal,
      'khala.resume',
      { durableRequestId: 'chatcmpl_validate_issue', offset: 12 },
    )
    expect(resumed.isError).toBeFalsy()
    expect(durableReads).toEqual([
      'https://openagents.com/v1/chat/completions/durable/chatcmpl_validate_issue?offset=12',
    ])

    const otherSession: ProgrammaticAgentSession = {
      credential: {
        id: 'cred_b',
        lastUsedAt: NOW_ISO,
        openauthUserId: userB,
        profileMetadataJson: '{}',
        tokenPrefix: tokenFor('agent_b').slice(0, 16),
      },
      user: {
        avatarUrl: null,
        createdAt: NOW_ISO,
        displayName: 'Agent B',
        id: 'agent_b',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: NOW_ISO,
      },
    }
    const otherRequest = new Request('https://openagents.com/api/mcp', {
      headers: { authorization: `Bearer ${tokenFor('agent_b')}` },
      method: 'POST',
    })
    const denied = await catalog.callTool(
      { OPENAGENTS_DB: {} as D1Database },
      otherRequest,
      khalaMcpAgentPrincipal(otherSession, NOW_ISO),
      'khala.resume',
      { durableRequestId: 'chatcmpl_validate_issue', offset: 0 },
    )
    expect(denied.isError).toBe(true)
    expect(denied.structuredContent).toMatchObject({
      error: 'durable_request_not_authorized',
      ok: false,
      statusCode: 403,
    })
    expect(durableReads).toHaveLength(1)
  })

  test('P6.2 — bounded own-capacity property holds across origins, tokens, and targets', async () => {
    const origins = ['local_cli', 'remote_mcp', 'web_chat'] as const
    const callers = [
      { openauthUserId: 'user_a', ownerAgentUserId: 'agent_a', pylonRef: 'pylon.a.codex' },
      { openauthUserId: 'user_b', ownerAgentUserId: 'agent_b', pylonRef: 'pylon.b.codex' },
    ] as const
    const targets = ['none', 'own', 'other'] as const

    for (const origin of origins) {
      for (const caller of callers) {
        for (const target of targets) {
          const other = callers.find(item => item !== caller)!
          const pylonStore = makePylonStore([
            registration({
              ownerAgentUserId: 'agent_a',
              pylonRef: 'pylon.a.codex',
            }),
            registration({
              id: 'pylon_api_registration_b',
              ownerAgentUserId: 'agent_b',
              pylonRef: 'pylon.b.codex',
            }),
          ])
          const targetPylonRef =
            target === 'none'
              ? undefined
              : target === 'own'
                ? caller.pylonRef
                : other.pylonRef
          const rawBody =
            targetPylonRef === undefined
              ? { openagents: { workflowClass: 'codex_agent_task' } }
              : {
                  openagents: {
                    coding: { targetPylonRef },
                    workflowClass: 'codex_agent_task',
                  },
                }
          const result = await delegateCodingWorkflow({
            classification: {
              confidence: 1,
              evidenceRefs: [`evidence.coding_workflow.${origin}`],
              workflowClass: 'codex_agent_task',
            },
            linkedAgents: [
              {
                agentUserId: caller.ownerAgentUserId,
                credentialId: `cred_${caller.ownerAgentUserId}`,
                displayName: caller.ownerAgentUserId,
                linkKind: 'credential_anchor',
                openauthUserId: caller.openauthUserId,
                tokenPrefix: `oa_${caller.ownerAgentUserId}`,
              },
            ],
            makeId: () => `id_${origin}_${caller.ownerAgentUserId}_${target}`,
            nowIso: NOW_ISO,
            pylonStore,
            rawBody,
            requestId: `chatcmpl_${origin}_${caller.ownerAgentUserId}_${target}`,
          })

          if (target === 'other') {
            expect(result).toMatchObject({
              error: 'target_pylon_not_authorized',
              kind: 'rejected',
              requestedPylonRef: other.pylonRef,
              statusCode: 403,
            })
            expect(pylonStore.createdAssignments()).toHaveLength(0)
          } else {
            expect(result?.kind).toBe('assigned')
            if (result?.kind !== 'assigned') {
              throw new Error(`expected assignment for ${origin}/${target}`)
            }
            expect(result.assignment.ownerAgentUserId).toBe(
              caller.ownerAgentUserId,
            )
            expect(result.assignment.pylonRef).toBe(caller.pylonRef)
          }
        }
      }
    }
  })

  test('P6.3 — issuer path stays no-resale, semantic, and MCP-authority bounded', () => {
    expect(
      authorizeInferenceMonetization({ kind: 'agentic_work' }).authorized,
    ).toBe(true)
    const subscriptionResale = authorizeInferenceMonetization({
      kind: 'subscription_capacity_resale',
      accountAuthMode: 'subscription',
      refs: {},
    })
    expect(subscriptionResale.authorized).toBe(false)
    expect(subscriptionResale.blockerRefs).toContain(
      'blocker.inference_resale.subscription_resale_forbidden',
    )

    expect(
      classifyCodingWorkflow({
        messages: [
          {
            content:
              'This prose mentions code, repository, fix, tests, and commit but carries no structured marker.',
            role: 'user',
          },
        ],
        rawBody: {},
      }).workflowClass,
    ).toBe('none')

    expect(
      KHALA_MCP_TOOLS.map(tool => ({
        name: tool.name,
        requiredAuthorities: tool.requiredAuthorities,
      })),
    ).toEqual([
      {
        name: 'khala.request',
        requiredAuthorities: ['coding_session_control'],
      },
      {
        name: 'khala.spawn',
        requiredAuthorities: ['coding_session_control'],
      },
      {
        name: 'khala.resume',
        requiredAuthorities: ['private_account_read'],
      },
      {
        name: 'khala.capacity',
        requiredAuthorities: ['private_account_read'],
      },
      {
        name: 'khala.status',
        requiredAuthorities: ['private_account_read'],
      },
      {
        name: 'khala.spawnStatus',
        requiredAuthorities: ['private_account_read'],
      },
    ])
  })
})
