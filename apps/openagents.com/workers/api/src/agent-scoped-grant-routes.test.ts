import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  type AgentScopedGrantReceiptRecord,
  type AgentScopedGrantStore,
  type OwnerAgentRecord,
  type OwnerClaimRecord,
  makeAgentScopedGrantRoutes,
} from './agent-scoped-grant-routes'
import { authenticateCustomerOrderAgentRequest } from './customer-order-agent-auth'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    login: string
    name: string
    userId: string
  }>
}>

class MemoryAgentScopedGrantStore implements AgentScopedGrantStore {
  readonly agents = new Map<string, OwnerAgentRecord>()
  readonly claims: ReadonlyArray<OwnerClaimRecord> = []
  readonly receipts = new Map<string, AgentScopedGrantReceiptRecord>()

  createReceipt(
    receipt: AgentScopedGrantReceiptRecord,
  ): Promise<AgentScopedGrantReceiptRecord> {
    this.receipts.set(receipt.idempotencyKeyHash, receipt)

    return Promise.resolve(receipt)
  }

  listAgentClaimsForOwner(): Promise<ReadonlyArray<OwnerClaimRecord>> {
    return Promise.resolve(this.claims)
  }

  listAgents(): Promise<ReadonlyArray<OwnerAgentRecord>> {
    return Promise.resolve(Array.from(this.agents.values()))
  }

  listReceiptsForOwner(
    ownerUserId: string,
    limit: number,
  ): Promise<ReadonlyArray<AgentScopedGrantReceiptRecord>> {
    return Promise.resolve(
      Array.from(this.receipts.values())
        .filter(receipt => receipt.ownerUserId === ownerUserId)
        .slice(0, limit),
    )
  }

  readAgent(agentUserId: string): Promise<OwnerAgentRecord | undefined> {
    return Promise.resolve(this.agents.get(agentUserId))
  }

  readReceiptByIdempotencyKeyHash(
    idempotencyKeyHash: string,
  ): Promise<AgentScopedGrantReceiptRecord | undefined> {
    return Promise.resolve(this.receipts.get(idempotencyKeyHash))
  }

  updateAgentMetadata(
    agentUserId: string,
    metadataJson: string,
    updatedAt: string,
  ): Promise<void> {
    const agent = this.agents.get(agentUserId)

    if (agent !== undefined) {
      this.agents.set(agentUserId, {
        ...agent,
        profileMetadataJson: metadataJson,
        updatedAt,
      })
    }

    return Promise.resolve()
  }
}

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  constructor(private readonly grantStore: MemoryAgentScopedGrantStore) {}

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  async findAgentByTokenHash(
    tokenHash: string,
  ): Promise<AgentCredentialLookup | undefined> {
    if (tokenHash !== (await sha256Hex('oa_agent_test_token'))) {
      return undefined
    }

    const agent = this.grantStore.agents.get('agent-1')

    if (agent === undefined) {
      return undefined
    }

    return {
      credentialId: agent.credentialId ?? 'credential-1',
      profileMetadataJson: agent.profileMetadataJson,
      tokenPrefix: agent.tokenPrefix ?? 'oa_agent_test',
      user: {
        avatarUrl: agent.avatarUrl,
        createdAt: agent.createdAt,
        displayName: agent.displayName,
        id: agent.userId,
        kind: 'agent',
        primaryEmail: agent.primaryEmail,
        status: 'active',
        updatedAt: agent.updatedAt,
      },
    }
  }

  touchAgentCredential(): Promise<void> {
    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }
}

const ownerSession: TestSession = {
  user: {
    email: 'owner@example.com',
    login: 'owner',
    name: 'Owner Example',
    userId: 'owner-1',
  },
}

const otherOwnerSession: TestSession = {
  user: {
    email: 'other@example.com',
    login: 'other',
    name: 'Other Owner',
    userId: 'owner-2',
  },
}

const makeStore = () => {
  const store = new MemoryAgentScopedGrantStore()
  store.agents.set('agent-1', {
    avatarUrl: null,
    createdAt: '2026-06-06T00:00:00.000Z',
    credentialExpiresAt: null,
    credentialId: 'credential-1',
    credentialLastUsedAt: null,
    credentialStatus: 'active',
    displayName: 'Grant Test Agent',
    primaryEmail: 'agent@example.com',
    profileMetadataJson: '{}',
    slug: 'grant-test-agent',
    tokenPrefix: 'oa_agent_test',
    updatedAt: '2026-06-06T00:00:00.000Z',
    userId: 'agent-1',
  })

  return store
}

const makeUuidFactory = () => {
  const values = ['grant-1', 'receipt-1', 'receipt-2', 'receipt-3']

  return () => values.shift() ?? 'fallback'
}

const makeRoutes = (
  store: MemoryAgentScopedGrantStore,
  session: TestSession | undefined = ownerSession,
  adminToken = false,
) =>
  makeAgentScopedGrantRoutes<TestSession, { store: MemoryAgentScopedGrantStore }>({
    requireAdminApiToken: () => Promise.resolve(adminToken),
    appOrigin: () => 'https://openagents.com',
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-test-session-refreshed', 'true')

      return response
    },
    makeStore: env => env.store,
    makeUuid: makeUuidFactory(),
    nowIso: () => '2026-06-06T00:00:00.000Z',
    requireBrowserSession: () => Promise.resolve(session),
  })

const runRoute = async (
  store: MemoryAgentScopedGrantStore,
  request: Request,
  session: TestSession | undefined = ownerSession,
  adminToken = false,
): Promise<Response> => {
  const effect = makeRoutes(store, session, adminToken).routeAgentScopedGrantRequest(
    request,
    { store },
    {} as ExecutionContext,
  )

  if (effect === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(effect)
}

const createGrant = (
  store: MemoryAgentScopedGrantStore,
  input: Readonly<{
    idempotencyKey?: string
    scopes?: ReadonlyArray<string>
    session?: TestSession
  }> = {},
) =>
  runRoute(
    store,
    new Request('https://openagents.com/api/agents/scoped-grants', {
      body: JSON.stringify({
        agentUserId: 'agent-1',
        expiresAt: '2026-06-07T00:00:00.000Z',
        grantKind: 'customer_orders',
        scopes: input.scopes ?? [
          'customer_orders.read',
          'customer_orders.feedback',
        ],
      }),
      headers: {
        'content-type': 'application/json',
        'idempotency-key': input.idempotencyKey ?? 'grant-key-1',
      },
      method: 'POST',
    }),
    input.session ?? ownerSession,
  )

describe('agent scoped grant routes', () => {
  test('lets an operator with the admin token issue an owner-bound grant', async () => {
    const store = makeStore()
    const operatorRequest = (idempotencyKey: string) =>
      new Request('https://openagents.com/api/operator/agents/scoped-grants', {
        body: JSON.stringify({
          agentUserId: 'agent-1',
          grantKind: 'customer_orders',
          ownerUserId: 'owner-1',
          reason: 'operator-issued for live smoke',
          scopes: ['customer_orders.read', 'customer_orders.write'],
        }),
        headers: {
          'content-type': 'application/json',
          'idempotency-key': idempotencyKey,
        },
        method: 'POST',
      })
    const denied = await runRoute(
      store,
      operatorRequest('operator-grant-1'),
      undefined,
      false,
    )
    const created = await runRoute(
      store,
      operatorRequest('operator-grant-1'),
      undefined,
      true,
    )
    const createdBody = (await created.json()) as {
      grant: { ownerUserId: string; scopes: ReadonlyArray<string> }
      receipt: { ownerUserId: string }
    }
    const missingOwner = await runRoute(
      store,
      new Request('https://openagents.com/api/operator/agents/scoped-grants', {
        body: JSON.stringify({
          agentUserId: 'agent-1',
          grantKind: 'customer_orders',
          scopes: ['customer_orders.read'],
        }),
        headers: {
          'content-type': 'application/json',
          'idempotency-key': 'operator-grant-2',
        },
        method: 'POST',
      }),
      undefined,
      true,
    )

    expect(denied.status).toBe(401)
    expect(created.status).toBe(201)
    expect(createdBody.grant).toMatchObject({
      ownerUserId: 'owner-1',
      scopes: ['customer_orders.read', 'customer_orders.write'],
    })
    expect(createdBody.receipt.ownerUserId).toBe('owner-1')
    expect(missingOwner.status).toBe(400)
  })

  test('lets an owner grant customer-order scopes to a registered agent', async () => {
    const store = makeStore()
    const response = await createGrant(store)
    const body = (await response.json()) as {
      grant: { grantId: string; ownerUserId: string; scopes: ReadonlyArray<string> }
      receipt: { scopes: ReadonlyArray<string> }
    }
    const metadata = JSON.parse(
      store.agents.get('agent-1')?.profileMetadataJson ?? '{}',
    ) as { customerOrderGrants?: ReadonlyArray<{ grantId: string }> }

    expect(response.status).toBe(201)
    expect(body.grant).toMatchObject({
      grantId: 'agent_grant_grant-1',
      ownerUserId: 'owner-1',
      scopes: ['customer_orders.feedback', 'customer_orders.read'],
    })
    expect(body.receipt.scopes).toEqual([
      'customer_orders.feedback',
      'customer_orders.read',
    ])
    expect(metadata.customerOrderGrants?.[0]?.grantId).toBe(
      'agent_grant_grant-1',
    )
    expect(JSON.stringify(body)).not.toContain('oa_agent_test_token')
  })

  test('rejects duplicate, expired, and unsupported grants', async () => {
    const store = makeStore()
    const first = await createGrant(store)
    const duplicate = await createGrant(store, { idempotencyKey: 'grant-key-2' })
    const expired = await runRoute(
      store,
      new Request('https://openagents.com/api/agents/scoped-grants', {
        body: JSON.stringify({
          agentUserId: 'agent-1',
          expiresAt: '2026-06-05T00:00:00.000Z',
          grantKind: 'customer_orders',
          scopes: ['customer_orders.read'],
        }),
        headers: {
          'content-type': 'application/json',
          'idempotency-key': 'grant-key-3',
        },
        method: 'POST',
      }),
    )
    const unsupported = await createGrant(store, {
      idempotencyKey: 'grant-key-4',
      scopes: ['customer_orders.admin'],
    })

    expect(first.status).toBe(201)
    expect(duplicate.status).toBe(409)
    expect(expired.status).toBe(400)
    expect(unsupported.status).toBe(400)
  })

  test('prevents wrong-owner revoke and revokes owner grants immediately', async () => {
    const store = makeStore()
    await createGrant(store)
    const wrongOwner = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/scoped-grants/agent_grant_grant-1/revoke',
        {
          headers: { 'idempotency-key': 'revoke-key-1' },
          method: 'POST',
        },
      ),
      otherOwnerSession,
    )
    const beforeRevoke = await Effect.runPromise(
      authenticateCustomerOrderAgentRequest(
        new Request('https://openagents.com/api/customer-orders', {
          headers: { authorization: 'Bearer oa_agent_test_token' },
        }),
        new MemoryAgentRegistrationStore(store),
        {
          nowIso: () => '2026-06-06T00:00:00.000Z',
          ownerUserId: 'owner-1',
          requiredScope: 'customer_orders.read',
        },
      ),
    )
    const revoke = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/scoped-grants/agent_grant_grant-1/revoke',
        {
          headers: { 'idempotency-key': 'revoke-key-2' },
          method: 'POST',
        },
      ),
    )
    const afterRevoke = await Effect.runPromiseExit(
      authenticateCustomerOrderAgentRequest(
        new Request('https://openagents.com/api/customer-orders', {
          headers: { authorization: 'Bearer oa_agent_test_token' },
        }),
        new MemoryAgentRegistrationStore(store),
        {
          nowIso: () => '2026-06-06T00:00:00.000Z',
          ownerUserId: 'owner-1',
          requiredScope: 'customer_orders.read',
        },
      ),
    )

    expect(wrongOwner.status).toBe(403)
    expect(beforeRevoke.ownerUserId).toBe('owner-1')
    expect(revoke.status).toBe(200)
    expect(afterRevoke._tag).toBe('Failure')
  })
})
