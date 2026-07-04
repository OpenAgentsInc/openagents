import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentRateLimitChallengeRecord,
  type AgentRateLimitEntitlementRecord,
  type AgentRateLimitReceiptRecord,
  type AgentRateLimitRecoveryRuntime,
  type AgentRateLimitRecoveryStore,
  type AgentRateLimitRedemptionRecord,
  PublicAgentProposalRecoveryRoute,
  systemAgentRateLimitRecoveryRuntime,
} from './agent-rate-limit-recovery'
import {
  AGENT_TOKEN_PREFIX,
  type AgentCredentialLookup,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  type AgentProposalRecord,
  type AgentProposalStore,
  makeMirroredAgentProposalStore,
  makeAgentProposalRoutes,
} from './agent-proposal-routes'
import type { AgentRuntimeRemainderMirror } from './agent-runtime-remainder-store'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

class MemoryAgentProposalStore implements AgentProposalStore {
  readonly proposals = new Map<string, AgentProposalRecord>()

  countRecentByClientFingerprint(
    clientFingerprintHash: string,
    sinceIso: string,
  ): Promise<number> {
    return Promise.resolve(
      Array.from(this.proposals.values()).filter(
        proposal =>
          proposal.clientFingerprintHash === clientFingerprintHash &&
          proposal.createdAt >= sinceIso,
      ).length,
    )
  }

  createProposal(record: AgentProposalRecord): Promise<void> {
    this.proposals.set(record.id, record)

    return Promise.resolve()
  }

  listProposals(input: {
    limit: number
    status: AgentProposalRecord['status'] | 'all'
  }): Promise<ReadonlyArray<AgentProposalRecord>> {
    return Promise.resolve(
      Array.from(this.proposals.values())
        .filter(proposal => input.status === 'all' || proposal.status === input.status)
        .slice(0, input.limit),
    )
  }

  readById(proposalId: string): Promise<AgentProposalRecord | undefined> {
    return Promise.resolve(this.proposals.get(proposalId))
  }

  readByIdempotencyKeyHash(
    idempotencyKeyHash: string,
  ): Promise<AgentProposalRecord | undefined> {
    return Promise.resolve(
      Array.from(this.proposals.values()).find(
        proposal => proposal.idempotencyKeyHash === idempotencyKeyHash,
      ),
    )
  }

  transitionProposal(input: {
    decidedAt: string
    note: string | null
    operatorUserId: string
    promotedTargetRef?: string | null
    promotionKind?: AgentProposalRecord['promotionKind'] | null
    proposalId: string
    status: Extract<AgentProposalRecord['status'], 'promoted' | 'rejected'>
  }): Promise<AgentProposalRecord | undefined> {
    const proposal = this.proposals.get(input.proposalId)

    if (proposal === undefined || proposal.status !== 'pending') {
      return Promise.resolve(proposal)
    }

    const transitioned: AgentProposalRecord = {
      ...proposal,
      decidedAt: input.decidedAt,
      operatorNote: input.note,
      operatorUserId: input.operatorUserId,
      promotedTargetRef: input.promotedTargetRef ?? null,
      promotionKind: input.promotionKind ?? null,
      status: input.status,
      updatedAt: input.decidedAt,
    }
    this.proposals.set(input.proposalId, transitioned)

    return Promise.resolve(transitioned)
  }
}

class MemoryAgentRuntimeRemainderMirror implements AgentRuntimeRemainderMirror {
  readonly calls: Array<{
    pkValues: ReadonlyArray<string>
    table: string
  }> = []

  mirrorRowsByPk = async (
    table: Parameters<AgentRuntimeRemainderMirror['mirrorRowsByPk']>[0],
    pkValues: ReadonlyArray<string>,
  ) => {
    this.calls.push({ pkValues, table })
  }
}

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly touched: Array<string> = []

  constructor(
    private readonly input: Readonly<{
      lookup?: AgentCredentialLookup | undefined
      token: string
    }>,
  ) {}

  createAgentRegistration = async () => {}

  findAgentByTokenHash = async (tokenHash: string) =>
    tokenHash === (await sha256Hex(this.input.token))
      ? this.input.lookup
      : undefined

  touchAgentCredential = async (credentialId: string) => {
    this.touched.push(credentialId)
  }

  updateAgentDisplayName = async () => 0
}

class MemoryAgentRateLimitRecoveryStore implements AgentRateLimitRecoveryStore {
  readonly challenges = new Map<string, AgentRateLimitChallengeRecord>()
  readonly entitlements = new Map<string, AgentRateLimitEntitlementRecord>()
  readonly receipts = new Map<string, AgentRateLimitReceiptRecord>()
  readonly redemptions = new Map<string, AgentRateLimitRedemptionRecord>()

  consumeEntitlement = async (
    input: Parameters<AgentRateLimitRecoveryStore['consumeEntitlement']>[0],
  ) => {
    const entitlement = this.entitlements.get(input.entitlementRef)

    if (
      entitlement === undefined ||
      entitlement.actorRef !== input.actorRef ||
      entitlement.clientFingerprintHash !== input.clientFingerprintHash ||
      entitlement.method !== input.method ||
      entitlement.path !== input.path ||
      entitlement.requestBodyDigest !== input.requestBodyDigest ||
      entitlement.routeKey !== input.routeKey ||
      entitlement.submissionIdempotencyKeyHash !==
        input.submissionIdempotencyKeyHash ||
      entitlement.status !== 'active' ||
      entitlement.expiresAt <= input.nowIso
    ) {
      return undefined
    }

    const consumed: AgentRateLimitEntitlementRecord = {
      ...entitlement,
      consumedAt: input.nowIso,
      status: 'consumed',
    }
    this.entitlements.set(consumed.entitlementRef, consumed)

    return consumed
  }

  createChallenge = async (record: AgentRateLimitChallengeRecord) => {
    if (
      Array.from(this.challenges.values()).every(
        challenge =>
          challenge.idempotencyKeyHash !== record.idempotencyKeyHash,
      )
    ) {
      this.challenges.set(record.id, record)
    }
  }

  createRedemptionBundle = async (
    input: Parameters<
      AgentRateLimitRecoveryStore['createRedemptionBundle']
    >[0],
  ) => {
    if (
      Array.from(this.redemptions.values()).some(
        redemption => redemption.challengeId === input.redemption.challengeId,
      )
    ) {
      return
    }

    this.receipts.set(input.receipt.receiptRef, input.receipt)
    this.entitlements.set(input.entitlement.entitlementRef, input.entitlement)
    this.redemptions.set(input.redemption.challengeId, input.redemption)
  }

  readChallengeById = async (challengeId: string) =>
    this.challenges.get(challengeId)

  readChallengeByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    Array.from(this.challenges.values()).find(
      challenge => challenge.idempotencyKeyHash === idempotencyKeyHash,
    )

  readEntitlementByRef = async (entitlementRef: string) =>
    this.entitlements.get(entitlementRef)

  readReceiptByRef = async (receiptRef: string) => this.receipts.get(receiptRef)

  readRedemptionByChallengeId = async (challengeId: string) =>
    this.redemptions.get(challengeId)
}

const adminSession: TestSession = {
  user: {
    email: 'chris@openagents.com',
    userId: 'github:admin',
  },
}

const makeRoutes = (
  store: MemoryAgentProposalStore,
  options: Readonly<{
    adminToken?: boolean
    agentLookup?: AgentCredentialLookup
    agentToken?: string
    makeUuid?: () => string
    nowIso?: () => string
    proposalRateLimit?: number
    recoveryRuntime?: AgentRateLimitRecoveryRuntime
    recoveryStore?: MemoryAgentRateLimitRecoveryStore
    session?: TestSession | undefined
  }> = {},
) => {
  const agentLookup = options.agentLookup
  const recoveryStore = options.recoveryStore

  return makeAgentProposalRoutes<
    TestSession,
    {
      agentStore?: MemoryAgentRegistrationStore
      recoveryStore?: MemoryAgentRateLimitRecoveryStore
      store: MemoryAgentProposalStore
    }
  >({
    ...(agentLookup === undefined
      ? {}
      : {
          agentStore: env =>
            env.agentStore ??
            new MemoryAgentRegistrationStore({
              lookup: agentLookup,
              token: options.agentToken ?? `${AGENT_TOKEN_PREFIX}test`,
            }),
        }),
    appOrigin: () => 'https://openagents.com',
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-test-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email.endsWith('@openagents.com'),
    makeStore: env => env.store,
    makeUuid: options.makeUuid ?? (() => 'proposal-1'),
    nowIso: options.nowIso ?? (() => '2026-06-06T00:00:00.000Z'),
    requireAdminApiToken: () => Promise.resolve(options.adminToken === true),
    requireBrowserSession: () => Promise.resolve(options.session),
    ...(recoveryStore === undefined
      ? {}
      : {
          rateLimitRecoveryRuntime:
            options.recoveryRuntime ?? systemAgentRateLimitRecoveryRuntime,
          recoveryStore: () => recoveryStore,
        }),
    ...(options.proposalRateLimit === undefined
      ? {}
      : { proposalRateLimit: options.proposalRateLimit }),
  })
}

const runRoute = async (
  store: MemoryAgentProposalStore,
  request: Request,
  options: Parameters<typeof makeRoutes>[1] = {},
): Promise<Response> => {
  const response = makeRoutes(store, options).routeAgentProposalRequest(
    request,
    {
      ...(options.agentLookup === undefined
        ? {}
        : {
            agentStore: new MemoryAgentRegistrationStore({
              lookup: options.agentLookup,
              token: options.agentToken ?? `${AGENT_TOKEN_PREFIX}test`,
            }),
          }),
      ...(options.recoveryStore === undefined
        ? {}
        : { recoveryStore: options.recoveryStore }),
      store,
    },
    {} as ExecutionContext,
  )

  if (response === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(response)
}

const proposalRequest = (
  idempotencyKey = 'proposal-key-1',
  input: Readonly<{
    authorization?: string
    entitlementRef?: string
    title?: string
  }> = {},
): Request =>
  new Request('https://openagents.com/api/agents/proposals', {
    body: JSON.stringify({
      author: { agentName: 'Dry Run Agent' },
      bodyText:
        'This is a bounded proposal with public evidence and no authority-bearing action.',
      kind: 'site_improvement',
      sourceUrls: ['https://example.com/source'],
      summary: 'Improve the public OTEC page with a clearer evidence section.',
      target: { siteSlug: 'otec' },
      title: input.title ?? 'Add clearer OTEC evidence',
    }),
    headers: {
      ...(input.authorization === undefined
        ? {}
        : { authorization: input.authorization }),
      'content-type': 'application/json',
      'idempotency-key': idempotencyKey,
      ...(input.entitlementRef === undefined
        ? {}
        : {
            'x-openagents-rate-limit-entitlement': input.entitlementRef,
          }),
    },
    method: 'POST',
  })

const agentToken = `${AGENT_TOKEN_PREFIX}proposal-recovery-test`
const agentAuthorization = `Bearer ${agentToken}`
const recoveryAgentLookup = (): AgentCredentialLookup => ({
  credentialId: 'agent_credential_recovery',
  profileMetadataJson: JSON.stringify({
    agentRateLimitRecoveryGrants: [
      {
        expiresAt: null,
        ownerUserId: 'github:owner',
        routeKeys: [PublicAgentProposalRecoveryRoute.routeKey],
        spendCap: {
          amount: 100,
          asset: 'bitcoin',
          denomination: 'sats',
        },
        status: 'active',
      },
    ],
  }),
  tokenPrefix: `${AGENT_TOKEN_PREFIX}proposal`,
  user: {
    avatarUrl: null,
    createdAt: '2026-06-05T00:00:00.000Z',
    displayName: 'Proposal Recovery Agent',
    id: 'agent_user_recovery',
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: '2026-06-05T00:00:00.000Z',
  },
})

const recoveryRuntime: AgentRateLimitRecoveryRuntime = {
  ...systemAgentRateLimitRecoveryRuntime,
  makeChallengeId: () => 'agent_rate_limit_challenge_test',
  makeEntitlementId: () => 'agent_rate_limit_entitlement_test',
  makeReceiptId: () => 'agent_rate_limit_receipt_test',
  makeRedemptionId: () => 'agent_rate_limit_redemption_test',
  nowIso: () => '2026-06-06T00:00:00.000Z',
  nowMillis: () => Date.parse('2026-06-06T00:00:00.000Z'),
}

const recoveryPreviewRequest = (idempotencyKey: string) =>
  new Request('https://openagents.com/api/agents/proposals/rate-limit/preview', {
    body: JSON.stringify({
      idempotencyKey,
      proposal: {
        author: { agentName: 'Dry Run Agent' },
        bodyText:
          'This is a bounded proposal with public evidence and no authority-bearing action.',
        kind: 'site_improvement',
        sourceUrls: ['https://example.com/source'],
        summary: 'Improve the public OTEC page with a clearer evidence section.',
        target: { siteSlug: 'otec' },
        title: 'Add clearer OTEC evidence',
      },
      spendCap: {
        amount: 100,
        asset: 'bitcoin',
        denomination: 'sats',
      },
    }),
    headers: {
      authorization: agentAuthorization,
      'content-type': 'application/json',
      'idempotency-key': 'preview-key-1',
    },
    method: 'POST',
  })

const proposalRecord = (id: string): AgentProposalRecord => ({
  authorJson: JSON.stringify({ agentName: 'Mirror Agent' }),
  bodyText:
    'This is a bounded proposal with public evidence and no authority-bearing action.',
  clientFingerprintHash: 'fingerprint_hash_mirror',
  createdAt: '2026-06-06T00:00:00.000Z',
  decidedAt: null,
  id,
  idempotencyKeyHash: `idempotency_hash_${id}`,
  kind: 'site_improvement',
  operatorNote: null,
  operatorUserId: null,
  promotedTargetRef: null,
  promotionKind: null,
  receiptRef: `agent_proposal_receipt_${id}`,
  sourceUrlsJson: JSON.stringify(['https://example.com/source']),
  status: 'pending',
  summary: 'Improve a public evidence section.',
  targetJson: JSON.stringify({ siteSlug: 'otec' }),
  title: 'Improve public evidence',
  updatedAt: '2026-06-06T00:00:00.000Z',
})

describe('agent proposal routes', () => {
  test('mirrored store mirrors proposal creates and transitions by key only', async () => {
    const d1 = new MemoryAgentProposalStore()
    const mirror = new MemoryAgentRuntimeRemainderMirror()
    const store = makeMirroredAgentProposalStore(d1, mirror)

    await store.createProposal(proposalRecord('agent_proposal_mirror'))
    await store.transitionProposal({
      decidedAt: '2026-06-06T00:05:00.000Z',
      note: 'Looks useful.',
      operatorUserId: 'github:admin',
      promotedTargetRef: 'site_feedback:otec:evidence',
      promotionKind: 'site_feedback',
      proposalId: 'agent_proposal_mirror',
      status: 'promoted',
    })

    expect(mirror.calls).toEqual([
      {
        pkValues: ['agent_proposal_mirror'],
        table: 'agent_proposals',
      },
      {
        pkValues: ['agent_proposal_mirror'],
        table: 'agent_proposals',
      },
    ])
  })

  test('lets no-token agents submit pending public-safe proposals', async () => {
    const store = new MemoryAgentProposalStore()
    const response = await runRoute(store, proposalRequest())
    const body = (await response.json()) as {
      authority: Record<string, boolean | string>
      proposal: { id: string; receiptRef: string; status: string }
    }

    expect(response.status).toBe(201)
    expect(response.headers.get('x-openagents-paid-recovery')).toBe(
      'planned_not_live',
    )
    expect(body.proposal).toMatchObject({
      id: 'agent_proposal_proposal-1',
      receiptRef:
        'agent_proposal_receipt_agent_proposal_proposal-1',
      status: 'pending',
    })
    expect(body.authority).toMatchObject({
      createsCustomerOrder: false,
      deploysSite: false,
      postsPublicly: false,
      sendsEmail: false,
      spendsMoney: false,
    })
  })

  test('replays duplicate idempotency keys instead of creating duplicates', async () => {
    const store = new MemoryAgentProposalStore()
    const first = await runRoute(store, proposalRequest('proposal-key-2'))
    const second = await runRoute(store, proposalRequest('proposal-key-2'), {
      makeUuid: () => 'proposal-2',
    })
    const body = (await second.json()) as {
      idempotentReplay: boolean
      proposal: { id: string }
    }

    expect(first.status).toBe(201)
    expect(second.status).toBe(200)
    expect(body.idempotentReplay).toBe(true)
    expect(body.proposal.id).toBe('agent_proposal_proposal-1')
    expect(store.proposals.size).toBe(1)
  })

  test('rate-limits new public proposals by client fingerprint', async () => {
    const store = new MemoryAgentProposalStore()
    await runRoute(store, proposalRequest('proposal-key-3'), {
      proposalRateLimit: 1,
    })
    const response = await runRoute(store, proposalRequest('proposal-key-4'), {
      makeUuid: () => 'proposal-2',
      proposalRateLimit: 1,
    })

    expect(response.status).toBe(429)
    expect(response.headers.get('ratelimit-limit')).toBe('1')
    expect(response.headers.get('retry-after')).toBe('3600')
    expect(response.headers.get('x-openagents-paid-recovery')).toBe('wait_only')
  })

  test('previews, redeems, and consumes one paid rate-limit recovery entitlement', async () => {
    const store = new MemoryAgentProposalStore()
    const recoveryStore = new MemoryAgentRateLimitRecoveryStore()
    const options = {
      agentLookup: recoveryAgentLookup(),
      agentToken,
      proposalRateLimit: 1,
      recoveryRuntime,
      recoveryStore,
    }
    await runRoute(store, proposalRequest('proposal-key-paid-1'), options)
    const preview = await runRoute(
      store,
      recoveryPreviewRequest('proposal-key-paid-2'),
      options,
    )
    const previewBody = (await preview.json()) as {
      challenge: {
        challengeId: string
        price: { amount: number; asset: string; denomination: string }
      }
      paidRecovery: string
    }

    expect(preview.status).toBe(200)
    expect(preview.headers.get('x-openagents-paid-recovery')).toBe(
      'available_l402',
    )
    expect(previewBody.challenge).toMatchObject({
      challengeId: 'agent_rate_limit_challenge_test',
      price: { amount: 100, asset: 'bitcoin', denomination: 'sats' },
    })
    expect(previewBody.paidRecovery).toBe('available_l402')

    const redeemRequest = new Request(
      'https://openagents.com/api/agents/proposals/rate-limit/redeem',
      {
        body: JSON.stringify({
          challengeId: previewBody.challenge.challengeId,
          l402ProofRef: 'mdk_rate_limit_payment_public_ref_1',
        }),
        headers: {
          authorization: agentAuthorization,
          'content-type': 'application/json',
          'idempotency-key': 'redeem-key-1',
        },
        method: 'POST',
      },
    )
    const firstRedeem = await runRoute(store, redeemRequest, options)
    const firstRedeemBody = (await firstRedeem.json()) as {
      entitlementRef: string
      receiptRef: string
      replayed: boolean
    }
    const replayRedeem = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/proposals/rate-limit/redeem',
        {
          body: JSON.stringify({
            challengeId: previewBody.challenge.challengeId,
            l402ProofRef: 'mdk_rate_limit_payment_public_ref_1',
          }),
          headers: {
            authorization: agentAuthorization,
            'content-type': 'application/json',
            'idempotency-key': 'redeem-key-1',
          },
          method: 'POST',
        },
      ),
      options,
    )
    const replayRedeemBody = (await replayRedeem.json()) as {
      receiptRef: string
      replayed: boolean
    }

    expect(firstRedeem.status).toBe(200)
    expect(firstRedeemBody).toMatchObject({
      receiptRef:
        'receipt.agent_rate_limit.agent_rate_limit_challenge_test',
      replayed: false,
    })
    expect(replayRedeem.status).toBe(200)
    expect(replayRedeemBody).toMatchObject({
      receiptRef: firstRedeemBody.receiptRef,
      replayed: true,
    })
    expect(recoveryStore.receipts.size).toBe(1)

    const paidSubmit = await runRoute(
      store,
      proposalRequest('proposal-key-paid-2', {
        authorization: agentAuthorization,
        entitlementRef: firstRedeemBody.entitlementRef,
      }),
      {
        ...options,
        makeUuid: () => 'proposal-paid-2',
      },
    )
    const paidSubmitBody = (await paidSubmit.json()) as {
      paidRecovery: { entitlementRef: string; receiptRef: string; status: string }
      proposal: { id: string }
    }
    const entitlement = recoveryStore.entitlements.get(
      firstRedeemBody.entitlementRef,
    )

    expect(paidSubmit.status).toBe(201)
    expect(paidSubmitBody.proposal.id).toBe(
      'agent_proposal_proposal-paid-2',
    )
    expect(paidSubmitBody.paidRecovery).toMatchObject({
      entitlementRef: firstRedeemBody.entitlementRef,
      receiptRef: firstRedeemBody.receiptRef,
      status: 'consumed',
    })
    expect(entitlement?.status).toBe('consumed')
    expect(store.proposals.size).toBe(2)

    const replayPaidSubmit = await runRoute(
      store,
      proposalRequest('proposal-key-paid-2', {
        authorization: agentAuthorization,
        entitlementRef: firstRedeemBody.entitlementRef,
      }),
      {
        ...options,
        makeUuid: () => 'proposal-paid-duplicate',
      },
    )
    const replayPaidSubmitBody = (await replayPaidSubmit.json()) as {
      idempotentReplay: boolean
      proposal: { id: string }
    }

    expect(replayPaidSubmit.status).toBe(200)
    expect(replayPaidSubmitBody.idempotentReplay).toBe(true)
    expect(replayPaidSubmitBody.proposal.id).toBe(
      'agent_proposal_proposal-paid-2',
    )
    expect(store.proposals.size).toBe(2)
  })

  test('rejects a paid recovery entitlement when the retried body changes', async () => {
    const store = new MemoryAgentProposalStore()
    const recoveryStore = new MemoryAgentRateLimitRecoveryStore()
    const options = {
      agentLookup: recoveryAgentLookup(),
      agentToken,
      proposalRateLimit: 1,
      recoveryRuntime,
      recoveryStore,
    }
    await runRoute(store, proposalRequest('proposal-key-mismatch-1'), options)
    const preview = await runRoute(
      store,
      recoveryPreviewRequest('proposal-key-mismatch-2'),
      options,
    )
    const previewBody = (await preview.json()) as {
      challenge: { challengeId: string }
    }
    const redeem = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/proposals/rate-limit/redeem',
        {
          body: JSON.stringify({
            challengeId: previewBody.challenge.challengeId,
            l402ProofRef: 'mdk_rate_limit_payment_public_ref_2',
          }),
          headers: {
            authorization: agentAuthorization,
            'content-type': 'application/json',
            'idempotency-key': 'redeem-key-2',
          },
          method: 'POST',
        },
      ),
      options,
    )
    const redeemBody = (await redeem.json()) as { entitlementRef: string }
    const mismatched = await runRoute(
      store,
      proposalRequest('proposal-key-mismatch-2', {
        authorization: agentAuthorization,
        entitlementRef: redeemBody.entitlementRef,
        title: 'Different proposal title',
      }),
      options,
    )
    const mismatchBody = (await mismatched.json()) as { error: string }

    expect(mismatched.status).toBe(403)
    expect(mismatchBody.error).toBe('agent_rate_limit_entitlement_not_found')
    expect(
      recoveryStore.entitlements.get(redeemBody.entitlementRef)?.status,
    ).toBe('active')
    expect(store.proposals.size).toBe(1)
  })

  test('public status read returns the pending proposal without authority', async () => {
    const store = new MemoryAgentProposalStore()
    await runRoute(store, proposalRequest())
    const response = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/agents/proposals/agent_proposal_proposal-1',
      ),
    )
    const body = (await response.json()) as {
      proposal: { status: string; title: string }
    }

    expect(response.status).toBe(200)
    expect(body.proposal.status).toBe('pending')
    expect(body.proposal.title).toBe('Add clearer OTEC evidence')
  })

  test('operators can inspect, promote, and reject proposals', async () => {
    const store = new MemoryAgentProposalStore()
    await runRoute(store, proposalRequest())
    const list = await runRoute(
      store,
      new Request('https://openagents.com/api/operator/agent-proposals'),
      { session: adminSession },
    )
    const promoted = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/operator/agent-proposals/agent_proposal_proposal-1/promote',
        {
          body: JSON.stringify({
            note: 'Ready for manual Site feedback review.',
            promotedTargetRef: 'site:otec',
            promotionKind: 'site_feedback',
          }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { session: adminSession },
    )
    const promotedBody = (await promoted.json()) as {
      proposal: { promotionKind: string; status: string }
    }

    expect(list.status).toBe(200)
    expect(list.headers.get('x-test-session-refreshed')).toBe('true')
    expect(promoted.status).toBe(200)
    expect(promotedBody.proposal.status).toBe('promoted')
    expect(promotedBody.proposal.promotionKind).toBe('site_feedback')

    await runRoute(store, proposalRequest('proposal-key-5'), {
      makeUuid: () => 'proposal-5',
    })
    const rejected = await runRoute(
      store,
      new Request(
        'https://openagents.com/api/operator/agent-proposals/agent_proposal_proposal-5/reject',
        {
          body: JSON.stringify({ reason: 'Not public-safe enough.' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      { session: adminSession },
    )
    const rejectedBody = (await rejected.json()) as {
      proposal: { operatorNote: string; status: string }
    }

    expect(rejected.status).toBe(200)
    expect(rejectedBody.proposal.status).toBe('rejected')
    expect(rejectedBody.proposal.operatorNote).toBe('Not public-safe enough.')
  })

  test('operator routes reject unauthenticated callers', async () => {
    const store = new MemoryAgentProposalStore()
    const response = await runRoute(
      store,
      new Request('https://openagents.com/api/operator/agent-proposals'),
    )

    expect(response.status).toBe(401)
  })
})
