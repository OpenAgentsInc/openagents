import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeOperatorArtanisDashboardRoutes,
  operatorAccountUsageProjection,
} from './artanis-operator-dashboard-routes'

const executionContext = {} as ExecutionContext

type ThreadRow = Readonly<{
  thread_ref: string
  caller_id: string
  caller_kind: string
  subject_agent_ref: string
  subject_agent_kind: string
  title: string
  status: string
  source_ref: string | null
  last_message_at: string
  created_at: string
  updated_at: string
}>

type MessageRow = Readonly<{
  message_ref: string
  thread_ref: string
  caller_id: string
  author_id: string
  author_kind: string
  body: string
  created_at: string
}>

type AccountRow = {
  provider_account_ref: string
  provider: string
  cooldown_until: string | null
  recent_failure_class: string | null
  user_id: string
  deleted_at: string | null
}

const threads: ReadonlyArray<ThreadRow> = [
  {
    caller_id: 'github:14167547',
    caller_kind: 'owner',
    created_at: '2026-06-27T16:00:00.000Z',
    last_message_at: '2026-06-27T16:10:00.000Z',
    source_ref: null,
    status: 'open',
    subject_agent_kind: 'codex',
    subject_agent_ref: 'codex-4',
    thread_ref: 'artanis_thread_owner_codex_1',
    title: 'Codex burn review',
    updated_at: '2026-06-27T16:10:00.000Z',
  },
  {
    caller_id: 'claude:worker-2',
    caller_kind: 'agent',
    created_at: '2026-06-27T15:00:00.000Z',
    last_message_at: '2026-06-27T15:05:00.000Z',
    source_ref: 'agent.public.claude.worker_2',
    status: 'open',
    subject_agent_kind: 'artanis',
    subject_agent_ref: 'agent_artanis',
    thread_ref: 'artanis_thread_claude_2',
    title: 'Claude needs steering',
    updated_at: '2026-06-27T15:05:00.000Z',
  },
]

const messages: ReadonlyArray<MessageRow> = [
  {
    author_id: 'github:14167547',
    author_kind: 'owner',
    body: 'Give me the Codex fleet status.',
    caller_id: 'github:14167547',
    created_at: '2026-06-27T16:00:00.000Z',
    message_ref: 'artanis_message_owner_1',
    thread_ref: 'artanis_thread_owner_codex_1',
  },
  {
    author_id: 'agent_artanis',
    author_kind: 'agent',
    body: 'Two Codex assignments are proof-ready.',
    caller_id: 'github:14167547',
    created_at: '2026-06-27T16:10:00.000Z',
    message_ref: 'artanis_message_artanis_1',
    thread_ref: 'artanis_thread_owner_codex_1',
  },
  {
    author_id: 'claude:worker-2',
    author_kind: 'agent',
    body: 'Need a decision on the forum reply.',
    caller_id: 'claude:worker-2',
    created_at: '2026-06-27T15:05:00.000Z',
    message_ref: 'artanis_message_claude_1',
    thread_ref: 'artanis_thread_claude_2',
  },
]

const accountRows = (): Array<AccountRow> => [
  {
    cooldown_until: '2099-01-01T00:00:00.000Z',
    deleted_at: null,
    provider: 'chatgpt_codex',
    provider_account_ref: 'acct_hash_codex_1',
    recent_failure_class: 'rate_limited',
    user_id: 'github:operator',
  },
  {
    cooldown_until: null,
    deleted_at: null,
    provider: 'claude',
    provider_account_ref: 'acct_hash_claude_1',
    recent_failure_class: null,
    user_id: 'github:operator',
  },
  {
    cooldown_until: '2099-01-01T00:00:00.000Z',
    deleted_at: null,
    provider: 'chatgpt_codex',
    provider_account_ref: 'acct_hash_other',
    recent_failure_class: 'rate_limited',
    user_id: 'github:other',
  },
]

const makeFakeD1 = (accounts: Array<AccountRow> = accountRows()): D1Database =>
  ({
    prepare: (sql: string) => ({
      bind: (...values: ReadonlyArray<unknown>) => ({
        all: async () => query(sql, values, accounts),
        run: async () => run(sql, values, accounts),
      }),
      all: async () => query(sql, [], accounts),
    }),
  }) as unknown as D1Database

const query = (
  sql: string,
  values: ReadonlyArray<unknown>,
  accounts: Array<AccountRow>,
) => {
  if (sql.includes('FROM provider_accounts')) {
    const userId = typeof values[0] === 'string' ? values[0] : ''

    return {
      results: accounts
        .filter(
          account =>
            account.user_id === userId &&
            account.deleted_at === null &&
            (account.provider === 'chatgpt_codex' ||
              account.provider === 'claude'),
        )
        .map(account => ({
          cooldown_until: account.cooldown_until,
          provider: account.provider,
          provider_account_ref: account.provider_account_ref,
          recent_failure_class: account.recent_failure_class,
        })),
    }
  }

  if (sql.includes('FROM artanis_threads')) {
    const callerId = typeof values[0] === 'string' ? values[0] : undefined
    const rows = threads
      .filter(thread => callerId === undefined || thread.caller_id === callerId)
      .map(thread => ({
        ...thread,
        message_count: messages.filter(
          message => message.thread_ref === thread.thread_ref,
        ).length,
      }))
      .sort((left, right) =>
        right.last_message_at.localeCompare(left.last_message_at),
      )

    return { results: rows }
  }

  const threadRef = typeof values[0] === 'string' ? values[0] : ''
  return {
    results: messages.filter(message => message.thread_ref === threadRef),
  }
}

const run = (
  sql: string,
  values: ReadonlyArray<unknown>,
  accounts: Array<AccountRow>,
) => {
  if (sql.includes('UPDATE provider_accounts')) {
    const [, userId, accountRefHash] = values as [string, string, string]
    const account = accounts.find(
      candidate =>
        candidate.user_id === userId &&
        candidate.provider_account_ref === accountRefHash &&
        candidate.deleted_at === null,
    )

    if (account === undefined) {
      return { meta: { changes: 0 }, success: true }
    }

    account.cooldown_until = null
    account.recent_failure_class = null

    return { meta: { changes: 1 }, success: true }
  }

  return { meta: { changes: 0 }, success: true }
}

const route = (options: {
  readonly adminToken?: boolean
  readonly browserEmail?: string | undefined
}) =>
  makeOperatorArtanisDashboardRoutes({
    appendRefreshedSessionCookies: response => response,
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    requireAdminApiToken: request =>
      Promise.resolve(
        options.adminToken === true &&
          request.headers.get('authorization') === 'Bearer admin',
      ),
    requireBrowserSession: () =>
      Promise.resolve(
        options.browserEmail === undefined
          ? undefined
          : {
              user: {
                email: options.browserEmail,
                userId: 'github:operator',
              },
            },
      ),
  }).routeOperatorArtanisDashboardRequest

const request = (path = '/api/operator/artanis/dashboard') =>
  new Request(`https://openagents.com${path}`, {
    headers: { authorization: 'Bearer admin' },
  })

describe('Artanis operator dashboard routes', () => {
  test('requires owner/admin authority before exposing chat transcripts', async () => {
    const anonymous = await Effect.runPromise(
      route({})(request(), { OPENAGENTS_DB: makeFakeD1() }, executionContext)!,
    )
    const nonAdmin = await Effect.runPromise(
      route({ browserEmail: 'user@example.com' })(
        request(),
        { OPENAGENTS_DB: makeFakeD1() },
        executionContext,
      )!,
    )

    expect(anonymous.status).toBe(401)
    expect(nonAdmin.status).toBe(403)
  })

  test('lists threads and opens the selected markdown conversation', async () => {
    const response = await Effect.runPromise(
      route({ adminToken: true })(
        request(
          '/api/operator/artanis/dashboard?thread_ref=artanis_thread_owner_codex_1',
        ),
        { OPENAGENTS_DB: makeFakeD1() },
        executionContext,
      )!,
    )
    const body = await response.json() as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      dashboardRef: 'operator.artanis.dashboard',
      accountUsage: {
        accounts: [],
      },
      selectedThread: {
        callerId: 'github:14167547',
        messageCount: 2,
        threadRef: 'artanis_thread_owner_codex_1',
      },
      messages: [
        {
          authorKind: 'owner',
          body: 'Give me the Codex fleet status.',
        },
        {
          authorKind: 'agent',
          body: 'Two Codex assignments are proof-ready.',
        },
      ],
    })
    expect((body.threads as ReadonlyArray<unknown>).length).toBe(2)
  })

  test('filters thread inventory by caller_id', async () => {
    const response = await Effect.runPromise(
      route({ adminToken: true })(
        request('/api/operator/artanis/dashboard?caller_id=claude%3Aworker-2'),
        { OPENAGENTS_DB: makeFakeD1() },
        executionContext,
      )!,
    )
    const body = await response.json() as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      callerIdFilter: 'claude:worker-2',
      selectedThread: {
        callerId: 'claude:worker-2',
        threadRef: 'artanis_thread_claude_2',
      },
      threads: [
        {
          callerId: 'claude:worker-2',
          title: 'Claude needs steering',
        },
      ],
    })
  })

  test('serves owner-only account status rows for the Artanis accounts page', async () => {
    const response = await Effect.runPromise(
      route({ browserEmail: 'chris@openagents.com' })(
        request('/api/operator/accounts/status'),
        { OPENAGENTS_DB: makeFakeD1() },
        executionContext,
      )!,
    )
    const body = await response.json() as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      accounts: [
        {
          accountRefHash: 'acct_hash_codex_1',
          cooldownExpiresAt: '2099-01-01T00:00:00.000Z',
          isRateLimited: true,
          provider: 'codex',
        },
        {
          accountRefHash: 'acct_hash_claude_1',
          cooldownExpiresAt: null,
          isRateLimited: false,
          provider: 'claude',
        },
      ],
    })
  })

  test('manual reset clears the selected owner account cooldown', async () => {
    const accounts = accountRows()
    const response = await Effect.runPromise(
      route({ browserEmail: 'chris@openagents.com' })(
        new Request('https://openagents.com/api/operator/accounts/reset', {
          body: JSON.stringify({ accountRefHash: 'acct_hash_codex_1' }),
          headers: { authorization: 'Bearer admin' },
          method: 'POST',
        }),
        { OPENAGENTS_DB: makeFakeD1(accounts) },
        executionContext,
      )!,
    )
    const body = await response.json() as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      ok: true,
      status: {
        accounts: expect.arrayContaining([
          expect.objectContaining({
            accountRefHash: 'acct_hash_codex_1',
            cooldownExpiresAt: null,
            isRateLimited: false,
          }),
        ]),
      },
    })
    expect(accounts[0]).toMatchObject({
      cooldown_until: null,
      recent_failure_class: null,
    })
    expect(accounts[2]).toMatchObject({
      cooldown_until: '2099-01-01T00:00:00.000Z',
      recent_failure_class: 'rate_limited',
    })
  })

  test('projects bounded hourly and weekly token usage windows for dashboard meters', () => {
    const projection = operatorAccountUsageProjection(
      [
        {
          accountRefHash: 'acct_hash_codex_1',
          cooldownExpiresAt: '2026-06-28T02:00:00.000Z',
          hourlyCap: 1_000,
          hourlyUsage: 250,
          isRateLimited: true,
          manualResetsRemaining: 2,
          provider: 'codex',
          weeklyCap: 10_000,
          weeklyUsage: 12_000,
        },
      ],
      '2026-06-28T01:05:00.000Z',
    )

    expect(projection).toMatchObject({
      observedAt: '2026-06-28T01:05:00.000Z',
      accounts: [
        {
          accountRefHash: 'acct_hash_codex_1',
          isRateLimited: true,
          windows: [
            {
              cap: 1_000,
              label: 'hourly',
              percentUsed: 25,
              remaining: 750,
              used: 250,
            },
            {
              cap: 10_000,
              label: 'weekly',
              percentUsed: 100,
              remaining: 0,
              used: 12_000,
            },
          ],
        },
      ],
    })
  })
})
