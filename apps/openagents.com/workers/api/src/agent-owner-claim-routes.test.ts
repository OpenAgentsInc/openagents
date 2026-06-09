import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentOwnerClaimRecord,
  type AgentOwnerClaimStore,
  makeAgentOwnerClaimRoutes,
} from './agent-owner-claim-routes'
import { type AgentRegistrationRecord, sha256Hex } from './agent-registration'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    login: string
    name: string
    userId: string
  }>
}>

class MemoryAgentOwnerClaimStore implements AgentOwnerClaimStore {
  readonly claims = new Map<string, AgentOwnerClaimRecord>()
  readonly registrations: Array<AgentRegistrationRecord> = []

  approveClaim(input: {
    claimId: string
    credentialExpiresAt: string
    decidedAt: string
    ownerUserId: string
    registration: AgentRegistrationRecord
  }): Promise<AgentOwnerClaimRecord | undefined> {
    const claim = this.claims.get(input.claimId)

    if (claim === undefined || claim.status !== 'pending') {
      return Promise.resolve(claim)
    }

    this.registrations.push(input.registration)
    const approved: AgentOwnerClaimRecord = {
      ...claim,
      agentUserId: input.registration.user.id,
      credentialId: input.registration.credential.id,
      decidedAt: input.decidedAt,
      expiresAt: input.credentialExpiresAt,
      ownerUserId: input.ownerUserId,
      status: 'approved',
      tokenIssuedAt: input.decidedAt,
      tokenPrefix: input.registration.credential.tokenPrefix,
      updatedAt: input.decidedAt,
    }
    this.claims.set(input.claimId, approved)

    return Promise.resolve(approved)
  }

  createClaim(record: AgentOwnerClaimRecord): Promise<void> {
    this.claims.set(record.id, record)

    return Promise.resolve()
  }

  expireClaim(
    claimId: string,
    now: string,
  ): Promise<AgentOwnerClaimRecord | undefined> {
    const claim = this.claims.get(claimId)

    if (
      claim === undefined ||
      claim.status !== 'pending' ||
      claim.expiresAt > now
    ) {
      return Promise.resolve(claim)
    }

    const expired: AgentOwnerClaimRecord = {
      ...claim,
      status: 'expired',
      updatedAt: now,
    }
    this.claims.set(claimId, expired)

    return Promise.resolve(expired)
  }

  readClaimById(claimId: string): Promise<AgentOwnerClaimRecord | undefined> {
    return Promise.resolve(this.claims.get(claimId))
  }

  rejectClaim(input: {
    claimId: string
    decidedAt: string
    ownerUserId: string
    reason: string | null
  }): Promise<AgentOwnerClaimRecord | undefined> {
    const claim = this.claims.get(input.claimId)

    if (claim === undefined || claim.status !== 'pending') {
      return Promise.resolve(claim)
    }

    const rejected: AgentOwnerClaimRecord = {
      ...claim,
      decidedAt: input.decidedAt,
      ownerUserId: input.ownerUserId,
      rejectedReason: input.reason,
      status: 'rejected',
      updatedAt: input.decidedAt,
    }
    this.claims.set(input.claimId, rejected)

    return Promise.resolve(rejected)
  }
}

const makeUuidFactory = (values: ReadonlyArray<string>) => {
  const queue = Array.from(values)

  return (): string => {
    const value = queue.shift()

    if (value === undefined) {
      throw new Error('uuid factory exhausted')
    }

    return value
  }
}

const session: TestSession = {
  user: {
    email: 'owner@example.com',
    login: 'owner',
    name: 'Owner Example',
    userId: 'github:owner-1',
  },
}

const makeRoutes = (
  store: MemoryAgentOwnerClaimStore,
  options: Readonly<{
    claimTtlMs?: number
    makeUuid?: () => string
    nowIso?: () => string
    session?: TestSession | undefined
  }> = {},
) =>
  makeAgentOwnerClaimRoutes<TestSession, { store: MemoryAgentOwnerClaimStore }>(
    {
      appOrigin: () => 'https://openagents.com',
      appendRefreshedSessionCookies: response => {
        response.headers.set('x-test-session-refreshed', 'true')

        return response
      },
      makeStore: env => env.store,
      makeToken: () => 'oa_agent_pending_owner_claim_token',
      makeUuid: options.makeUuid ?? makeUuidFactory(['claim-1']),
      nowIso: options.nowIso ?? (() => '2026-06-06T00:00:00.000Z'),
      requireBrowserSession: () => Promise.resolve(options.session),
      ...(options.claimTtlMs === undefined
        ? {}
        : { claimTtlMs: options.claimTtlMs }),
    },
  )

const runRoute = async (
  store: MemoryAgentOwnerClaimStore,
  request: Request,
  options: Parameters<typeof makeRoutes>[1] = {},
): Promise<Response> => {
  const response = makeRoutes(store, options).routeAgentOwnerClaimRequest(
    request,
    { store },
    {} as ExecutionContext,
  )

  if (response === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(response)
}

const createClaim = (
  store: MemoryAgentOwnerClaimStore,
  options: Parameters<typeof makeRoutes>[1] = {},
): Promise<Response> =>
  runRoute(
    store,
    new Request('https://openagents.com/api/agents/claims', {
      body: JSON.stringify({
        displayName: 'Claimed Agent',
        externalId: 'claimed-agent-local',
        metadata: { purpose: 'owner_claim_test' },
        slug: 'claimed-agent',
      }),
      headers: { 'content-type': 'application/json' },
      method: 'POST',
    }),
    options,
  )

describe('agent owner claim routes', () => {
  test('creates a pending claim with a one-time inactive token', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    const response = await createClaim(store)
    const body = (await response.json()) as {
      claim: { id: string; status: string; claimUrl: string }
      oneTimePendingAgentToken: string
    }

    expect(response.status).toBe(201)
    expect(body.claim).toMatchObject({
      claimUrl: 'https://openagents.com/agents/claims/agent_claim_claim-1',
      id: 'agent_claim_claim-1',
      status: 'pending',
    })
    expect(body.oneTimePendingAgentToken).toBe(
      'oa_agent_pending_owner_claim_token',
    )
    expect(store.claims.get('agent_claim_claim-1')?.claimTokenHash).toBe(
      await sha256Hex('oa_agent_pending_owner_claim_token'),
    )
    expect(JSON.stringify(body.claim)).not.toContain(
      'oa_agent_pending_owner_claim_token',
    )
  })

  test('lets the pending token read status and rejects the wrong token', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store)

    const ok = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-1',
        {
          headers: {
            authorization: 'Bearer oa_agent_pending_owner_claim_token',
          },
        },
      ),
    )
    const denied = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-1',
        {
          headers: {
            authorization: 'Bearer oa_agent_wrong_token',
          },
        },
      ),
    )

    expect(ok.status).toBe(200)
    expect(
      ((await ok.json()) as { claim: { status: string } }).claim.status,
    ).toBe('pending')
    expect(denied.status).toBe(401)
  })

  test('serves an owner claim page without exposing the raw pending token', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store)

    const response = await runRoute(
      store,
      new Request('https://openagents.com/agents/claims/agent_claim_claim-1'),
    )
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe(
      'text/html; charset=utf-8',
    )
    expect(html).toContain('OpenAgents agent claim')
    expect(html).toContain('Claimed Agent')
    expect(html).toContain('/api/agents/claims/agent_claim_claim-1/approve')
    expect(html).toContain('/api/agents/claims/agent_claim_claim-1/reject')
    expect(html).toContain(
      'href="/login/github?returnTo=%2Fagents%2Fclaims%2Fagent_claim_claim-1"',
    )
    expect(html).toContain('response.status === 401')
    expect(html).toContain('sessionStorage.setItem(claimRetryKey, action)')
    expect(html).toContain("if (takePendingAction() === 'approve')")
    expect(html).toContain('window.location.assign(loginPath)')
    expect(html).not.toContain('href="/login"')
    expect(html).not.toContain('oa_agent_pending_owner_claim_token')
  })

  test('approved claim page presents the finished state without login actions', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-2',
        'user-2',
        'credential-2',
        'identity-2',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-2', 'credential-2', 'identity-2']),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )

    const response = await runRoute(
      store,
      new Request('https://openagents.com/agents/claims/agent_claim_claim-2'),
      { nowIso: () => '2026-06-06T00:06:00.000Z' },
    )
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(html).toContain(
      '<div class="row"><dt>Status</dt><dd>Approved</dd></div>',
    )
    expect(html).toContain('Approved. Original pending token is active.')
    expect(html).toContain('Token prefix: oa_agent_pending_own.')
    expect(html).not.toContain('id="approve"')
    expect(html).not.toContain('id="reject"')
    expect(html).not.toContain('Sign in')
    expect(html).not.toContain('oa_agent_pending_owner_claim_token')
  })

  test('signed-in owner approval activates the original pending token hash', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-2',
        'user-2',
        'credential-2',
        'identity-2',
      ]),
    })

    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-2', 'credential-2', 'identity-2']),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )
    const body = (await response.json()) as {
      approval: { tokenWasDisplayedAgain: boolean }
      claim: { credential: { tokenPrefix: string }; status: string }
    }
    const registration = store.registrations[0]

    expect(response.status).toBe(200)
    expect(response.headers.get('x-test-session-refreshed')).toBe('true')
    expect(body.claim.status).toBe('approved')
    expect(body.approval.tokenWasDisplayedAgain).toBe(false)
    expect(registration?.credential.tokenHash).toBe(
      await sha256Hex('oa_agent_pending_owner_claim_token'),
    )
    expect(registration?.credential.expiresAt).toBe('2026-09-04T00:05:00.000Z')
    expect(JSON.stringify(body)).not.toContain(
      'oa_agent_pending_owner_claim_token',
    )
  })

  test('signed-in owner can reject a pending claim', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store)
    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-1/reject',
        {
          body: JSON.stringify({ reason: 'Not this agent.' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { session },
    )
    const body = (await response.json()) as {
      claim: { rejectedReason: string; status: string }
    }

    expect(response.status).toBe(200)
    expect(body.claim.status).toBe('rejected')
    expect(body.claim.rejectedReason).toBe('Not this agent.')
  })

  test('expires stale pending claims before status projection', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      claimTtlMs: 1000,
      nowIso: () => '2026-06-06T00:00:00.000Z',
    })
    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-1',
        {
          headers: {
            'x-openagents-claim-token': 'oa_agent_pending_owner_claim_token',
          },
        },
      ),
      { nowIso: () => '2026-06-06T00:00:02.000Z' },
    )
    const body = (await response.json()) as {
      claim: { status: string }
    }

    expect(response.status).toBe(200)
    expect(body.claim.status).toBe('expired')
  })
})
