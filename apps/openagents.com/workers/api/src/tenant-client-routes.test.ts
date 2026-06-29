import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeTenantClientRoutes } from './tenant-client-routes'
import { type TenantRef } from './tenant-custom-hostnames'

// Reuse the same fake-D1 shape used by the core test, scoped to what the route
// needs the core to read.
type MembershipRow = Readonly<{
  team_id: string
  user_id: string
  role: 'owner' | 'admin' | 'member' | 'viewer'
}>

type WorkroomFixture = Readonly<{
  id: string
  site_team_id: string | null
  visibility: 'private' | 'customer' | 'team' | 'public'
}>

const baseWorkroomRow = (fixture: WorkroomFixture) => ({
  accepted_outcome_contract_id: null,
  archived_at: null,
  artifact_refs_json: JSON.stringify([]),
  assignment_id: null,
  blocker_refs_json: JSON.stringify([]),
  classification_caveat_ref: 'classification_caveat_reviewed',
  created_at: '2026-06-14T00:00:00.000Z',
  customer_intent_ref: 'intent.customer.summary',
  data_classification: 'customer',
  email_refs_json: JSON.stringify([]),
  id: fixture.id,
  idempotency_key: `idem_${fixture.id}`,
  metadata_json: JSON.stringify({}),
  public_receipt_ref: `omni_workroom:order:${fixture.id}`,
  receipt_refs_json: JSON.stringify([]),
  site_id: fixture.site_team_id === null ? null : `site_${fixture.id}`,
  site_team_id: fixture.site_team_id,
  software_order_id: `software_order_${fixture.id}`,
  source_refs_json: JSON.stringify([]),
  status: 'active',
  task_packet_ref: null,
  trust_tier: 'verified',
  updated_at: '2026-06-14T01:00:00.000Z',
  visibility: fixture.visibility,
  work_kind: 'coding',
})

class Store {
  memberships: Array<MembershipRow> = []
  workrooms: Array<WorkroomFixture> = []
}

class Statement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: Store,
  ) {}

  bind(...values: ReadonlyArray<unknown>) {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM team_memberships')) {
      const row = this.store.memberships.find(
        m =>
          m.team_id === String(this.values[0]) &&
          m.user_id === String(this.values[1]),
      )

      return Promise.resolve((row ? { role: row.role } : null) as T | null)
    }

    if (this.query.includes('FROM omni_workrooms')) {
      const fixture = this.store.workrooms.find(
        w => w.id === String(this.values[0]),
      )

      return Promise.resolve(
        (fixture ? baseWorkroomRow(fixture) : null) as T | null,
      )
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }
}

const makeDb = (store: Store): D1Database =>
  ({
    prepare: (query: string) => new Statement(query, store),
  }) as unknown as D1Database

type Env = Readonly<{ db: D1Database }>

const ctx = {} as ExecutionContext

const tenant = (teamId: string): TenantRef => ({
  teamId,
  hostname: `${teamId}.clients.example.com`,
  status: 'active',
})

const makeRoutes = (
  store: Store,
  overrides: Partial<{
    session: Readonly<{ user: Readonly<{ userId: string }> }> | undefined
    tenant: TenantRef | undefined
  }> = {},
) =>
  makeTenantClientRoutes<
    Readonly<{ user: Readonly<{ userId: string }> }>,
    Env
  >({
    database: env => env.db,
    requireBrowserSession: () =>
      Promise.resolve(
        'session' in overrides
          ? overrides.session
          : { user: { userId: 'client_1' } },
      ),
    resolveTenant: () =>
      Promise.resolve(
        'tenant' in overrides ? overrides.tenant : tenant('team_a'),
      ),
  })

const get = (id: string): Request =>
  new Request(`https://team_a.clients.example.com/api/tenant/client/workrooms/${id}`)

describe('makeTenantClientRoutes', () => {
  test('non-matching path returns undefined', () => {
    const routes = makeRoutes(new Store())
    const out = routes.routeTenantClientRequest(
      new Request('https://x.example.com/api/other'),
      { db: makeDb(new Store()) },
      ctx,
    )

    expect(out).toBeUndefined()
  })

  test('authorized client gets 200 with customer projection', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'viewer',
    })
    store.workrooms.push({
      id: 'wr_1',
      site_team_id: 'team_a',
      visibility: 'customer',
    })

    const routes = makeRoutes(store)
    const effect = routes.routeTenantClientRequest(
      get('wr_1'),
      { db: makeDb(store) },
      ctx,
    )
    const response = await Effect.runPromise(effect!)

    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.surface).toBe('customer')
    expect(body.workroomId).toBe('wr_1')
    const workroom = body.workroom as Record<string, unknown>
    // customer surface — must not carry team/operator-only fields
    expect('acceptedOutcomeContractId' in workroom).toBe(false)
    expect('sourceRefs' in workroom).toBe(false)
  })

  test('unauthenticated request is 401', async () => {
    const store = new Store()
    const routes = makeRoutes(store, { session: undefined })
    const response = await Effect.runPromise(
      routes.routeTenantClientRequest(get('wr_1'), { db: makeDb(store) }, ctx)!,
    )

    expect(response.status).toBe(401)
  })

  test('no resolved tenant is 404 (route only applies on branded hosts)', async () => {
    const store = new Store()
    const routes = makeRoutes(store, { tenant: undefined })
    const response = await Effect.runPromise(
      routes.routeTenantClientRequest(get('wr_1'), { db: makeDb(store) }, ctx)!,
    )

    expect(response.status).toBe(404)
  })

  test('non-member client is 403', async () => {
    const store = new Store()
    store.workrooms.push({
      id: 'wr_1',
      site_team_id: 'team_a',
      visibility: 'customer',
    })

    const routes = makeRoutes(store, { session: { user: { userId: 'stranger' } } })
    const response = await Effect.runPromise(
      routes.routeTenantClientRequest(get('wr_1'), { db: makeDb(store) }, ctx)!,
    )

    expect(response.status).toBe(403)
  })

  test('cross-tenant workroom is 404', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'admin',
    })
    store.workrooms.push({
      id: 'wr_other',
      site_team_id: 'team_b',
      visibility: 'customer',
    })

    const routes = makeRoutes(store)
    const response = await Effect.runPromise(
      routes.routeTenantClientRequest(
        get('wr_other'),
        { db: makeDb(store) },
        ctx,
      )!,
    )

    expect(response.status).toBe(404)
  })

  test('private-visibility workroom is 404 (never leaked to client)', async () => {
    const store = new Store()
    store.memberships.push({
      team_id: 'team_a',
      user_id: 'client_1',
      role: 'admin',
    })
    store.workrooms.push({
      id: 'wr_private',
      site_team_id: 'team_a',
      visibility: 'private',
    })

    const routes = makeRoutes(store)
    const response = await Effect.runPromise(
      routes.routeTenantClientRequest(
        get('wr_private'),
        { db: makeDb(store) },
        ctx,
      )!,
    )

    expect(response.status).toBe(404)
  })

  test('POST is 405', async () => {
    const store = new Store()
    const routes = makeRoutes(store)
    const response = await Effect.runPromise(
      routes.routeTenantClientRequest(
        new Request(
          'https://team_a.clients.example.com/api/tenant/client/workrooms/wr_1',
          { method: 'POST' },
        ),
        { db: makeDb(store) },
        ctx,
      )!,
    )

    expect(response.status).toBe(405)
  })
})
