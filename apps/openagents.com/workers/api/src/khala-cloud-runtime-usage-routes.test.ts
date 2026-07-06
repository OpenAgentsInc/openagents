import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeLedgerMeteringHook } from './inference/metering-hook'
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
import { readAgentBalance } from './payments-ledger'
import type {
  TokenUsageIngestResult,
  TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-07-06T12:00:00.000Z'
const agentToken = 'oa_agent_khala_cloud_runtime_usage_test'
const agentUserId = 'agent-khala-cloud-runtime-1'
const ownerAccountRef = 'agent:user-owner-1'

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

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
    return { results }
  }

  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

const BILLING_SCHEMA = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  held_msat INTEGER NOT NULL DEFAULT 0,
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL,
  payer_ref TEXT NOT NULL,
  cost_msat INTEGER NOT NULL CHECK (cost_msat > 0),
  state TEXT NOT NULL,
  failure_reason TEXT,
  rung TEXT,
  context_ref TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  public_receipt_ref TEXT,
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL
);
CREATE TABLE pay_in_legs (
  id TEXT PRIMARY KEY,
  pay_in_id TEXT NOT NULL,
  direction TEXT NOT NULL,
  kind TEXT NOT NULL,
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);
`

const makeBillingDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(BILLING_SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const seedOwnerBalance = async (
  db: D1Database,
  balanceMsat: number,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
       VALUES (?, ?, ?, ?)`,
    )
    .bind(ownerAccountRef, balanceMsat, nowIso, nowIso)
    .run()
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
      accountRef: ownerAccountRef,
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

  test('charges the owner credit balance idempotently per turn, not per executor retry', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const { ledger } = makeLedger()
    const billingDb = makeBillingDb()
    await seedOwnerBalance(billingDb, 10_000)
    const routes = makeKhalaCloudRuntimeUsageRoutes({
      agentStore: () => new MemoryAgentStore(tokenHash),
      ledger: () => ledger,
      meteringHook: () =>
        makeLedgerMeteringHook({
          db: billingDb,
          nowIso: () => nowIso,
          usdToMsat: () => 4_000,
        }),
      nowIso: () => nowIso,
    })

    const first = await Effect.runPromise(
      routes.handleKhalaCloudRuntimeUsageIngestApi(post(body()), {}),
    )
    const retriedWithDifferentUsageRef = await Effect.runPromise(
      routes.handleKhalaCloudRuntimeUsageIngestApi(
        post(
          body({
            usage: {
              cacheReadInputTokens: 2,
              inputTokens: 10,
              outputTokens: 5,
              reasoningTokens: 3,
              totalTokens: 18,
              usageRef: 'usage.runtime.retry-ref',
            },
          }),
        ),
        {},
      ),
    )

    expect(first.status).toBe(200)
    expect(retriedWithDifferentUsageRef.status).toBe(200)
    const firstJson = await first.json() as {
      tokenChargeMetered: boolean
      tokenChargeReceiptRef: string | null
    }
    const secondJson = await retriedWithDifferentUsageRef.json() as {
      tokenChargeMetered: boolean
      tokenChargeReceiptRef: string | null
    }
    expect(firstJson.tokenChargeMetered).toBe(true)
    expect(secondJson.tokenChargeMetered).toBe(true)
    expect(secondJson.tokenChargeReceiptRef).toBe(firstJson.tokenChargeReceiptRef)
    const balance = await readAgentBalance(billingDb, ownerAccountRef)
    expect(balance?.availableMsat).toBe(6_000)
  })

  test('never lets a post-turn charge make the owner balance negative and publishes an insufficient-credit event', async () => {
    const tokenHash = await sha256Hex(agentToken)
    const { ledger } = makeLedger()
    const billingDb = makeBillingDb()
    await seedOwnerBalance(billingDb, 1_000)
    const publishedEvents: Array<unknown> = []
    const routes = makeKhalaCloudRuntimeUsageRoutes({
      agentStore: () => new MemoryAgentStore(tokenHash),
      ledger: () => ledger,
      meteringHook: () =>
        makeLedgerMeteringHook({
          db: billingDb,
          nowIso: () => nowIso,
          usdToMsat: () => 4_000,
        }),
      nowIso: () => nowIso,
      publishInsufficientCreditEvent: (_env, input) => {
        publishedEvents.push(input)
        return Effect.succeed({
          eventRef: 'event.khala_cloud_billing.insufficient_credit.turn-1',
          published: true,
        })
      },
    })

    const response = await Effect.runPromise(
      routes.handleKhalaCloudRuntimeUsageIngestApi(post(body()), {}),
    )
    const json = await response.json() as {
      insufficientCreditEventPublished: boolean
      tokenChargeFailureReason: string | null
      tokenChargeMetered: boolean
    }

    expect(response.status).toBe(200)
    expect(json.tokenChargeMetered).toBe(false)
    expect(json.tokenChargeFailureReason).toBe('insufficient_credit')
    expect(json.insufficientCreditEventPublished).toBe(true)
    expect(publishedEvents).toHaveLength(1)
    const balance = await readAgentBalance(billingDb, ownerAccountRef)
    expect(balance?.availableMsat).toBe(1_000)
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
