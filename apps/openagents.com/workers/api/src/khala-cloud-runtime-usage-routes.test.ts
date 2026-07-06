import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH,
  KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
  makeKhalaCloudRuntimeUsageRoutes,
} from './khala-cloud-runtime-usage-routes'
import type {
  TokenUsageIngestResult,
  TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-07-06T12:00:00.000Z'
const agentToken = 'oa_agent_khala_cloud_runtime_usage_test'
const agentUserId = 'agent-khala-cloud-runtime-1'

class MemoryAgentStore implements AgentRegistrationStore {
  constructor(
    private readonly tokenHash: string,
    private readonly openauthUserId: string | null = null,
  ) {}

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    if (tokenHash !== this.tokenHash) return Promise.resolve(undefined)
    return Promise.resolve({
      credentialId: 'credential-khala-cloud-runtime-1',
      openauthUserId: this.openauthUserId,
      profileMetadataJson: '{}',
      tokenPrefix: 'oa_agent_khal',
      user: {
        avatarUrl: null,
        createdAt: nowIso,
        displayName: 'Khala Cloud Runtime',
        id: agentUserId,
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: nowIso,
      },
    })
  }

  touchAgentCredential(
    _credentialId: string,
    _lastUsedAt: string,
  ): Promise<void> {
    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }
}

const makeLedger = () => {
  const events: Array<unknown> = []
  const unusedLedgerMethod = () => Effect.die('unused token usage ledger method')
  const ledger: TokenUsageLedgerShape = {
    ingestEvent: (body: unknown) => {
      events.push(body)
      return Effect.succeed({
        event: body,
        inserted: true,
      } as TokenUsageIngestResult)
    },
    readAggregates: unusedLedgerMethod,
    readInferenceAnalytics: unusedLedgerMethod,
    readLeaderboardPreference: unusedLedgerMethod,
    readLeaderboards: unusedLedgerMethod,
    readPublicTokensServed: unusedLedgerMethod,
    readPublicTokensServedChannelMix: unusedLedgerMethod,
    readPublicTokensServedDemandMix: unusedLedgerMethod,
    readPublicTokensServedHistory: unusedLedgerMethod,
    readPublicTokensServedModelMix: unusedLedgerMethod,
    updateLeaderboardPreference: unusedLedgerMethod,
  }
  return { events, ledger }
}

const body = (overrides: Record<string, unknown> = {}) => ({
  schemaVersion: KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
  lane: 'hosted_khala',
  model: 'gemini-3.5-flash',
  observedAt: nowIso,
  ownerUserId: 'user-owner-1',
  provider: 'vertex-gemini',
  pylonRef: 'pylon.org-cloud.1',
  runtimeEventId: 'event.runtime.usage.1',
  threadId: 'thread-1',
  turnId: 'turn-1',
  usage: {
    cacheReadInputTokens: 2,
    inputTokens: 10,
    outputTokens: 5,
    reasoningTokens: 3,
    totalTokens: 18,
    usageRef: 'usage.runtime.1',
  },
  ...overrides,
})

const post = (payload: unknown, token = agentToken) =>
  new Request(`https://openagents.com${KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH}`, {
    body: JSON.stringify(payload),
    headers: {
      Authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

describe('khala cloud runtime usage routes', () => {
  test('rejects missing agent bearer', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const { ledger } = makeLedger()
    const routes = makeKhalaCloudRuntimeUsageRoutes({
      agentStore: () => new MemoryAgentStore(tokenHash),
      ledger: () => ledger,
      nowIso: () => nowIso,
    })

    const response = await Effect.runPromise(
      routes.handleKhalaCloudRuntimeUsageIngestApi(
        new Request(`https://openagents.com${KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH}`, {
          body: JSON.stringify(body()),
          method: 'POST',
        }),
        {},
      ),
    )

    expect(response.status).toBe(401)
  })

  test('writes an exact external Khala mobile org-cloud token usage event', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const { events, ledger } = makeLedger()
    const routes = makeKhalaCloudRuntimeUsageRoutes({
      agentStore: () => new MemoryAgentStore(tokenHash),
      ledger: () => ledger,
      nowIso: () => nowIso,
    })

    const response = await Effect.runPromise(
      routes.handleKhalaCloudRuntimeUsageIngestApi(post(body()), {}),
    )
    const json = await response.json() as {
      insertedTokenUsage: boolean
      tokenUsageEventRef: string
      tokensServedDelta: number
    }

    expect(response.status).toBe(200)
    expect(json.insertedTokenUsage).toBe(true)
    expect(json.tokenUsageEventRef).toMatch(
      /^event\.inference\.served-tokens\.khala-cloud-runtime\./,
    )
    expect(json.tokensServedDelta).toBe(18)
    expect(events).toHaveLength(1)
    const event = events[0] as {
      actor: { accountRef: string; userId: string }
      demand: Record<string, unknown>
      provider: string
      model: string
      safeMetadata: Record<string, unknown>
      tokenCounts: Record<string, number>
      usageTruth: string
    }
    expect(event.actor).toEqual({
      accountRef: `agent:${agentUserId}`,
      userId: 'user-owner-1',
    })
    expect(event.demand).toMatchObject({
      demandChannel: 'khala_api',
      demandClient: 'khala-code-mobile',
      demandKind: 'external',
      demandSource: 'khala_mobile_org_cloud_runtime',
    })
    expect(event.provider).toBe('vertex-gemini')
    expect(event.model).toBe('gemini-3.5-flash')
    expect(event.safeMetadata).toMatchObject({
      executorMode: 'org_cloud',
      lane: 'hosted_khala',
      usageBasis: 'khala_runtime_usage_recorded',
    })
    expect(event.tokenCounts).toMatchObject({
      cacheReadTokens: 2,
      inputTokens: 10,
      outputTokens: 8,
      reasoningTokens: 3,
      totalTokens: 18,
    })
    expect(event.usageTruth).toBe('exact')
  })

  test('rejects linked user-pylon agents posting usage for a different owner', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const { ledger } = makeLedger()
    const routes = makeKhalaCloudRuntimeUsageRoutes({
      agentStore: () => new MemoryAgentStore(tokenHash, 'user-linked-1'),
      ledger: () => ledger,
      nowIso: () => nowIso,
    })

    const response = await Effect.runPromise(
      routes.handleKhalaCloudRuntimeUsageIngestApi(post(body()), {}),
    )
    const json = await response.json() as { error: string; reason: string }

    expect(response.status).toBe(403)
    expect(json.error).toBe('khala_cloud_runtime_forbidden')
    expect(json.reason).toContain('may only post runtime usage')
  })

  test('rejects zero-token usage because charges must come from exact receipts', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const { ledger } = makeLedger()
    const routes = makeKhalaCloudRuntimeUsageRoutes({
      agentStore: () => new MemoryAgentStore(tokenHash),
      ledger: () => ledger,
      nowIso: () => nowIso,
    })

    const response = await Effect.runPromise(
      routes.handleKhalaCloudRuntimeUsageIngestApi(
        post(
          body({
            usage: {
              inputTokens: 0,
              outputTokens: 0,
              totalTokens: 0,
              usageRef: 'usage.zero',
            },
          }),
        ),
        {},
      ),
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({
      error: 'khala_cloud_runtime_validation_error',
    })
  })
})
