import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentOwnerClaimRecord,
  type AgentOwnerClaimStore,
  type XOwnerClaimChallengeRecord,
  type XVerificationTweetLookup,
  makeAgentOwnerClaimRoutes,
  type XClaimRewardRecord,
  type XClaimRewardState,
} from './agent-owner-claim-routes'
import {
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'

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
  readonly xChallenges = new Map<string, XOwnerClaimChallengeRecord>()

  readonly rewards = new Map<string, XClaimRewardRecord>()

  countXClaimRewards = async () =>
    [...this.rewards.values()].filter(
      reward => reward.state !== 'refused' && reward.state !== 'failed',
    ).length

  createXClaimReward = async (record: XClaimRewardRecord) => {
    const existing = [...this.rewards.values()].find(
      reward =>
        reward.challengeId === record.challengeId ||
        reward.xAccountRef === record.xAccountRef,
    )

    if (existing !== undefined) {
      return existing
    }

    this.rewards.set(record.id, record)

    return record
  }

  listXClaimRewards = async (limit: number) =>
    [...this.rewards.values()]
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
      .slice(0, limit)

  readXClaimRewardByChallengeId = async (challengeId: string) =>
    [...this.rewards.values()].find(
      reward => reward.challengeId === challengeId,
    )

  readXClaimRewardById = async (rewardId: string) => this.rewards.get(rewardId)

  readXClaimRewardByReceiptRef = async (receiptRef: string) =>
    [...this.rewards.values()].find(
      reward => reward.receiptRef === receiptRef,
    )

  updateXClaimRewardState = async (input: {
    evidenceRefs: ReadonlyArray<string>
    now: string
    rewardId: string
    stateReasonRef: string | null
    toState: XClaimRewardState
  }) => {
    const existing = this.rewards.get(input.rewardId)

    if (existing === undefined) {
      return undefined
    }

    const updated: XClaimRewardRecord = {
      ...existing,
      evidenceRefs: input.evidenceRefs,
      state: input.toState,
      stateReasonRef: input.stateReasonRef,
      updatedAt: input.now,
    }
    this.rewards.set(input.rewardId, updated)

    return updated
  }

  attachClaimApproval(input: {
    claimId: string
    decidedAt: string
    ownerUserId: string
  }): Promise<AgentOwnerClaimRecord | undefined> {
    const claim = this.claims.get(input.claimId)

    if (
      claim === undefined ||
      claim.status !== 'pending' ||
      claim.agentUserId === null
    ) {
      return Promise.resolve(claim)
    }

    const approved: AgentOwnerClaimRecord = {
      ...claim,
      decidedAt: input.decidedAt,
      ownerUserId: input.ownerUserId,
      status: 'approved',
      updatedAt: input.decidedAt,
    }
    this.claims.set(input.claimId, approved)

    return Promise.resolve(approved)
  }

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

  createXChallenge(
    record: XOwnerClaimChallengeRecord,
  ): Promise<XOwnerClaimChallengeRecord> {
    const duplicateActiveClaim = Array.from(this.xChallenges.values()).find(
      challenge =>
        challenge.agentClaimId === record.agentClaimId &&
        [
          'pending_owner_session',
          'pending_x_connection',
          'pending_tweet',
          'verified',
          'approved',
        ].includes(challenge.state),
    )

    if (duplicateActiveClaim !== undefined) {
      return Promise.resolve(duplicateActiveClaim)
    }

    this.xChallenges.set(record.id, record)

    return Promise.resolve(record)
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

  readActiveXChallengeByClaimId(
    claimId: string,
  ): Promise<XOwnerClaimChallengeRecord | undefined> {
    return Promise.resolve(
      Array.from(this.xChallenges.values()).find(
        challenge =>
          challenge.agentClaimId === claimId &&
          [
            'pending_owner_session',
            'pending_x_connection',
            'pending_tweet',
            'verified',
            'approved',
          ].includes(challenge.state),
      ),
    )
  }

  readXChallengeById(
    challengeId: string,
  ): Promise<XOwnerClaimChallengeRecord | undefined> {
    return Promise.resolve(this.xChallenges.get(challengeId))
  }

  readVerifiedPublicIdentityForAgentUserId() {
    return Promise.resolve(undefined)
  }

  rejectXChallenge(input: {
    challengeId: string
    now: string
    reason: string
  }): Promise<XOwnerClaimChallengeRecord | undefined> {
    const challenge = this.xChallenges.get(input.challengeId)

    if (challenge === undefined) {
      return Promise.resolve(undefined)
    }

    const rejected: XOwnerClaimChallengeRecord = {
      ...challenge,
      rejectedReason: input.reason,
      state: 'rejected',
      updatedAt: input.now,
    }
    this.xChallenges.set(input.challengeId, rejected)

    return Promise.resolve(rejected)
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

  verifyXChallenge(input: {
    challengeId: string
    now: string
    tweetRef: string
    tweetUrl: string
    xAccountRef: string
    xHandle: string
  }): Promise<XOwnerClaimChallengeRecord | undefined> {
    const challenge = this.xChallenges.get(input.challengeId)

    if (challenge === undefined || challenge.state !== 'pending_tweet') {
      return Promise.resolve(challenge)
    }

    const duplicateTweet = Array.from(this.xChallenges.values()).find(
      record =>
        record.id !== input.challengeId &&
        record.tweetRef === input.tweetRef &&
        ['verified', 'approved'].includes(record.state),
    )

    if (duplicateTweet !== undefined) {
      return Promise.reject(new Error('UNIQUE constraint failed: tweet_ref'))
    }

    const duplicateAccount = Array.from(this.xChallenges.values()).find(
      record =>
        record.id !== input.challengeId &&
        record.xAccountRef === input.xAccountRef &&
        ['verified', 'approved'].includes(record.state),
    )

    if (duplicateAccount !== undefined) {
      return Promise.reject(
        new Error('UNIQUE constraint failed: x_account_ref'),
      )
    }

    const verified: XOwnerClaimChallengeRecord = {
      ...challenge,
      state: 'verified',
      tweetRef: input.tweetRef,
      tweetUrl: input.tweetUrl,
      updatedAt: input.now,
      verifiedAt: input.now,
      xAccountRef: input.xAccountRef,
      xHandle: input.xHandle,
    }
    this.xChallenges.set(input.challengeId, verified)

    return Promise.resolve(verified)
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
    resolveXVerificationTweet?: (
      input: Readonly<{ tweetUrl: string }>,
    ) => Promise<XVerificationTweetLookup>
    agentStore?: AgentRegistrationStore
    requireAdminApiToken?: () => Promise<boolean>
    rewardCampaignCap?: number
    session?: TestSession | undefined
  }> = {},
) =>
  makeAgentOwnerClaimRoutes<TestSession, { store: MemoryAgentOwnerClaimStore }>(
    {
      ...(options.agentStore === undefined
        ? {}
        : { agentStore: () => options.agentStore as AgentRegistrationStore }),
      ...(options.requireAdminApiToken === undefined
        ? {}
        : { requireAdminApiToken: options.requireAdminApiToken }),
      ...(options.rewardCampaignCap === undefined
        ? {}
        : { rewardCampaignCap: options.rewardCampaignCap }),
      appOrigin: () => 'https://openagents.com',
      appendRefreshedSessionCookies: response => {
        response.headers.set('x-test-session-refreshed', 'true')

        return response
      },
      makeStore: env => env.store,
      makeToken: () => 'oa_agent_pending_owner_claim_token',
      makeUuid: options.makeUuid ?? makeUuidFactory(['claim-1']),
      nowIso: options.nowIso ?? (() => '2026-06-06T00:00:00.000Z'),
      ...(options.resolveXVerificationTweet === undefined
        ? {}
        : { resolveXVerificationTweet: options.resolveXVerificationTweet }),
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

  test('attaches an owner claim to an existing registered agent without new identity inserts', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    const existingTokenHash = await sha256Hex('oa_agent_existing_agent_token')
    const agentStore = {
      findAgentByTokenHash: (tokenHash: string) =>
        Promise.resolve(
          tokenHash === existingTokenHash
            ? {
                credentialId: 'agent_credential_existing',
                profileMetadataJson: '{}',
                tokenPrefix: 'oa_agent_existing_a',
                user: {
                  avatarUrl: null,
                  createdAt: '2026-06-01T00:00:00.000Z',
                  displayName: 'Existing Agent',
                  id: 'user_existing_agent',
                  kind: 'agent' as const,
                  primaryEmail: null,
                  status: 'active' as const,
                  updatedAt: '2026-06-01T00:00:00.000Z',
                },
              }
            : undefined,
        ),
      touchAgentCredential: () => Promise.resolve(),
    } as unknown as AgentRegistrationStore
    const created = await runRoute(
      store,
      new Request('https://openagents.com/api/agents/claims', {
        body: JSON.stringify({ displayName: 'Existing Agent' }),
        headers: {
          authorization: 'Bearer oa_agent_existing_agent_token',
          'content-type': 'application/json',
        },
        method: 'POST',
      }),
      { agentStore },
    )
    const createdBody = (await created.json()) as {
      claim: { agentUserRef: string | null; id: string; status: string }
      instructions: ReadonlyArray<string>
    }
    const denied = await runRoute(
      store,
      new Request('https://openagents.com/api/agents/claims', {
        body: JSON.stringify({ displayName: 'Existing Agent' }),
        headers: {
          authorization: 'Bearer oa_agent_unknown_token',
          'content-type': 'application/json',
        },
        method: 'POST',
      }),
      { agentStore },
    )
    const approved = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-1/approve',
        { method: 'POST' },
      ),
      {
        agentStore,
        session: {
          user: { email: 'owner@example.com', userId: 'user_owner_1' },
        } as unknown as TestSession,
      },
    )
    const approvedBody = (await approved.json()) as {
      approval: {
        attachedAgentUserRef: string
        tokenWasDisplayedAgain: boolean
      }
      claim: { status: string }
    }

    expect(created.status).toBe(201)
    expect(createdBody.claim.status).toBe('pending')
    expect(createdBody.instructions.join(' ')).toContain(
      'attaches to your existing registered agent identity',
    )
    expect(denied.status).toBe(401)
    expect(approved.status).toBe(200)
    expect(approvedBody.claim.status).toBe('approved')
    expect(approvedBody.approval).toMatchObject({
      attachedAgentUserRef: 'agent:user_existing_agent',
      tokenWasDisplayedAgain: false,
    })
    expect(store.registrations).toHaveLength(0)
    expect(store.claims.get('agent_claim_claim-1')?.ownerUserId).toBe(
      'user_owner_1',
    )
    expect(store.claims.get('agent_claim_claim-1')?.agentUserId).toBe(
      'user_existing_agent',
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
    expect(html).toContain('Verify on X')
    expect(html).toContain('id="prepareXClaim"')
    expect(html).toContain('/x/challenge')
    expect(html).toContain('/x/verify')
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

  test('requires signed-in owner session before starting X claim challenge', async () => {
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
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'owner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('creates friendly X verification tweet challenge for the approved owner', async () => {
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
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({}),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-1', 'nonce-1']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: {
        nonce: string
        postIntentUrl: string
        requiredText: string
        state: string
        xAccountRef: string
        xHandle: string
      }
    }

    expect(response.status).toBe(201)
    expect(body.xClaim).toMatchObject({
      nonce: 'oa-x-nonce1',
      state: 'pending_tweet',
      xAccountRef: 'x:pending:agent_x_claim_x-1',
      xHandle: '',
    })
    expect(body.xClaim.requiredText).toBe(
      'Verifying my agent Claimed Agent is joining @OpenAgents\n\nCode: oa-x-nonce1',
    )
    expect(body.xClaim.postIntentUrl).toContain('https://x.com/intent/post')
    expect(new URL(body.xClaim.postIntentUrl).searchParams.get('text')).toContain(
      'Verifying my agent Claimed Agent is joining @OpenAgents',
    )
    expect(JSON.stringify(body)).not.toContain('oauth')
    expect(JSON.stringify(body)).not.toContain(
      'oa_agent_pending_owner_claim_token',
    )
  })

  test('keeps the X verification code at 12 characters or fewer', async () => {
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
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({}),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory([
          '123e4567-e89b-42d3-a456-426614174000',
          'f47ac10b-58cc-4372-a567-0e02b2c3d479',
        ]),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: { nonce: string; requiredText: string }
    }

    expect(response.status).toBe(201)
    expect(body.xClaim.nonce).toBe('oa-x-f47ac10')
    expect(body.xClaim.nonce.length).toBeLessThanOrEqual(12)
    expect(body.xClaim.requiredText).toContain('Code: oa-x-f47ac10')
  })

  test('supersedes a stale long-code challenge that has not been tweeted', async () => {
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
    await store.createXChallenge({
      agentClaimId: 'agent_claim_claim-2',
      agentUserId: 'user-2',
      caveatRefsJson: '[]',
      createdAt: '2026-06-06T00:05:30.000Z',
      expiresAt: '2026-06-08T00:05:30.000Z',
      id: 'agent_x_claim_legacy-long',
      nonce: 'oa-x-709b776813a047cfbf0c8ce8',
      ownerUserId: session.user.userId,
      policyRefsJson: '[]',
      receiptRef: 'agent_x_claim_receipt_agent_x_claim_legacy-long',
      rejectedReason: null,
      requiredText:
        'Verifying my agent Claimed Agent is joining @OpenAgents\n\nCode: oa-x-709b776813a047cfbf0c8ce8',
      requiredUrl:
        'https://openagents.com/agents/claims/agent_claim_claim-2',
      state: 'pending_tweet',
      tweetRef: null,
      tweetUrl: null,
      updatedAt: '2026-06-06T00:05:30.000Z',
      verifiedAt: null,
      xAccountRef: 'x:pending:agent_x_claim_legacy-long',
      xHandle: '',
    })

    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({}),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory([
          '123e4567-e89b-42d3-a456-426614174000',
          'f47ac10b-58cc-4372-a567-0e02b2c3d479',
        ]),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: { nonce: string; state: string }
    }

    expect(response.status).toBe(201)
    expect(body.xClaim.nonce).toBe('oa-x-f47ac10')
    expect(body.xClaim.state).toBe('pending_tweet')

    const superseded = await store.readXChallengeById(
      'agent_x_claim_legacy-long',
    )
    expect(superseded?.state).toBe('rejected')
    expect(superseded?.rejectedReason).toBe(
      'superseded_by_short_verification_code',
    )
  })

  test('escapes friendly tweet copy inside the X intent URL', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await runRoute(
      store,
      new Request('https://openagents.com/api/agents/claims', {
        body: JSON.stringify({
          displayName: 'A&B <Agent>',
          externalId: 'escaped-agent-local',
          metadata: { purpose: 'owner_claim_test' },
          slug: 'escaped-agent',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      {
        makeUuid: makeUuidFactory([
          'claim-4',
          'user-4',
          'credential-4',
          'identity-4',
        ]),
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-4/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-4', 'credential-4', 'identity-4']),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )

    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-4/x/challenge',
        {
          body: JSON.stringify({}),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-4', 'nonce-4']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: { postIntentUrl: string; requiredText: string }
    }
    const intent = new URL(body.xClaim.postIntentUrl)

    expect(intent.origin + intent.pathname).toBe('https://x.com/intent/post')
    expect(intent.searchParams.get('text')).toBe(
      'Verifying my agent A&B <Agent> is joining @OpenAgents\n\nCode: oa-x-nonce4',
    )
    expect(body.xClaim.requiredText).toContain('A&B <Agent>')
  })

  test('rejects X verification when nonce is missing', async () => {
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
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'owner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-1', 'nonce-1']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )

    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/owner/status/100',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'owner',
            htmlText:
              'OpenAgents claim agent_claim_claim-2 https://openagents.com/agents/claims/agent_claim_claim-2',
            state: 'visible',
            tweetRef: 'x_tweet:100',
            tweetUrl: 'https://x.com/owner/status/100',
          }),
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: { rejectedReason: string; state: string }
    }

    expect(response.status).toBe(409)
    expect(body.xClaim.state).toBe('rejected')
    expect(body.xClaim.rejectedReason).toContain('missing the required code')
  })

  test('rejects X verification from the wrong account', async () => {
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
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'owner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-1', 'nonce-1']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )

    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/intruder/status/100',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'intruder',
            htmlText:
              'OpenAgents claim agent_claim_claim-2 oa-x-nonce1 https://openagents.com/agents/claims/agent_claim_claim-2',
            state: 'visible',
            tweetRef: 'x_tweet:100',
            tweetUrl: 'https://x.com/intruder/status/100',
          }),
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: { rejectedReason: string; state: string }
    }

    expect(response.status).toBe(409)
    expect(body.xClaim.state).toBe('rejected')
    expect(body.xClaim.rejectedReason).toContain('not published')
  })

  test('verifies code-only X claim, binds the author, and keeps projection token-free', async () => {
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
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({}),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-1', 'nonce-1']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )

    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/owner/status/100',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'owner',
            htmlText:
              'Verifying my agent Claimed Agent is joining @OpenAgents Code: oa-x-nonce1',
            state: 'visible',
            tweetRef: 'x_tweet:100',
            tweetUrl: 'https://x.com/owner/status/100',
          }),
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: {
        state: string
        tweetRef: string
        tweetUrl: string
        xAccountRef: string
        xHandle: string
      }
    }

    expect(response.status).toBe(200)
    expect(body.xClaim).toMatchObject({
      state: 'verified',
      tweetRef: 'x_tweet:100',
      tweetUrl: 'https://x.com/owner/status/100',
      xAccountRef: 'x:owner',
      xHandle: 'owner',
    })
    expect(JSON.stringify(body)).not.toContain('oauth')
    expect(JSON.stringify(body)).not.toContain('bearer')
    expect(JSON.stringify(body)).not.toContain(
      'oa_agent_pending_owner_claim_token',
    )
  })

  test('continues to verify old-format tweets during the transition window', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-7',
        'user-7',
        'credential-7',
        'identity-7',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-7/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-7', 'credential-7', 'identity-7']),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-7/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'owner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-7', 'nonce-7']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const challenge = store.xChallenges.get('agent_x_claim_x-7')
    if (challenge === undefined) {
      throw new Error('expected challenge')
    }
    store.xChallenges.set(challenge.id, {
      ...challenge,
      requiredText:
        'OpenAgents claim agent_claim_claim-7 oa-x-nonce7 https://openagents.com/agents/claims/agent_claim_claim-7',
    })

    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-7/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/owner/status/700',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'owner',
            htmlText:
              'OpenAgents claim agent_claim_claim-7 oa-x-nonce7 https://openagents.com/agents/claims/agent_claim_claim-7',
            state: 'visible',
            tweetRef: 'x_tweet:700',
            tweetUrl: 'https://x.com/owner/status/700',
          }),
        session,
      },
    )
    const body = (await response.json()) as {
      xClaim: { state: string; xAccountRef: string }
    }

    expect(response.status).toBe(200)
    expect(body.xClaim).toMatchObject({
      state: 'verified',
      xAccountRef: 'x:owner',
    })
  })

  test('creates reward eligibility on X verification and gates operator dispatch', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-9',
        'user-9',
        'credential-9',
        'identity-9',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-9/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-9', 'credential-9', 'identity-9']),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-9/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'rewardowner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-9', 'nonce-9']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const verifyResponse = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-9/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/rewardowner/status/900',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['reward-9']),
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'rewardowner',
            htmlText:
              'OpenAgents claim agent_claim_claim-9 oa-x-nonce9 https://openagents.com/agents/claims/agent_claim_claim-9',
            state: 'visible',
            tweetRef: 'x_tweet:900',
            tweetUrl: 'https://x.com/rewardowner/status/900',
          }),
        session,
      },
    )
    const verifyBody = (await verifyResponse.json()) as {
      reward: { amountSats: number; rewardId: string; state: string }
    }
    const rewardId = verifyBody.reward.rewardId
    const dispatchDenied = await runRoute(
      store,
      new Request(
        `https://openagents.com/api/agents/claims/rewards/${rewardId}/dispatch`,
        {
          body: JSON.stringify({ action: 'approve_dispatch' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { requireAdminApiToken: () => Promise.resolve(false) },
    )
    const approveDispatch = await runRoute(
      store,
      new Request(
        `https://openagents.com/api/agents/claims/rewards/${rewardId}/dispatch`,
        {
          body: JSON.stringify({ action: 'approve_dispatch' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { requireAdminApiToken: () => Promise.resolve(true) },
    )
    const markDispatched = await runRoute(
      store,
      new Request(
        `https://openagents.com/api/agents/claims/rewards/${rewardId}/dispatch`,
        {
          body: JSON.stringify({ action: 'mark_dispatched' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { requireAdminApiToken: () => Promise.resolve(true) },
    )
    const settleWithoutEvidence = await runRoute(
      store,
      new Request(
        `https://openagents.com/api/agents/claims/rewards/${rewardId}/dispatch`,
        {
          body: JSON.stringify({ action: 'mark_settled' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { requireAdminApiToken: () => Promise.resolve(true) },
    )
    const settled = await runRoute(
      store,
      new Request(
        `https://openagents.com/api/agents/claims/rewards/${rewardId}/dispatch`,
        {
          body: JSON.stringify({
            action: 'mark_settled',
            evidenceRefs: [
              'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
            ],
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { requireAdminApiToken: () => Promise.resolve(true) },
    )
    const settledBody = (await settled.json()) as {
      reward: { state: string }
    }

    expect(verifyResponse.status).toBe(200)
    expect(verifyBody.reward).toMatchObject({
      amountSats: 1000,
      state: 'eligible',
    })
    expect(dispatchDenied.status).toBe(401)
    expect(approveDispatch.status).toBe(200)
    expect(markDispatched.status).toBe(200)
    expect(settleWithoutEvidence.status).toBe(400)
    expect(settled.status).toBe(200)
    expect(settledBody.reward.state).toBe('settled')
    expect(JSON.stringify(settledBody)).not.toContain('lnbc')
  })

  test('serves the public x_claim_reward eligibility read path after verification (#4754)', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-12',
        'user-12',
        'credential-12',
        'identity-12',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-12/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-12', 'credential-12', 'identity-12']),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-12/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'eligiblereader' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-12', 'nonce-12']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const verifyResponse = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-12/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/eligiblereader/status/1200',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['reward-12']),
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'eligiblereader',
            htmlText:
              'OpenAgents claim agent_claim_claim-12 oa-x-nonce12 https://openagents.com/agents/claims/agent_claim_claim-12',
            state: 'visible',
            tweetRef: 'x_tweet:1200',
            tweetUrl: 'https://x.com/eligiblereader/status/1200',
          }),
        session,
      },
    )
    const verifyBody = (await verifyResponse.json()) as {
      reward: { receiptRef: string; rewardId: string }
    }

    // The owner cites the refs the verify response returned; both
    // resolve on the public read path with no session.
    const byReceiptRef = await runRoute(
      store,
      new Request(
        `https://openagents.com/api/agents/claims/rewards/${verifyBody.reward.receiptRef}`,
      ),
      { session: undefined },
    )
    const byReceiptBody = (await byReceiptRef.json()) as Record<
      string,
      unknown
    >

    expect(byReceiptRef.status).toBe(200)
    expect(byReceiptBody).toMatchObject({
      contractVersion: 'projection.x_claim_reward_eligibility.v1',
      reward: {
        lifecycleStage: 'eligible',
        receiptRef: verifyBody.reward.receiptRef,
        rewardId: verifyBody.reward.rewardId,
        state: 'eligible',
      },
    })

    const list = await runRoute(
      store,
      new Request('https://openagents.com/api/agents/claims/rewards'),
      { session: undefined },
    )
    const listBody = (await list.json()) as Record<string, unknown>

    expect(list.status).toBe(200)
    expect(listBody).toMatchObject({
      counts: { eligible: 1 },
      lifecycle: ['eligible', 'operator_approved', 'dispatched', 'settled'],
    })
    // No X handle, owner id, or agent user id leaves the ledger.
    const publicJson = JSON.stringify(listBody) + JSON.stringify(byReceiptBody)
    expect(publicJson).not.toContain('eligiblereader')
    expect(publicJson).not.toContain('github:owner-1')
    expect(publicJson).not.toContain('agent_user_user-12')
    expect(typeof (listBody as { generatedAt?: unknown }).generatedAt).toBe(
      'string',
    )
  })

  test('refuses reward eligibility when the campaign budget is exhausted', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-8',
        'user-8',
        'credential-8',
        'identity-8',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-8/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-8', 'credential-8', 'identity-8']),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-8/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'capowner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-8', 'nonce-8']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    const verifyResponse = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-8/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/capowner/status/800',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['reward-8']),
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'capowner',
            htmlText:
              'OpenAgents claim agent_claim_claim-8 oa-x-nonce8 https://openagents.com/agents/claims/agent_claim_claim-8',
            state: 'visible',
            tweetRef: 'x_tweet:800',
            tweetUrl: 'https://x.com/capowner/status/800',
          }),
        rewardCampaignCap: 0,
        session,
      },
    )
    const verifyBody = (await verifyResponse.json()) as {
      reward: { state: string; stateReasonRef: string }
    }

    expect(verifyBody.reward).toMatchObject({
      state: 'refused',
      stateReasonRef:
        'reason.public.x_claim_reward_campaign_budget_exhausted',
    })
  })

  test('rejects a second verified claim for the same X account', async () => {
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
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/challenge',
        {
          body: JSON.stringify({ xHandle: '@Owner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-1', 'nonce-1']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-2/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/owner/status/100',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'owner',
            htmlText:
              'OpenAgents claim agent_claim_claim-2 oa-x-nonce1 https://openagents.com/agents/claims/agent_claim_claim-2',
            state: 'visible',
            tweetRef: 'x_tweet:100',
            tweetUrl: 'https://x.com/owner/status/100',
          }),
        session,
      },
    )

    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-3',
        'user-3',
        'credential-3',
        'identity-3',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-3/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory(['user-3', 'credential-3', 'identity-3']),
        nowIso: () => '2026-06-06T00:08:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-3/x/challenge',
        {
          body: JSON.stringify({ xHandle: 'owner' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-2', 'nonce-2']),
        nowIso: () => '2026-06-06T00:09:00.000Z',
        session,
      },
    )

    const duplicateResponse = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-3/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/owner/status/101',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:10:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'owner',
            htmlText:
              'OpenAgents claim agent_claim_claim-3 oa-x-nonce2 https://openagents.com/agents/claims/agent_claim_claim-3',
            state: 'visible',
            tweetRef: 'x_tweet:101',
            tweetUrl: 'https://x.com/owner/status/101',
          }),
        session,
      },
    )
    const body = (await duplicateResponse.json()) as { error: string }
    const verifiedOwnerClaims = Array.from(store.xChallenges.values()).filter(
      challenge =>
        challenge.xAccountRef === 'x:owner' &&
        ['verified', 'approved'].includes(challenge.state),
    )

    expect(duplicateResponse.status).toBe(409)
    expect(body.error).toBe('agent_x_claim_duplicate_tweet_or_account')
    expect(verifiedOwnerClaims).toHaveLength(1)
    expect(store.xChallenges.get('agent_x_claim_x-2')).toMatchObject({
      state: 'pending_tweet',
      tweetRef: null,
      xAccountRef: 'x:owner',
    })
  })

  test('rejects replaying the same tweet on a second claim', async () => {
    const store = new MemoryAgentOwnerClaimStore()
    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-10',
        'user-10',
        'credential-10',
        'identity-10',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-10/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory([
          'user-10',
          'credential-10',
          'identity-10',
        ]),
        nowIso: () => '2026-06-06T00:05:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-10/x/challenge',
        {
          body: JSON.stringify({}),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-10', 'nonce-10']),
        nowIso: () => '2026-06-06T00:06:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-10/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/owner/status/1000',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:07:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'owner',
            htmlText: 'Code: oa-x-nonce10',
            state: 'visible',
            tweetRef: 'x_tweet:1000',
            tweetUrl: 'https://x.com/owner/status/1000',
          }),
        session,
      },
    )

    await createClaim(store, {
      makeUuid: makeUuidFactory([
        'claim-11',
        'user-11',
        'credential-11',
        'identity-11',
      ]),
    })
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-11/approve',
        { method: 'POST' },
      ),
      {
        makeUuid: makeUuidFactory([
          'user-11',
          'credential-11',
          'identity-11',
        ]),
        nowIso: () => '2026-06-06T00:08:00.000Z',
        session,
      },
    )
    await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-11/x/challenge',
        {
          body: JSON.stringify({}),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        makeUuid: makeUuidFactory(['x-11', 'nonce-11']),
        nowIso: () => '2026-06-06T00:09:00.000Z',
        session,
      },
    )

    const replay = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/claims/agent_claim_claim-11/x/verify',
        {
          body: JSON.stringify({
            tweetUrl: 'https://x.com/other/status/1000',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        nowIso: () => '2026-06-06T00:10:00.000Z',
        resolveXVerificationTweet: () =>
          Promise.resolve({
            authorHandle: 'other',
            htmlText: 'Code: oa-x-nonce11',
            state: 'visible',
            tweetRef: 'x_tweet:1000',
            tweetUrl: 'https://x.com/other/status/1000',
          }),
        session,
      },
    )
    const body = (await replay.json()) as { error: string }

    expect(replay.status).toBe(409)
    expect(body.error).toBe('agent_x_claim_duplicate_tweet_or_account')
    expect(store.xChallenges.get('agent_x_claim_x-11')).toMatchObject({
      state: 'pending_tweet',
      tweetRef: null,
      xAccountRef: 'x:pending:agent_x_claim_x-11',
    })
  })
})
