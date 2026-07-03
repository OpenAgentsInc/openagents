import { decodeAgentDefinition, type AgentDefinition } from '@openagentsinc/agent-runtime-schema'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentCredentialRecord,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  type AgentReissueSelector,
  type AgentReissueTarget,
  sha256Hex,
} from './agent-registration'
import {
  type AgentDefinitionStore,
  handleAgentDefinitionsApi,
} from './agent-definition-routes'

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly touchedCredentialIds: Array<string> = []

  constructor(
    private readonly lookupsByTokenHash: ReadonlyMap<string, AgentCredentialLookup>,
  ) {}

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(this.lookupsByTokenHash.get(tokenHash))
  }

  touchAgentCredential(credentialId: string, _lastUsedAt: string): Promise<void> {
    this.touchedCredentialIds.push(credentialId)

    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }

  findAgentForReissue(
    _selector: AgentReissueSelector,
  ): Promise<AgentReissueTarget | undefined> {
    return Promise.resolve(undefined)
  }

  addAgentCredential(_record: AgentCredentialRecord): Promise<void> {
    return Promise.resolve()
  }
}

class MemoryAgentDefinitionStore implements AgentDefinitionStore {
  readonly rows: Array<{
    readonly ownerAgentUserId: string
    readonly definition: AgentDefinition
  }> = []

  createDefinition(
    ownerAgentUserId: string,
    definition: AgentDefinition,
  ): Promise<void> {
    const conflict = this.rows.some(row =>
      row.definition.id === definition.id ||
      (
        row.ownerAgentUserId === ownerAgentUserId &&
        row.definition.slug === definition.slug
      ),
    )

    if (conflict) {
      return Promise.reject(new Error('UNIQUE constraint failed'))
    }

    this.rows.push({ ownerAgentUserId, definition })

    return Promise.resolve()
  }

  listDefinitions(
    ownerAgentUserId: string,
    limit: number,
  ): Promise<ReadonlyArray<AgentDefinition>> {
    return Promise.resolve(
      this.rows
        .filter(row => row.ownerAgentUserId === ownerAgentUserId)
        .map(row => row.definition)
        .slice(0, limit),
    )
  }

  readDefinition(
    ownerAgentUserId: string,
    definitionId: string,
  ): Promise<AgentDefinition | undefined> {
    return Promise.resolve(
      this.rows.find(row =>
        row.ownerAgentUserId === ownerAgentUserId &&
        row.definition.id === definitionId,
      )?.definition,
    )
  }

  updateDefinition(
    ownerAgentUserId: string,
    definition: AgentDefinition,
  ): Promise<boolean> {
    const index = this.rows.findIndex(row =>
      row.ownerAgentUserId === ownerAgentUserId &&
      row.definition.id === definition.id,
    )

    if (index === -1) {
      return Promise.resolve(false)
    }

    this.rows[index] = { ownerAgentUserId, definition }

    return Promise.resolve(true)
  }
}

const lookup = (input: {
  readonly credentialId: string
  readonly tokenPrefix: string
  readonly userId: string
}): AgentCredentialLookup => ({
  user: {
    id: input.userId,
    kind: 'agent',
    displayName: `Agent ${input.userId}`,
    primaryEmail: null,
    avatarUrl: null,
    status: 'active',
    createdAt: '2026-07-03T00:00:00.000Z',
    updatedAt: '2026-07-03T00:00:00.000Z',
  },
  credentialId: input.credentialId,
  openauthUserId: null,
  profileMetadataJson: '{}',
  tokenPrefix: input.tokenPrefix,
})

const makeStores = async () => {
  const ownerToken = 'oa_agent_owner_route_test'
  const otherToken = 'oa_agent_other_route_test'
  const agentStore = new MemoryAgentRegistrationStore(
    new Map([
      [
        await sha256Hex(ownerToken),
        lookup({
          credentialId: 'credential_owner',
          tokenPrefix: 'oa_agent_owner',
          userId: 'agent_user_owner',
        }),
      ],
      [
        await sha256Hex(otherToken),
        lookup({
          credentialId: 'credential_other',
          tokenPrefix: 'oa_agent_other',
          userId: 'agent_user_other',
        }),
      ],
    ]),
  )

  return {
    agentStore,
    definitionStore: new MemoryAgentDefinitionStore(),
    otherToken,
    ownerToken,
  }
}

const definitionBody = (name = 'Daily Definition') => ({
  name,
  goal: 'Review a bounded queue and produce one public-safe receipt.',
  harness: {
    kind: 'codex',
    modelHint: 'openagents/pylon-codex',
  },
  toolset: {
    allow: ['tool.openagents.crm.read', 'tool.openagents.receipt.write'],
    deny: ['tool.openagents.payment.*'],
    ask: ['tool.openagents.email.draft'],
    networkPolicy: 'owner_scoped',
    secretPolicy: 'owner_scoped_refs_only',
  },
  triggers: [
    {
      kind: 'manual',
      triggerRef: 'trigger.public.route_test.manual',
    },
  ],
  lane: 'own_pylon',
  budget: {
    maxRunSeconds: 900,
    maxRunsPerDay: 3,
    maxCreditsPerDay: 0,
  },
  escalation: {
    channel: 'operator',
    askPolicy: {
      policyRef: 'policy.public.agent_definition.operator_required.v1',
      mode: 'operator_required',
    },
  },
  sourceRefs: ['issue.public.github.OpenAgentsInc.openagents.8188'],
})

const jsonRequest = (
  method: string,
  token: string | undefined,
  body?: unknown,
  path = '/v1/agent-definitions',
) =>
  new Request(`https://openagents.com${path}`, {
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    headers: {
      ...(body === undefined ? {} : { 'content-type': 'application/json' }),
      ...(token === undefined ? {} : { authorization: `Bearer ${token}` }),
    },
    method,
  })

describe('agent definition routes', () => {
  test('requires a registered agent bearer token', async () => {
    const { agentStore, definitionStore } = await makeStores()
    const response = await handleAgentDefinitionsApi(
      jsonRequest('GET', undefined),
      { agentStore, definitionStore },
    )

    expect(response.status).toBe(401)
  })

  test('creates and round-trips openagents.agent_definition.v1 records', async () => {
    const { agentStore, definitionStore, ownerToken } = await makeStores()
    const response = await handleAgentDefinitionsApi(
      jsonRequest('POST', ownerToken, definitionBody()),
      {
        agentStore,
        definitionStore,
        makeId: () => 'agent_definition.route_test.owner',
        nowIso: () => '2026-07-03T00:00:00.000Z',
      },
    )
    const body = (await response.json()) as { definition: AgentDefinition }
    const decoded = decodeAgentDefinition(body.definition)

    expect(response.status).toBe(201)
    expect(decoded).toMatchObject({
      schema: 'openagents.agent_definition.v1',
      id: 'agent_definition.route_test.owner',
      ownerRef: 'agent:agent_user_owner',
      slug: 'daily-definition',
      harness: { kind: 'codex' },
      lane: 'own_pylon',
    })
    expect(definitionStore.rows).toHaveLength(1)
  })

  test('lists only definitions owned by the authenticated agent', async () => {
    const { agentStore, definitionStore, otherToken, ownerToken } = await makeStores()
    await handleAgentDefinitionsApi(
      jsonRequest('POST', ownerToken, definitionBody('Owner Definition')),
      {
        agentStore,
        definitionStore,
        makeId: () => 'agent_definition.route_test.owner',
        nowIso: () => '2026-07-03T00:00:00.000Z',
      },
    )
    await handleAgentDefinitionsApi(
      jsonRequest('POST', otherToken, definitionBody('Other Definition')),
      {
        agentStore,
        definitionStore,
        makeId: () => 'agent_definition.route_test.other',
        nowIso: () => '2026-07-03T00:00:00.000Z',
      },
    )

    const response = await handleAgentDefinitionsApi(
      jsonRequest('GET', ownerToken),
      { agentStore, definitionStore },
    )
    const body = (await response.json()) as {
      definitions: ReadonlyArray<AgentDefinition>
    }

    expect(response.status).toBe(200)
    expect(body.definitions.map(definition => definition.id)).toEqual([
      'agent_definition.route_test.owner',
    ])
  })

  test('reads and patches definitions only inside the owner scope', async () => {
    const { agentStore, definitionStore, otherToken, ownerToken } = await makeStores()
    await handleAgentDefinitionsApi(
      jsonRequest('POST', ownerToken, definitionBody()),
      {
        agentStore,
        definitionStore,
        makeId: () => 'agent_definition.route_test.owner',
        nowIso: () => '2026-07-03T00:00:00.000Z',
      },
    )

    const forbiddenRead = await handleAgentDefinitionsApi(
      jsonRequest(
        'GET',
        otherToken,
        undefined,
        '/v1/agent-definitions?id=agent_definition.route_test.owner',
      ),
      { agentStore, definitionStore },
    )
    const patchResponse = await handleAgentDefinitionsApi(
      jsonRequest('PATCH', ownerToken, {
        id: 'agent_definition.route_test.owner',
        goal: 'Produce one reviewed background-agent receipt.',
        ownerRef: 'agent:agent_user_other',
      }),
      {
        agentStore,
        definitionStore,
        nowIso: () => '2026-07-03T00:01:00.000Z',
      },
    )
    const patchBody = (await patchResponse.json()) as {
      definition: AgentDefinition
    }

    expect(forbiddenRead.status).toBe(404)
    expect(patchResponse.status).toBe(200)
    expect(patchBody.definition).toMatchObject({
      id: 'agent_definition.route_test.owner',
      ownerRef: 'agent:agent_user_owner',
      goal: 'Produce one reviewed background-agent receipt.',
      updatedAt: '2026-07-03T00:01:00.000Z',
    })
  })
})
