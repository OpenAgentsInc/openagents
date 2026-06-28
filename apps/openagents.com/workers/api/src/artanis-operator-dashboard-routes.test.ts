import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOperatorArtanisDashboardRoutes } from './artanis-operator-dashboard-routes'

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

const makeFakeD1 = (): D1Database =>
  ({
    prepare: (sql: string) => ({
      bind: (...values: ReadonlyArray<unknown>) => ({
        all: async () => query(sql, values),
      }),
      all: async () => query(sql, []),
    }),
  }) as unknown as D1Database

const query = (sql: string, values: ReadonlyArray<unknown>) => {
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
})
