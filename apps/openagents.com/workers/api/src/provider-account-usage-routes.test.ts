import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ProviderAccountUsageResponse,
  buildProviderAccountUsageProjection,
  makeProviderAccountUsageRoutes,
} from './provider-account-usage-routes'

const NOW = '2026-06-12T12:00:00.000Z'
const ADMIN_EMAIL = 'admin@openagents.com'
const OWNER_USER_ID = 'github:1001'
const LEASED_ACCOUNT_REF = 'provider-account_chatgpt_codex_alpha'
const UNATTRIBUTED_REF = 'provider-account://unattributed'

type FakeLedgerRow = {
  account_ref: string | null
  total_tokens: number
  window_total_tokens: number
  usage_events: number
}

type FakePoolRow = {
  provider_account_ref: string
  provider: string
  account_label: string | null
  operator_label: string | null
  status: string
  health: string
  low_credit_flag: number
  cooldown_until: string | null
}

class FakeUsageD1 {
  ledger: Array<FakeLedgerRow> = []
  pool: Array<FakePoolRow> = []

  asD1 = (): D1Database =>
    ({
      batch: (statements: Array<{ run: () => Promise<unknown> }>) =>
        Promise.all(statements.map(statement => statement.run())),
      prepare: (query: string) => this.statement(query),
    }) as unknown as D1Database

  private statement = (query: string, values: Array<unknown> = []) => ({
    bind: (...next: Array<unknown>) => this.statement(query, next),
    all: <T>(): Promise<{ results: Array<T> }> =>
      Promise.resolve({ results: this.all(query) as Array<T> }),
    first: <T>(): Promise<T | null> => Promise.resolve(null as T | null),
    run: () => Promise.resolve({ success: true }),
  })

  private all = (query: string): Array<unknown> => {
    if (query.includes('FROM token_usage_events events')) {
      return this.ledger.map(row => ({
        account_ref: row.account_ref,
        input_tokens: row.total_tokens,
        output_tokens: 0,
        reasoning_tokens: 0,
        cache_read_tokens: 0,
        cache_write_5m_tokens: 0,
        cache_write_1h_tokens: 0,
        total_tokens: row.total_tokens,
        window_total_tokens: row.window_total_tokens,
        usage_events: row.usage_events,
      }))
    }

    if (query.includes('FROM provider_accounts pa')) {
      return this.pool
    }

    return []
  }
}

const adminSessionDependencies = (db: FakeUsageD1) => ({
  appendRefreshedSessionCookies: (response: Response) => response,
  isOpenAgentsAdminEmail: (email: string) => email === ADMIN_EMAIL,
  nowIso: () => NOW,
  requireBrowserSession: () =>
    Promise.resolve({
      user: { email: ADMIN_EMAIL, userId: OWNER_USER_ID },
    }),
  openAgentsDatabase: db,
})

describe('buildProviderAccountUsageProjection', () => {
  test('joins ledger usage with pool state and surfaces unattributed bucket', async () => {
    const db = new FakeUsageD1()
    db.ledger = [
      {
        account_ref: LEASED_ACCOUNT_REF,
        total_tokens: 1000,
        window_total_tokens: 400,
        usage_events: 5,
      },
      {
        account_ref: UNATTRIBUTED_REF,
        total_tokens: 250,
        window_total_tokens: 100,
        usage_events: 2,
      },
    ]
    db.pool = [
      {
        provider_account_ref: LEASED_ACCOUNT_REF,
        provider: 'chatgpt_codex',
        account_label: 'Alpha',
        operator_label: null,
        status: 'connected',
        health: 'healthy',
        low_credit_flag: 1,
        cooldown_until: '2026-06-12T13:00:00.000Z',
      },
    ]

    const projection = await buildProviderAccountUsageProjection(db.asD1(), {
      now: NOW,
      userId: OWNER_USER_ID,
      windowSinceIso: null,
    })

    const attributed = projection.accounts.find(
      account => account.providerAccountRef === LEASED_ACCOUNT_REF,
    )
    const unattributed = projection.accounts.find(
      account => account.providerAccountRef === UNATTRIBUTED_REF,
    )

    expect(attributed?.attributed).toBe(true)
    expect(attributed?.poolKnown).toBe(true)
    expect(attributed?.lowCredit).toBe(true)
    expect(attributed?.cooldownActive).toBe(true)
    expect(attributed?.totals.totalTokens).toBe(1000)

    expect(unattributed?.attributed).toBe(false)
    expect(unattributed?.poolKnown).toBe(false)
    expect(unattributed?.lowCredit).toBeNull()

    expect(projection.summary.attributedAccounts).toBe(1)
    expect(projection.summary.unattributedTotalTokens).toBe(250)
    expect(projection.overBudgetEvents).toHaveLength(0)
    expect(projection.staleness.composition).toBe('live_at_read')
  })

  test('emits advisory over-budget events without enforcement authority', async () => {
    const db = new FakeUsageD1()
    db.ledger = [
      {
        account_ref: LEASED_ACCOUNT_REF,
        total_tokens: 5000,
        window_total_tokens: 1200,
        usage_events: 9,
      },
    ]
    db.pool = [
      {
        provider_account_ref: LEASED_ACCOUNT_REF,
        provider: 'chatgpt_codex',
        account_label: 'Alpha',
        operator_label: null,
        status: 'connected',
        health: 'healthy',
        low_credit_flag: 0,
        cooldown_until: null,
      },
    ]

    const projection = await buildProviderAccountUsageProjection(db.asD1(), {
      budgets: [
        {
          providerAccountRef: LEASED_ACCOUNT_REF,
          maxTotalTokens: 4000,
          maxWindowTokens: 2000,
        },
      ],
      now: NOW,
      userId: OWNER_USER_ID,
      windowSinceIso: '2026-06-12T00:00:00.000Z',
    })

    expect(projection.overBudgetEvents).toHaveLength(1)
    const event = projection.overBudgetEvents[0]
    expect(event?.field).toBe('budget.totalTokens')
    expect(event?.authority).toBe('advisory_event_only')
    expect(event?.overBy).toBe(1000)
    expect(event?.providerAccountRef).toBe(LEASED_ACCOUNT_REF)
  })
})

describe('makeProviderAccountUsageRoutes', () => {
  const buildRoutes = (db: FakeUsageD1) => {
    const deps = adminSessionDependencies(db)

    return makeProviderAccountUsageRoutes<
      { user: { email: string; userId: string } },
      { OPENAGENTS_DB: D1Database }
    >({
      appendRefreshedSessionCookies: deps.appendRefreshedSessionCookies,
      isOpenAgentsAdminEmail: deps.isOpenAgentsAdminEmail,
      nowIso: deps.nowIso,
      requireBrowserSession: deps.requireBrowserSession,
    })
  }

  test('returns the projection for an admin session', async () => {
    const db = new FakeUsageD1()
    db.ledger = [
      {
        account_ref: LEASED_ACCOUNT_REF,
        total_tokens: 10,
        window_total_tokens: 10,
        usage_events: 1,
      },
    ]
    db.pool = [
      {
        provider_account_ref: LEASED_ACCOUNT_REF,
        provider: 'chatgpt_codex',
        account_label: 'Alpha',
        operator_label: null,
        status: 'connected',
        health: 'healthy',
        low_credit_flag: 0,
        cooldown_until: null,
      },
    ]
    const routes = buildRoutes(db)
    const env = { OPENAGENTS_DB: db.asD1() }

    const response = await Effect.runPromise(
      routes.handleProviderAccountUsageApi(
        new Request('https://openagents.com/api/admin/provider-accounts/usage'),
        env,
        {} as ExecutionContext,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as ProviderAccountUsageResponse
    expect(body.accounts).toHaveLength(1)
    expect(body.policyVersion).toBeDefined()
    expect(body.generatedAt).toBe(NOW)
  })

  test('rejects a non-admin session with 403', async () => {
    const db = new FakeUsageD1()
    const routes = makeProviderAccountUsageRoutes<
      { user: { email: string; userId: string } },
      { OPENAGENTS_DB: D1Database }
    >({
      appendRefreshedSessionCookies: (response: Response) => response,
      isOpenAgentsAdminEmail: () => false,
      nowIso: () => NOW,
      requireBrowserSession: () =>
        Promise.resolve({
          user: { email: 'user@example.com', userId: 'github:2' },
        }),
    })
    const env = { OPENAGENTS_DB: db.asD1() }

    const response = await Effect.runPromise(
      routes.handleProviderAccountUsageApi(
        new Request('https://openagents.com/api/admin/provider-accounts/usage'),
        env,
        {} as ExecutionContext,
      ),
    )

    expect(response.status).toBe(403)
  })

  test('rejects a missing session with 401', async () => {
    const db = new FakeUsageD1()
    const routes = makeProviderAccountUsageRoutes<
      { user: { email: string; userId: string } },
      { OPENAGENTS_DB: D1Database }
    >({
      appendRefreshedSessionCookies: (response: Response) => response,
      isOpenAgentsAdminEmail: () => true,
      nowIso: () => NOW,
      requireBrowserSession: () => Promise.resolve(undefined),
    })
    const env = { OPENAGENTS_DB: db.asD1() }

    const response = await Effect.runPromise(
      routes.handleProviderAccountUsageApi(
        new Request('https://openagents.com/api/admin/provider-accounts/usage'),
        env,
        {} as ExecutionContext,
      ),
    )

    expect(response.status).toBe(401)
  })
})
