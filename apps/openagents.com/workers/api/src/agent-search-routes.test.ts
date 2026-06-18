import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_TOKEN_PREFIX,
  type AgentCredentialLookup,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID,
  AGENT_SEARCH_BASIC_RECOVERY_SCOPE_REF,
  AGENT_SEARCH_ENTITLEMENT_HEADER,
  type AgentSearchCacheEntry,
  type AgentSearchConsumedEntitlement,
  type AgentSearchMetricEvent,
  type AgentSearchQuotaEvent,
  type AgentSearchRequestRecord,
  type AgentSearchSourceCard,
  type AgentSearchStore,
} from './agent-search'
import {
  type AgentSearchEntitlementRecord,
  type AgentSearchPaymentChallengeRecord,
  type AgentSearchPaymentReceiptRecord,
  type AgentSearchPaymentRedemptionRecord,
  type AgentSearchPaymentStore,
} from './agent-search-payments'
import { makeAgentSearchRoutes } from './agent-search-routes'
import { type ExaClientShape, ExaConfigurationDisabled } from './exa'

const agentToken = `${AGENT_TOKEN_PREFIX}search-test-token`

const agentLookup = (): AgentCredentialLookup => ({
  credentialId: 'agent_credential_search',
  profileMetadataJson: '{}',
  tokenPrefix: `${AGENT_TOKEN_PREFIX}search`,
  user: {
    avatarUrl: null,
    createdAt: '2026-06-06T00:00:00.000Z',
    displayName: 'Search Test Agent',
    id: 'agent_search_user',
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: '2026-06-06T00:00:00.000Z',
  },
})

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

class MemoryAgentSearchStore implements AgentSearchStore {
  readonly cache = new Map<string, AgentSearchCacheEntry>()
  readonly metrics: Array<AgentSearchMetricEvent> = []
  readonly quotaEvents: Array<AgentSearchQuotaEvent> = []
  readonly requests = new Map<string, AgentSearchRequestRecord>()
  readonly sources: Array<AgentSearchSourceCard> = []
  readonly entitlements = new Map<
    string,
    AgentSearchConsumedEntitlement & {
      actorRef: string
      credentialId: string
      requestBodyDigest: string
      status: 'active' | 'consumed'
    }
  >()

  consumeEntitlement = async (input: {
    actorRef: string
    credentialId: string
    entitlementRef: string
    nowIso: string
    requestBodyDigest: string
  }) => {
    const entitlement = this.entitlements.get(input.entitlementRef)

    if (
      entitlement === undefined ||
      entitlement.actorRef !== input.actorRef ||
      entitlement.credentialId !== input.credentialId ||
      entitlement.requestBodyDigest !== input.requestBodyDigest ||
      entitlement.status !== 'active'
    ) {
      return undefined
    }

    entitlement.status = 'consumed'

    return {
      entitlementRef: entitlement.entitlementRef,
      productId: entitlement.productId,
      receiptRef: entitlement.receiptRef,
      scopeRef: entitlement.scopeRef,
    }
  }

  countProviderRequestsSince = async (sinceIso: string) =>
    this.quotaEvents.filter(
      event =>
        event.eventKind === 'provider_request' && event.createdAt >= sinceIso,
    ).length

  countQuotaEventsSince = async (input: {
    actorRef: string
    credentialId: string
    eventKind: AgentSearchQuotaEvent['eventKind']
    sinceIso: string
  }) =>
    this.quotaEvents.filter(
      event =>
        event.actorRef === input.actorRef &&
        event.credentialId === input.credentialId &&
        event.eventKind === input.eventKind &&
        event.createdAt >= input.sinceIso,
    ).length

  readFreshCache = async (cacheKey: string, nowIso: string) => {
    const entry = this.cache.get(cacheKey)

    return entry !== undefined && entry.expiresAt > nowIso ? entry : null
  }

  readRequestByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    Array.from(this.requests.values()).find(
      request => request.idempotencyKeyHash === idempotencyKeyHash,
    )

  recordMetric = async (event: AgentSearchMetricEvent) => {
    this.metrics.push(event)
  }

  recordSearch = async (input: {
    quotaEvents: ReadonlyArray<AgentSearchQuotaEvent>
    request: AgentSearchRequestRecord
    sources: ReadonlyArray<AgentSearchSourceCard>
  }) => {
    if (!this.requests.has(input.request.id)) {
      this.requests.set(input.request.id, input.request)
      this.sources.push(...input.sources)
      this.quotaEvents.push(...input.quotaEvents)
    }
  }

  storeCache = async (entry: AgentSearchCacheEntry) => {
    this.cache.set(entry.cacheKey, entry)
  }
}

class MemoryAgentSearchPaymentStore implements AgentSearchPaymentStore {
  readonly challenges = new Map<string, AgentSearchPaymentChallengeRecord>()
  readonly receipts = new Map<string, AgentSearchPaymentReceiptRecord>()
  readonly redemptions = new Map<string, AgentSearchPaymentRedemptionRecord>()

  constructor(private readonly searchStore: MemoryAgentSearchStore) {}

  createChallenge = async (challenge: AgentSearchPaymentChallengeRecord) => {
    if (!this.challenges.has(challenge.id)) {
      this.challenges.set(challenge.id, challenge)
    }
  }

  createRedemptionBundle = async (input: {
    entitlement: AgentSearchEntitlementRecord
    receipt: AgentSearchPaymentReceiptRecord
    redemption: AgentSearchPaymentRedemptionRecord
  }) => {
    if (!this.redemptions.has(input.redemption.challengeId)) {
      this.receipts.set(input.receipt.receiptRef, input.receipt)
      this.redemptions.set(input.redemption.challengeId, input.redemption)
      this.searchStore.entitlements.set(input.entitlement.entitlementRef, {
        actorRef: input.entitlement.actorRef,
        credentialId: input.entitlement.credentialId,
        entitlementRef: input.entitlement.entitlementRef,
        productId: input.entitlement.productId,
        receiptRef: input.entitlement.receiptRef,
        requestBodyDigest: input.entitlement.requestBodyDigest,
        scopeRef: input.entitlement.scopeRef,
        status: 'active',
      })
    }
  }

  readChallengeById = async (challengeId: string) =>
    this.challenges.get(challengeId)

  readChallengeByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    Array.from(this.challenges.values()).find(
      challenge => challenge.idempotencyKeyHash === idempotencyKeyHash,
    )

  readRedemptionByChallengeId = async (challengeId: string) =>
    this.redemptions.get(challengeId)
}

const minimalEnv = (overrides: Record<string, string | undefined> = {}) =>
  ({
    EXA_API_KEY: 'exa-test-key',
    GITHUB_CLIENT_ID: 'github-client',
    GITHUB_CLIENT_SECRET: 'github-secret',
    OPENAGENTS_APP_URL: 'https://openagents.com',
    OPENAUTH_CLIENT_ID: 'openauth-client',
    OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
    OPENAGENTS_DB: {} as D1Database,
    ...overrides,
  }) as const

const searchRequest = (
  body: Record<string, unknown>,
  input: Readonly<{
    entitlementRef?: string | undefined
    idempotencyKey?: string | undefined
    path?: string | undefined
    token?: string | undefined
  }> = {},
) => {
  const headers = new Headers({
    authorization: `Bearer ${input.token ?? agentToken}`,
    'content-type': 'application/json',
    'idempotency-key': input.idempotencyKey ?? 'search-key-1',
  })

  if (input.entitlementRef !== undefined) {
    headers.set(AGENT_SEARCH_ENTITLEMENT_HEADER, input.entitlementRef)
  }

  return new Request(
    `https://openagents.com${input.path ?? '/api/agents/search'}`,
    {
      body: JSON.stringify(body),
      headers,
      method: 'POST',
    },
  )
}

const makeFakeExaClient = (calls: Array<unknown>): ExaClientShape => ({
  getContents: () =>
    Effect.fail(
      new ExaConfigurationDisabled({
        reason: 'contents not used in agent search tests.',
      }),
    ),
  search: input => {
    calls.push(input)

    return Effect.succeed({
      costDollars: 0.012,
      requestId: 'exa_request_1',
      results: [
        {
          highlights: ['Public OTEC and SWAC source highlight.'],
          publishedDate: '2026-06-01',
          score: 0.91,
          title: 'OTEC public source',
          url: 'https://example.org/otec',
        },
      ],
    })
  },
})

const route = (
  store: MemoryAgentSearchStore,
  input: Readonly<{
    exaCalls?: Array<unknown> | undefined
    lookup?: AgentCredentialLookup | undefined
    paymentStore?: AgentSearchPaymentStore | undefined
    token?: string | undefined
  }> = {},
) => {
  const paymentStore =
    input.paymentStore ?? new MemoryAgentSearchPaymentStore(store)

  return makeAgentSearchRoutes({
    agentStore: () =>
      new MemoryAgentRegistrationStore({
        lookup: input.lookup ?? agentLookup(),
        token: input.token ?? agentToken,
      }),
    exaClient: () => makeFakeExaClient(input.exaCalls ?? []),
    makePaymentStore: () => paymentStore,
    makeStore: () => store,
  }).routeAgentSearchRequest
}

const runRoute = async (
  handler: ReturnType<typeof route>,
  request: Request,
) => {
  const response = handler(request, minimalEnv())

  if (response === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(response)
}

describe('agent hosted search routes', () => {
  test('requires a registered agent bearer token', async () => {
    const store = new MemoryAgentSearchStore()
    const response = await runRoute(
      route(store),
      new Request('https://openagents.com/api/agents/search', {
        body: JSON.stringify({ query: 'public source' }),
        headers: { 'idempotency-key': 'search-key-1' },
        method: 'POST',
      }),
    )

    expect(response.status).toBe(401)
  })

  test('rejects unsafe queries before calling Exa', async () => {
    const store = new MemoryAgentSearchStore()
    const exaCalls: Array<unknown> = []
    const response = await runRoute(
      route(store, { exaCalls }),
      searchRequest({ query: 'look up sk-test-secret in public pages' }),
    )
    const body = await response.json()

    expect(response.status).toBe(422)
    expect(body).toMatchObject({ error: 'unprocessable_entity' })
    expect(exaCalls).toHaveLength(0)
    expect(store.requests.size).toBe(0)
  })

  test('records a cache miss, replays idempotently, then serves cache hits', async () => {
    const store = new MemoryAgentSearchStore()
    const exaCalls: Array<unknown> = []
    const handler = route(store, { exaCalls })
    const first = await runRoute(
      handler,
      searchRequest({ numResults: 3, query: 'public OTEC SWAC evidence' }),
    )
    const firstBody = (await first.json()) as {
      search: { cache: string; id: string }
    }
    const replay = await runRoute(
      handler,
      searchRequest(
        { numResults: 3, query: 'public OTEC SWAC evidence' },
        { idempotencyKey: 'search-key-1' },
      ),
    )
    const replayBody = (await replay.json()) as {
      search: { cache: string; id: string }
    }
    const cached = await runRoute(
      handler,
      searchRequest(
        { numResults: 3, query: 'public OTEC SWAC evidence' },
        { idempotencyKey: 'search-key-2' },
      ),
    )
    const cachedBody = (await cached.json()) as {
      search: { cache: string; id: string }
    }

    expect(first.status).toBe(200)
    expect(firstBody.search.cache).toBe('miss')
    expect(replayBody.search.id).toBe(firstBody.search.id)
    expect(cachedBody.search.cache).toBe('hit')
    expect(exaCalls).toHaveLength(1)
    expect(store.requests.size).toBe(2)
    expect(
      store.quotaEvents.filter(event => event.eventKind === 'search_request'),
    ).toHaveLength(2)
    expect(
      store.quotaEvents.filter(event => event.eventKind === 'provider_request'),
    ).toHaveLength(1)
  })

  test('rate limits the free basic tier', async () => {
    const store = new MemoryAgentSearchStore()
    const now = new Date().toISOString()

    for (let index = 0; index < 5; index += 1) {
      store.quotaEvents.push({
        actorRef: 'agent:agent_search_user',
        createdAt: now,
        credentialId: 'agent_credential_search',
        entitlementRef: null,
        eventKind: 'search_request',
        id: `quota_${index}`,
        mode: 'basic',
        productId: null,
        units: 1,
      })
    }

    const response = await runRoute(
      route(store),
      searchRequest({ query: 'public source after quota' }),
    )
    const body = await response.json()

    expect(response.status).toBe(402)
    expect(body).toMatchObject({
      error: 'payment_required',
      previewHref: '/api/agents/search/payments/preview',
      requiredProductRefs: [AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID],
    })
  })

  test('previews and redeems a paid entitlement after free quota', async () => {
    const store = new MemoryAgentSearchStore()
    const paymentStore = new MemoryAgentSearchPaymentStore(store)
    const exaCalls: Array<unknown> = []
    const handler = route(store, { exaCalls, paymentStore })
    const now = new Date().toISOString()

    for (let index = 0; index < 5; index += 1) {
      store.quotaEvents.push({
        actorRef: 'agent:agent_search_user',
        createdAt: now,
        credentialId: 'agent_credential_search',
        entitlementRef: null,
        eventKind: 'search_request',
        id: `paid_quota_${index}`,
        mode: 'basic',
        productId: null,
        units: 1,
      })
    }

    const search = { numResults: 3, query: 'public paid search source' }
    const blocked = await runRoute(
      handler,
      searchRequest(search, { idempotencyKey: 'blocked-search-key' }),
    )

    expect(blocked.status).toBe(402)

    const preview = await runRoute(
      handler,
      searchRequest(
        {
          search,
          spendCap: {
            amountMinorUnits: 1,
            asset: 'credits',
            denomination: 'credit',
          },
        },
        {
          idempotencyKey: 'preview-key-1',
          path: '/api/agents/search/payments/preview',
        },
      ),
    )
    const previewBody = (await preview.json()) as {
      preview: {
        challenge: {
          id: string
          productId: string
          requestBodyDigest: string
        }
      }
    }

    expect(preview.status).toBe(200)
    expect(previewBody.preview.challenge.productId).toBe(
      AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID,
    )

    const redemption = await runRoute(
      handler,
      searchRequest(
        {
          challengeId: previewBody.preview.challenge.id,
          l402ProofRef: 'redacted_mdk_l402_ref_search_1',
        },
        {
          idempotencyKey: 'redeem-key-1',
          path: '/api/agents/search/payments/redeem',
        },
      ),
    )
    const redemptionBody = (await redemption.json()) as {
      redemption: {
        entitlement: {
          entitlementRef: string
          productId: string
          scopeRef: string
        }
        replayed: boolean
      }
    }

    expect(redemption.status).toBe(200)
    expect(redemptionBody.redemption.entitlement).toMatchObject({
      productId: AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID,
      scopeRef: AGENT_SEARCH_BASIC_RECOVERY_SCOPE_REF,
    })
    expect(redemptionBody.redemption.replayed).toBe(false)

    const paid = await runRoute(
      handler,
      searchRequest(search, {
        entitlementRef: redemptionBody.redemption.entitlement.entitlementRef,
        idempotencyKey: 'paid-search-key-1',
      }),
    )
    const paidBody = (await paid.json()) as {
      search: {
        charged: boolean
        payment: { state: string }
      }
    }

    expect(paid.status).toBe(200)
    expect(paidBody.search.charged).toBe(true)
    expect(paidBody.search.payment.state).toBe('paid_entitlement')
    expect(exaCalls).toHaveLength(1)
    expect(Array.from(store.requests.values()).at(-1)).toMatchObject({
      chargeState: 'paid_entitlement',
      entitlementRef: redemptionBody.redemption.entitlement.entitlementRef,
      productId: AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID,
    })
    expect(
      store.quotaEvents.filter(
        event =>
          event.entitlementRef ===
            redemptionBody.redemption.entitlement.entitlementRef &&
          event.productId === AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID,
      ),
    ).toHaveLength(2)

    const reused = await runRoute(
      handler,
      searchRequest(search, {
        entitlementRef: redemptionBody.redemption.entitlement.entitlementRef,
        idempotencyKey: 'paid-search-key-2',
      }),
    )
    const reusedBody = await reused.json()

    expect(reused.status).toBe(402)
    expect(reusedBody).toMatchObject({
      error: 'payment_required',
      requiredProductRefs: [AGENT_SEARCH_BASIC_RECOVERY_PRODUCT_ID],
    })
  })

  test('returns unavailable when Exa is not configured', async () => {
    const store = new MemoryAgentSearchStore()
    const handler = makeAgentSearchRoutes({
      agentStore: () =>
        new MemoryAgentRegistrationStore({
          lookup: agentLookup(),
          token: agentToken,
        }),
      makeStore: () => store,
    }).routeAgentSearchRequest
    const responseEffect = handler(
      searchRequest({ query: 'public source without exa' }),
      minimalEnv({ EXA_API_KEY: undefined }),
    )

    if (responseEffect === undefined) {
      throw new Error('route did not match')
    }

    const response = await Effect.runPromise(responseEffect)
    const body = await response.json()

    expect(response.status).toBe(503)
    expect(body).toMatchObject({ error: 'exa_disabled' })
  })
})
