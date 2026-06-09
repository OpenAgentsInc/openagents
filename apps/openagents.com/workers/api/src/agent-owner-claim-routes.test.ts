import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentOwnerClaimRecord,
  type AgentOwnerClaimStore,
  type XOwnerClaimChallengeRecord,
  type XVerificationTweetLookup,
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
  readonly xChallenges = new Map<string, XOwnerClaimChallengeRecord>()

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
        record.xAccountRef === challenge.xAccountRef &&
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

  test('creates X verification tweet challenge for the approved owner', async () => {
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
    const body = (await response.json()) as {
      xClaim: {
        nonce: string
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
      xAccountRef: 'x:owner',
      xHandle: 'owner',
    })
    expect(body.xClaim.requiredText).toContain('agent_claim_claim-2')
    expect(body.xClaim.requiredText).toContain(
      'https://openagents.com/agents/claims/agent_claim_claim-2',
    )
    expect(JSON.stringify(body)).not.toContain('oauth')
    expect(JSON.stringify(body)).not.toContain(
      'oa_agent_pending_owner_claim_token',
    )
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
    expect(body.xClaim.rejectedReason).toContain('missing the required nonce')
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

  test('verifies X claim and keeps public projection token-free', async () => {
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
              'OpenAgents claim agent_claim_claim-2 oa-x-nonce1 https://openagents.com/agents/claims/agent_claim_claim-2',
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
      }
    }

    expect(response.status).toBe(200)
    expect(body.xClaim).toMatchObject({
      state: 'verified',
      tweetRef: 'x_tweet:100',
      tweetUrl: 'https://x.com/owner/status/100',
      xAccountRef: 'x:owner',
    })
    expect(JSON.stringify(body)).not.toContain('oauth')
    expect(JSON.stringify(body)).not.toContain('bearer')
    expect(JSON.stringify(body)).not.toContain(
      'oa_agent_pending_owner_claim_token',
    )
  })
})
