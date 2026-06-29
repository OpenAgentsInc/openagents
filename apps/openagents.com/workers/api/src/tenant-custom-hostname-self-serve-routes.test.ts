import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type TeamRole } from './team-repository'
import { makeTenantHostnameSelfServeRoutes } from './tenant-custom-hostname-self-serve-routes'

type StoredHostname = {
  id: string
  team_id: string
  hostname: string
  status: string
  verification_token: string
  verified_at: string | null
  created_at: string
  updated_at: string
}

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
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

const textValue = (value: unknown): string => String(value)
const nullableText = (value: unknown): string | null =>
  value === null || value === undefined ? null : String(value)

const makeMemoryD1 = (rows: Array<StoredHostname> = []): D1Database => {
  const prepare = (query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []

    const statement: D1PreparedStatement = {
      bind: (...values: Array<unknown>) => {
        bound = values

        return statement
      },
      first: async <T>() => {
        if (query.includes('SELECT') && query.includes('WHERE hostname = ?')) {
          const hostname = textValue(bound[0])
          const row = rows.find(stored => stored.hostname === hostname)

          return (row === undefined ? null : (row as unknown as T)) as T
        }

        return null as unknown as T
      },
      run: async () => {
        if (query.includes('INSERT INTO tenant_custom_hostnames')) {
          const hostname = textValue(bound[2])

          if (rows.some(stored => stored.hostname === hostname)) {
            throw new Error('UNIQUE constraint failed')
          }

          rows.push({
            id: textValue(bound[0]),
            team_id: textValue(bound[1]),
            hostname,
            status: textValue(bound[3]),
            verification_token: textValue(bound[4]),
            verified_at: nullableText(bound[5]),
            created_at: textValue(bound[6]),
            updated_at: textValue(bound[7]),
          })

          return makeResult()
        }

        return makeResult()
      },
      all: async <T>() => {
        if (query.includes('SELECT') && query.includes('WHERE team_id = ?')) {
          const teamId = textValue(bound[0])

          return makeResult<T>(
            rows.filter(r => r.team_id === teamId) as unknown as Array<T>,
          )
        }

        return makeResult<T>([])
      },
      raw: async () => [] as unknown as never,
    } as unknown as D1PreparedStatement

    return statement
  }

  return {
    prepare,
    batch: async () => [],
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    withSession: () => {
      throw new Error('not implemented')
    },
  } as unknown as D1Database
}

type Session = Readonly<{ user: Readonly<{ userId: string }> }>
type Bindings = Readonly<{ db: D1Database }>

const ctx = {} as ExecutionContext

const makeRoutes = (params: {
  db: D1Database
  session?: Session
  role?: TeamRole
}) =>
  makeTenantHostnameSelfServeRoutes<Session, Bindings>({
    database: (env: Bindings) => env.db,
    requireBrowserSession: async () => params.session,
    readTeamRole: async () => params.role,
  })

const run = (effect: Effect.Effect<Response> | undefined) => {
  if (effect === undefined) {
    throw new Error('route did not apply')
  }

  return Effect.runPromise(effect)
}

describe('tenant-custom-hostname-self-serve-routes', () => {
  test('GET without a session is 401', async () => {
    const db = makeMemoryD1()
    const routes = makeRoutes({ db })
    const response = await run(
      routes.routeTenantHostnameSelfServeRequest(
        new Request('https://openagents.com/api/tenant/hostnames?teamId=team_a'),
        { db },
        ctx,
      ),
    )

    expect(response.status).toBe(401)
  })

  test('GET without team membership is 403', async () => {
    const db = makeMemoryD1()
    // No `role` supplied => readTeamRole resolves undefined => no membership.
    const routes = makeRoutes({
      db,
      session: { user: { userId: 'user_1' } },
    })
    const response = await run(
      routes.routeTenantHostnameSelfServeRequest(
        new Request('https://openagents.com/api/tenant/hostnames?teamId=team_a'),
        { db },
        ctx,
      ),
    )

    expect(response.status).toBe(403)
  })

  test('GET lists a member team hostnames', async () => {
    const db = makeMemoryD1([
      {
        id: 'h1',
        team_id: 'team_a',
        hostname: 'brand.example.com',
        status: 'pending',
        verification_token: 'tok',
        verified_at: null,
        created_at: '2026-01-01T00:00:00.000Z',
        updated_at: '2026-01-01T00:00:00.000Z',
      },
    ])
    const routes = makeRoutes({
      db,
      session: { user: { userId: 'user_1' } },
      role: 'viewer',
    })
    const response = await run(
      routes.routeTenantHostnameSelfServeRequest(
        new Request('https://openagents.com/api/tenant/hostnames?teamId=team_a'),
        { db },
        ctx,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      teamId: string
      hostnames: ReadonlyArray<{ hostname: string; servingLive: boolean }>
    }
    expect(body.teamId).toBe('team_a')
    expect(body.hostnames[0]?.hostname).toBe('brand.example.com')
    expect(body.hostnames[0]?.servingLive).toBe(false)
  })

  test('POST claim as a viewer is 403 (insufficient role)', async () => {
    const db = makeMemoryD1()
    const routes = makeRoutes({
      db,
      session: { user: { userId: 'user_1' } },
      role: 'viewer',
    })
    const response = await run(
      routes.routeTenantHostnameSelfServeRequest(
        new Request('https://openagents.com/api/tenant/hostnames', {
          method: 'POST',
          body: JSON.stringify({
            teamId: 'team_a',
            hostname: 'brand.example.com',
          }),
        }),
        { db },
        ctx,
      ),
    )

    expect(response.status).toBe(403)
  })

  test('POST claim as an owner creates a pending hostname (201)', async () => {
    const db = makeMemoryD1()
    const routes = makeRoutes({
      db,
      session: { user: { userId: 'user_1' } },
      role: 'owner',
    })
    const response = await run(
      routes.routeTenantHostnameSelfServeRequest(
        new Request('https://openagents.com/api/tenant/hostnames', {
          method: 'POST',
          body: JSON.stringify({
            teamId: 'team_a',
            hostname: 'brand.example.com',
          }),
        }),
        { db },
        ctx,
      ),
    )

    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      hostname: { status: string; servingLive: boolean }
    }
    expect(body.hostname.status).toBe('pending')
    expect(body.hostname.servingLive).toBe(false)
  })

  test('POST with an invalid hostname is 400', async () => {
    const db = makeMemoryD1()
    const routes = makeRoutes({
      db,
      session: { user: { userId: 'user_1' } },
      role: 'admin',
    })
    const response = await run(
      routes.routeTenantHostnameSelfServeRequest(
        new Request('https://openagents.com/api/tenant/hostnames', {
          method: 'POST',
          body: JSON.stringify({ teamId: 'team_a', hostname: 'not a host' }),
        }),
        { db },
        ctx,
      ),
    )

    expect(response.status).toBe(400)
  })

  test('a non-matching path does not apply', () => {
    const db = makeMemoryD1()
    const routes = makeRoutes({ db })
    const result = routes.routeTenantHostnameSelfServeRequest(
      new Request('https://openagents.com/api/other'),
      { db },
      ctx,
    )

    expect(result).toBeUndefined()
  })
})
