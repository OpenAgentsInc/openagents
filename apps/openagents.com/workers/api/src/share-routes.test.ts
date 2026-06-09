import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeShareRoutes } from './share-routes'

type QueryBinding = Readonly<{
  query: string
  values: ReadonlyArray<unknown>
}>

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    name: string
    userId: string
  }>
}>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const messageRow = {
  agent_run_id: null,
  author_avatar_url: 'https://avatars.githubusercontent.com/u/14167547?v=4',
  author_github_username: 'AtlantisPleb',
  author_name: 'Christopher David',
  author_user_id: 'github:14167547',
  autopilot_thread_id: null,
  body: 'Share API route smoke',
  created_at: '2026-06-05T00:00:00.000Z',
  id: 'team_chat_1',
  kind: 'message' as const,
  metadata_json: null,
  project_id: 'project_artanis',
  team_id: 'team_openagents_core',
}

const targetUser = {
  displayName: 'Christopher David',
  email: 'chris@openagents.com',
  githubUsername: 'AtlantisPleb',
  userId: 'github:14167547',
}

const makeScriptedD1 = (
  activeMemberIds: ReadonlyArray<string> = [targetUser.userId],
): Readonly<{
  bindings: Array<QueryBinding>
  db: D1Database
  insertedShare: () =>
    | Readonly<{
        audienceJson: string
        canonicalUrl: string
        createdAt: string
        id: string
        ownerUserId: string
        projectId: string | null
        projectionJson: string
        redactionPolicyId: string
        sourceId: string
        sourceKind: string
        summary: string | null
        teamId: string | null
        title: string
        updatedAt: string
      }>
    | undefined
}> => {
  const bindings: Array<QueryBinding> = []
  let share:
    | Readonly<{
        audienceJson: string
        canonicalUrl: string
        createdAt: string
        id: string
        ownerUserId: string
        projectId: string | null
        projectionJson: string
        redactionPolicyId: string
        sourceId: string
        sourceKind: string
        summary: string | null
        teamId: string | null
        title: string
        updatedAt: string
      }>
    | undefined

  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => {
      let values: ReadonlyArray<unknown> = []

      function raw<T = unknown[]>(options: {
        columnNames: true
      }): Promise<[Array<string>, ...Array<T>]>
      function raw<T = unknown[]>(options?: {
        columnNames?: false
      }): Promise<Array<T>>
      function raw<T = unknown[]>(options?: {
        columnNames?: boolean
      }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
        return options?.columnNames === true
          ? Promise.resolve([[]])
          : Promise.resolve([])
      }

      const statement: D1PreparedStatement = {
        all: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          return Promise.resolve(
            makeResult<T>(
              query.includes('FROM team_chat_messages')
                ? [jsonFixture<T>(messageRow)]
                : [],
            ),
          )
        },
        bind: (...nextValues: ReadonlyArray<unknown>) => {
          values = nextValues

          return statement
        },
        first: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          if (query.includes('FROM team_memberships')) {
            return Promise.resolve(
              activeMemberIds.includes(String(values[1]))
                ? jsonFixture<T>({ role: 'owner' })
                : null,
            )
          }

          if (query.includes('FROM share_projections') && share !== undefined) {
            return Promise.resolve(
              jsonFixture<T>({
                audience_json: share.audienceJson,
                canonical_url: share.canonicalUrl,
                created_at: share.createdAt,
                expires_at: null,
                id: share.id,
                owner_user_id: share.ownerUserId,
                project_id: share.projectId,
                projection_json: share.projectionJson,
                redaction_policy_id: share.redactionPolicyId,
                revoked_at: null,
                source_id: share.sourceId,
                source_kind: share.sourceKind,
                status: 'active',
                summary: share.summary,
                team_id: share.teamId,
                title: share.title,
                updated_at: share.updatedAt,
              }),
            )
          }

          return Promise.resolve(null)
        },
        raw,
        run: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          if (query.includes('INSERT INTO share_projections')) {
            share = {
              audienceJson: String(values[7]),
              canonicalUrl: String(values[1]),
              createdAt: String(values[13]),
              id: String(values[0]),
              ownerUserId: String(values[4]),
              projectId: values[6] === null ? null : String(values[6]),
              projectionJson: String(values[11]),
              redactionPolicyId: String(values[12]),
              sourceId: String(values[3]),
              sourceKind: String(values[2]),
              summary: values[9] === null ? null : String(values[9]),
              teamId: values[5] === null ? null : String(values[5]),
              title: String(values[8]),
              updatedAt: String(values[14]),
            }
          }

          return Promise.resolve(makeResult<T>())
        },
      }

      return statement
    },
    withSession: () => ({
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      getBookmark: () => null,
      prepare: query => db.prepare(query),
    }),
  }

  return { bindings, db, insertedShare: () => share }
}

const routeRequest = async (
  input: Readonly<{
    adminToken?: string
    agentToken?: string
    body: Record<string, unknown>
    db: D1Database
    targetSelectors?: Array<Record<string, unknown>>
  }>,
): Promise<Response> => {
  const targetSelectors = input.targetSelectors ?? []
  const routes = makeShareRoutes<TestSession>({
    appendRefreshedSessionCookies: response => response,
    appOrigin: () => 'https://openagents.com',
    authenticateRequestActor: request =>
      Promise.resolve(
        request.headers.get('authorization') === `Bearer ${input.agentToken}`
          ? {
              kind: 'agent' as const,
              agent: {
                user: {
                  displayName: 'Auth Totals Check Agent',
                  id: 'user_agent_1',
                  primaryEmail: null,
                },
              },
            }
          : undefined,
      ),
    isAdminEmail: email => email === 'chris@openagents.com',
    readSelectedOperatorTargetUser: (_db, selector) => {
      targetSelectors.push(selector)

      return Promise.resolve(targetUser)
    },
    requireAdminApiToken: request =>
      Promise.resolve(
        input.adminToken !== undefined &&
          request.headers.get('authorization') === `Bearer ${input.adminToken}`,
      ),
    requireBrowserSession: () => Promise.resolve(undefined),
  })
  const effect = routes.routeShareRequest(
    new Request('https://openagents.com/api/share', {
      body: JSON.stringify(input.body),
      headers: {
        authorization: `Bearer ${input.adminToken ?? input.agentToken ?? 'none'}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    }),
    { OPENAGENTS_DB: input.db } as never,
    {} as ExecutionContext,
  )

  if (effect === undefined) {
    throw new Error('Expected share route to handle /api/share.')
  }

  return Effect.runPromise(effect)
}

describe('share routes', () => {
  test('creates a team project share through the admin API token for a selected target user', async () => {
    const { bindings, db, insertedShare } = makeScriptedD1()
    const targetSelectors: Array<Record<string, unknown>> = []
    const response = await routeRequest({
      adminToken: 'admin-token',
      body: {
        audience: { _tag: 'Public' },
        email: 'chris@openagents.com',
        source: {
          id: 'project_artanis',
          kind: 'team-project-thread',
          projectId: 'project_artanis',
          teamId: 'team_openagents_core',
        },
        title: 'Admin API share',
      },
      db,
      targetSelectors,
    })
    const body = await response.json<{
      audienceLabel: string
      status: string
      url: string
    }>()
    const stored = insertedShare()

    expect(response.status).toBe(201)
    expect(body.audienceLabel).toBe('Shared publicly')
    expect(body.status).toBe('active')
    expect(body.url).toMatch(
      /^https:\/\/openagents\.com\/share\/[0-9a-f-]{36}$/,
    )
    expect(targetSelectors[0]?.email).toBe('chris@openagents.com')
    expect(stored?.ownerUserId).toBe('github:14167547')
    expect(stored?.sourceKind).toBe('team-project-thread')
    expect(stored?.sourceId).toBe('project_artanis')
    expect(stored?.teamId).toBe('team_openagents_core')
    expect(stored?.projectId).toBe('project_artanis')
    expect(
      bindings.some(
        binding =>
          binding.query.includes('FROM team_memberships') &&
          binding.values[0] === 'team_openagents_core' &&
          binding.values[1] === 'github:14167547',
      ),
    ).toBe(true)
    expect(
      JSON.parse(stored?.projectionJson ?? '{}').messages[0]?.avatarUrl,
    ).toBe('https://avatars.githubusercontent.com/u/14167547?v=4')
  })

  test('creates a share through a programmatic agent token as the agent user', async () => {
    const { db, insertedShare } = makeScriptedD1(['user_agent_1'])
    const targetSelectors: Array<Record<string, unknown>> = []
    const response = await routeRequest({
      agentToken: 'agent-token',
      body: {
        audience: { _tag: 'Public' },
        source: {
          id: 'team_openagents_core',
          kind: 'team-thread',
          teamId: 'team_openagents_core',
        },
        title: 'Agent API share',
      },
      db,
      targetSelectors,
    })

    expect(response.status).toBe(201)
    expect(targetSelectors).toEqual([])
    expect(insertedShare()?.ownerUserId).toBe('user_agent_1')
  })

  test('requires an explicit target user selector for admin-created shares', async () => {
    const { db } = makeScriptedD1()
    const response = await routeRequest({
      adminToken: 'admin-token',
      body: {
        audience: { _tag: 'Public' },
        source: {
          id: 'project_artanis',
          kind: 'team-project-thread',
          projectId: 'project_artanis',
          teamId: 'team_openagents_core',
        },
      },
      db,
    })

    await expect(response.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason:
        'admin share creation requires email, login, githubLogin, or userId',
    })
    expect(response.status).toBe(400)
  })
})
