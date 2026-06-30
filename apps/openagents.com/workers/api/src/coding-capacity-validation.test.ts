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

// ----------------------------------------------------------------------------
// #6354 — per-account Codex capacity: multiple linked accounts on ONE owner
// Pylon each dispatch their own concurrent assignments. A saturated account
// must NOT block another account's request on the same Pylon.
// ----------------------------------------------------------------------------

const ACCOUNT_A_HASH = 'account.pylon.codex.aaaaaaaaaaaa'
const ACCOUNT_A_KEY = 'aaaaaaaaaaaa'
const ACCOUNT_B_HASH = 'account.pylon.codex.bbbbbbbbbbbb'
const ACCOUNT_B_KEY = 'bbbbbbbbbbbb'

const perAccountRegistration = (
  overrides: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord =>
  registration({
    ownerAgentUserId: 'agent_a',
    pylonRef: 'pylon.a.codex',
    // Heartbeat advertises 8 slots per account (16 pooled), busy reported 0
    // (heartbeat lags the active leases pre-seeded into the gate below).
    latestCapacityRefs: [
      `capacity.coding.codex.account.${ACCOUNT_A_KEY}.ready=8`,
      `capacity.coding.codex.account.${ACCOUNT_A_KEY}.available=8`,
      `capacity.coding.codex.account.${ACCOUNT_B_KEY}.ready=8`,
      `capacity.coding.codex.account.${ACCOUNT_B_KEY}.available=8`,
    ],
    latestLoadRefs: [
      `load.coding.codex.account.${ACCOUNT_A_KEY}.busy=0`,
      `load.coding.codex.account.${ACCOUNT_A_KEY}.queued=0`,
      `load.coding.codex.account.${ACCOUNT_B_KEY}.busy=0`,
      `load.coding.codex.account.${ACCOUNT_B_KEY}.queued=0`,
    ],
    ...overrides,
  })

const activeCodexLease = (
  pylonRef: string,
  index: number,
  accountRefHash: string,
): PylonApiAssignmentRecord => ({
  acceptanceCriteriaRefs: [],
  acceptedWorkRefs: [],
  artifactRefs: [],
  assignmentRef: `assignment.public.khala_coding.seed_${accountRefHash}_${index}`,
  closeoutRefs: [],
  codingAssignment: {
    codex: { accountRefHash, agentKind: 'codex_sdk' },
    requiredCapabilityRefs: ['capability.pylon.local_codex'],
  },
  createdAt: NOW_ISO,
  id: `pylon_api_assignment_seed_${accountRefHash}_${index}`,
  idempotencyKeyHash: `seed:${accountRefHash}:${index}`,
  jobKind: 'codex_agent_task',
  // Far-future lease so the gate counts it as an active duplicate.
  leaseExpiresAt: '2026-06-25T13:00:00.000Z',
  ownerAgentUserId: 'agent_a',
  proofRefs: [],
  publicProjectionJson: '{}',
  pylonRef,
  rejectionRefs: [],
  resultExpectationRefs: [],
  state: 'running',
  taskRefs: [],
  updatedAt: NOW_ISO,
})

const linkedAgentA: LinkedAgentOwnerRecord = {
  agentUserId: 'agent_a',
  credentialId: 'cred_a',
  displayName: 'Agent A',
  linkKind: 'credential_anchor',
  openauthUserId: 'openauth-user-a',
  tokenPrefix: tokenFor('agent_a').slice(0, 16),
}

const delegateToAccount = (
  pylonStore: PylonApiStore,
  accountRefHash: string | undefined,
  requestId: string,
) =>
  delegateCodingWorkflow({
    classification: {
      confidence: 1,
      evidenceRefs: ['evidence.coding_workflow.structured_body'],
      workflowClass: 'codex_agent_task',
    },
    linkedAgents: [linkedAgentA],
    makeId: () => `id_${requestId}`,
    nowIso: NOW_ISO,
    pylonStore,
    rawBody: {
      openagents: {
        coding: {
          targetPylonRef: 'pylon.a.codex',
          ...(accountRefHash === undefined ? {} : { targetAccountRefHash: accountRefHash }),
        },
        workflowClass: 'codex_agent_task',
      },
    },
    requestId,
  })

describe('#6354 per-account Codex dispatch division-of-labor', () => {
  test('projection: per-account refs project per account and sum to the pooled total', () => {
    const projection = pylonCodingServiceCapacityProjection(perAccountRegistration())
    const codex = projection.find(item => item.service === 'codex')
    expect(codex).toBeDefined()
    // Pooled totals are derived from the account sum (8 + 8).
    expect(codex).toMatchObject({ available: 16, busy: 0, queued: 0, ready: 16 })
    expect(codex?.accounts).toEqual([
      { accountKey: ACCOUNT_A_KEY, available: 8, busy: 0, queued: 0, ready: 8 },
      { accountKey: ACCOUNT_B_KEY, available: 8, busy: 0, queued: 0, ready: 8 },
    ])
  })

  test('projection backward compat: pooled-only heartbeats keep pooled totals and empty accounts', () => {
    const projection = pylonCodingServiceCapacityProjection(
      registration({ pylonRef: 'pylon.pooled' }),
    )
    const codex = projection.find(item => item.service === 'codex')
    expect(codex).toMatchObject({ ready: 2, available: 1, busy: 1, queued: 0 })
    expect(codex?.accounts).toEqual([])
  })

  test('account A saturated by its own active leases does NOT block account B on the same Pylon', async () => {
    const pylonStore = makePylonStore([perAccountRegistration()])
    // Pre-seed 8 active leases pinned to account A — A is fully saturated.
    for (let index = 0; index < 8; index += 1) {
      await pylonStore.createAssignment(
        activeCodexLease('pylon.a.codex', index, ACCOUNT_A_HASH),
      )
    }

    // Account A is refused: 8 active leases >= 8 advertised slots.
    const refusedA = await delegateToAccount(pylonStore, ACCOUNT_A_HASH, 'req_a')
    expect(refusedA?.kind).toBe('rejected')
    if (refusedA?.kind !== 'rejected') {
      throw new Error('expected account A to be refused')
    }
    expect(refusedA.statusCode).toBe(409)
    expect(refusedA.error).toBe('target_pylon_unavailable')

    // Account B is admitted concurrently — its slots are independent of A.
    const admittedB = await delegateToAccount(pylonStore, ACCOUNT_B_HASH, 'req_b')
    expect(admittedB?.kind).toBe('assigned')
    if (admittedB?.kind !== 'assigned') {
      throw new Error('expected account B to be admitted')
    }
    expect(
      (admittedB.assignment.codingAssignment?.codex as { accountRefHash?: string })
        ?.accountRefHash,
    ).toBe(ACCOUNT_B_HASH)
  })

  test('root targetAccountRefHash pins account B even when account A is saturated', async () => {
    const pylonStore = makePylonStore([perAccountRegistration()])
    for (let index = 0; index < 8; index += 1) {
      await pylonStore.createAssignment(
        activeCodexLease('pylon.a.codex', index, ACCOUNT_A_HASH),
      )
    }

    const admitted = await delegateCodingWorkflow({
      classification: {
        confidence: 1,
        evidenceRefs: ['evidence.coding_workflow.structured_body'],
        workflowClass: 'codex_agent_task',
      },
      linkedAgents: [linkedAgentA],
      makeId: () => 'id_req_root_account_b',
      nowIso: NOW_ISO,
      pylonStore,
      rawBody: {
        openagents: { workflowClass: 'codex_agent_task' },
        targetAccountRefHash: ACCOUNT_B_HASH,
        targetPylonRef: 'pylon.a.codex',
      },
      requestId: 'req_root_account_b',
    })

    expect(admitted?.kind).toBe('assigned')
    if (admitted?.kind !== 'assigned') {
      throw new Error('expected root account hash request to be admitted')
    }
    expect(
      (admitted.assignment.codingAssignment?.codex as { accountRefHash?: string })
        ?.accountRefHash,
    ).toBe(ACCOUNT_B_HASH)
  })

  test('per-account busy accounting: only same-account active leases count toward a request', async () => {
    const pylonStore = makePylonStore([perAccountRegistration()])
    // 8 active leases on account A, plus 7 on account B (B has 1 free slot).
    for (let index = 0; index < 8; index += 1) {
      await pylonStore.createAssignment(
        activeCodexLease('pylon.a.codex', index, ACCOUNT_A_HASH),
      )
    }
    for (let index = 0; index < 7; index += 1) {
      await pylonStore.createAssignment(
        activeCodexLease('pylon.a.codex', index, ACCOUNT_B_HASH),
      )
    }
    // B still has 1 of 8 slots: A's 8 leases never count against B.
    const admittedB = await delegateToAccount(pylonStore, ACCOUNT_B_HASH, 'req_b1')
    expect(admittedB?.kind).toBe('assigned')
  })

  test('#6386: unpinned requests are admitted onto another advertised account when the first is full', async () => {
    const pylonStore = makePylonStore([perAccountRegistration()])
    // Account A is full. The caller did not pin an account, so the Worker must
    // try advertised account slots instead of creating an unkeyed Pylon-level
    // lease that would be blocked by A's active work.
    for (let index = 0; index < 8; index += 1) {
      await pylonStore.createAssignment(
        activeCodexLease('pylon.a.codex', index, ACCOUNT_A_HASH),
      )
    }

    const admitted = await delegateToAccount(
      pylonStore,
      undefined,
      'req_unpinned_b',
    )
    expect(admitted?.kind).toBe('assigned')
    if (admitted?.kind !== 'assigned') {
      throw new Error('expected unpinned request to be admitted')
    }
    expect(
      (admitted.assignment.codingAssignment?.codex as { accountRefHash?: string })
        ?.accountRefHash,
    ).toBe(ACCOUNT_B_HASH)
  })

  test('pooled backward compat: untagged requests gate against the pooled total', async () => {
    const pylonStore = makePylonStore([
      registration({
        ownerAgentUserId: 'agent_a',
        pylonRef: 'pylon.a.codex',
        latestCapacityRefs: [
          'capacity.coding.codex.ready=1',
          'capacity.coding.codex.available=1',
        ],
        latestLoadRefs: ['load.coding.codex.busy=0', 'load.coding.codex.queued=0'],
      }),
    ])
    // One untagged active lease saturates the single pooled slot.
    await pylonStore.createAssignment({
      ...activeCodexLease('pylon.a.codex', 0, ACCOUNT_A_HASH),
      codingAssignment: {
        codex: { agentKind: 'codex_sdk' },
        requiredCapabilityRefs: ['capability.pylon.local_codex'],
      },
    })
    const refused = await delegateToAccount(pylonStore, undefined, 'req_pooled')
    expect(refused?.kind).toBe('rejected')
    if (refused?.kind !== 'rejected') {
      throw new Error('expected pooled refusal')
    }
    expect(refused.statusCode).toBe(409)
  })

  test('rejects a malformed targetAccountRefHash with a typed 400', async () => {
    const pylonStore = makePylonStore([perAccountRegistration()])
    const rejected = await delegateToAccount(
      pylonStore,
      'not-an-account-hash',
      'req_bad',
    )
    expect(rejected?.kind).toBe('rejected')
    if (rejected?.kind !== 'rejected') {
      throw new Error('expected malformed account ref rejection')
    }
    expect(rejected.statusCode).toBe(400)
    expect(rejected.error).toBe('invalid_target_account_ref')
  })
})

// ----------------------------------------------------------------------------
// #6421 — per-account CLAUDE capacity: mirrors the #6354 Codex lane so the
// claude-supervisor can run several distinct Claude accounts on one owner Pylon.
// A saturated Claude account must NOT block another Claude account, and a Codex
// account hash must NOT be admissible on a claude_agent_task (cross-provider).
// ----------------------------------------------------------------------------

const CLAUDE_A_HASH = 'account.pylon.claude_agent.cccccccccccc'
const CLAUDE_A_KEY = 'cccccccccccc'
const CLAUDE_B_HASH = 'account.pylon.claude_agent.dddddddddddd'
const CLAUDE_B_KEY = 'dddddddddddd'

const claudePerAccountRegistration = (): PylonApiRegistrationRecord =>
  registration({
    ownerAgentUserId: 'agent_a',
    pylonRef: 'pylon.a.claude',
    capabilityRefs: ['capability.pylon.local_claude_agent'],
    latestCapacityRefs: [
      `capacity.coding.claude.account.${CLAUDE_A_KEY}.ready=2`,
      `capacity.coding.claude.account.${CLAUDE_A_KEY}.available=2`,
      `capacity.coding.claude.account.${CLAUDE_B_KEY}.ready=2`,
      `capacity.coding.claude.account.${CLAUDE_B_KEY}.available=2`,
    ],
    latestLoadRefs: [
      `load.coding.claude.account.${CLAUDE_A_KEY}.busy=0`,
      `load.coding.claude.account.${CLAUDE_A_KEY}.queued=0`,
      `load.coding.claude.account.${CLAUDE_B_KEY}.busy=0`,
      `load.coding.claude.account.${CLAUDE_B_KEY}.queued=0`,
    ],
  })

const activeClaudeLease = (
  pylonRef: string,
  index: number,
  accountRefHash: string,
): PylonApiAssignmentRecord => ({
  ...activeCodexLease(pylonRef, index, accountRefHash),
  assignmentRef: `assignment.public.khala_coding.claude_${accountRefHash}_${index}`,
  codingAssignment: {
    claudeAgent: { accountRefHash, agentKind: 'claude_agent_sdk' },
    requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
  },
  id: `pylon_api_assignment_claude_${accountRefHash}_${index}`,
  idempotencyKeyHash: `claude_seed:${accountRefHash}:${index}`,
  jobKind: 'claude_agent_task',
})

const delegateToClaudeAccount = (
  pylonStore: PylonApiStore,
  accountRefHash: string | undefined,
  requestId: string,
) =>
  delegateCodingWorkflow({
    classification: {
      confidence: 1,
      evidenceRefs: ['evidence.coding_workflow.structured_body'],
      workflowClass: 'claude_agent_task',
    },
    linkedAgents: [linkedAgentA],
    makeId: () => `id_${requestId}`,
    nowIso: NOW_ISO,
    pylonStore,
    rawBody: {
      openagents: {
        coding: {
          targetPylonRef: 'pylon.a.claude',
          ...(accountRefHash === undefined ? {} : { targetAccountRefHash: accountRefHash }),
        },
        workflowClass: 'claude_agent_task',
      },
    },
    requestId,
  })

describe('#6421 per-account Claude dispatch division-of-labor', () => {
  test('projection: per-Claude-account refs project per account and sum to pooled', () => {
    const projection = pylonCodingServiceCapacityProjection(
      claudePerAccountRegistration(),
    )
    const claude = projection.find(item => item.service === 'claude')
    expect(claude).toBeDefined()
    expect(claude).toMatchObject({ available: 4, busy: 0, queued: 0, ready: 4 })
    expect(claude?.accounts).toEqual([
      { accountKey: CLAUDE_A_KEY, available: 2, busy: 0, queued: 0, ready: 2 },
      { accountKey: CLAUDE_B_KEY, available: 2, busy: 0, queued: 0, ready: 2 },
    ])
  })

  test('Claude account A saturated by its own leases does NOT block Claude account B', async () => {
    const pylonStore = makePylonStore([claudePerAccountRegistration()])
    for (let index = 0; index < 2; index += 1) {
      await pylonStore.createAssignment(
        activeClaudeLease('pylon.a.claude', index, CLAUDE_A_HASH),
      )
    }

    const refusedA = await delegateToClaudeAccount(pylonStore, CLAUDE_A_HASH, 'c_req_a')
    expect(refusedA?.kind).toBe('rejected')
    if (refusedA?.kind !== 'rejected') {
      throw new Error('expected Claude account A to be refused')
    }
    expect(refusedA.statusCode).toBe(409)
    expect(refusedA.error).toBe('target_pylon_unavailable')

    const admittedB = await delegateToClaudeAccount(pylonStore, CLAUDE_B_HASH, 'c_req_b')
    expect(admittedB?.kind).toBe('assigned')
    if (admittedB?.kind !== 'assigned') {
      throw new Error('expected Claude account B to be admitted')
    }
    expect(admittedB.assignment.jobKind).toBe('claude_agent_task')
    expect(
      (admittedB.assignment.codingAssignment?.claudeAgent as {
        accountRefHash?: string
      })?.accountRefHash,
    ).toBe(CLAUDE_B_HASH)
  })

  test('per-account capacity admission: account A advertising available=0 is refused while B is admitted', async () => {
    // Heartbeat directly advertises account A as saturated (available=0) and B
    // as free (available=2). This exercises the per-account capacity ADMISSION
    // path (pylonCodingServiceAccountCapacity), not just the dispatch gate.
    const pylonStore = makePylonStore([
      registration({
        ownerAgentUserId: 'agent_a',
        pylonRef: 'pylon.a.claude',
        capabilityRefs: ['capability.pylon.local_claude_agent'],
        latestCapacityRefs: [
          `capacity.coding.claude.account.${CLAUDE_A_KEY}.ready=2`,
          `capacity.coding.claude.account.${CLAUDE_A_KEY}.available=0`,
          `capacity.coding.claude.account.${CLAUDE_B_KEY}.ready=2`,
          `capacity.coding.claude.account.${CLAUDE_B_KEY}.available=2`,
        ],
        latestLoadRefs: [
          `load.coding.claude.account.${CLAUDE_A_KEY}.busy=2`,
          `load.coding.claude.account.${CLAUDE_A_KEY}.queued=0`,
          `load.coding.claude.account.${CLAUDE_B_KEY}.busy=0`,
          `load.coding.claude.account.${CLAUDE_B_KEY}.queued=0`,
        ],
      }),
    ])
    const refusedA = await delegateToClaudeAccount(pylonStore, CLAUDE_A_HASH, 'c_cap_a')
    expect(refusedA?.kind).toBe('rejected')
    if (refusedA?.kind !== 'rejected') {
      throw new Error('expected account A capacity refusal')
    }
    expect(refusedA.statusCode).toBe(409)
    expect(refusedA.evidenceRefs).toContain(
      'evidence.khala_coding.target_pylon_ref.unavailable.no_available_claude_capacity',
    )

    const admittedB = await delegateToClaudeAccount(pylonStore, CLAUDE_B_HASH, 'c_cap_b')
    expect(admittedB?.kind).toBe('assigned')
  })

  test('a Codex account hash is rejected on a claude_agent_task (cross-provider guard)', async () => {
    const pylonStore = makePylonStore([claudePerAccountRegistration()])
    const rejected = await delegateToClaudeAccount(
      pylonStore,
      ACCOUNT_A_HASH, // account.pylon.codex.* — wrong provider for the Claude lane
      'c_req_xprov',
    )
    expect(rejected?.kind).toBe('rejected')
    if (rejected?.kind !== 'rejected') {
      throw new Error('expected cross-provider account ref rejection')
    }
    expect(rejected.statusCode).toBe(400)
    expect(rejected.error).toBe('invalid_target_account_ref')
  })
})
