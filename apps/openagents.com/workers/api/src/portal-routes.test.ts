// PORTAL-1 (#8652): client portal route + store coverage.
//
// Behavior-contract oracles (packages/behavior-contracts/src/openagents-apps.ts):
//   * openagents_web.portal_owner_scoped_engagement.v1 — clients see only
//     their own engagement; cross-client reads and decisions fail closed.
//   * openagents_web.portal_decision_receipts.v1 — every approve/reject
//     decision produces an immutable decision receipt ref.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PORTAL_KPI_PLACEHOLDERS,
  makePortalRoutes,
  portalEngagementOwnedBy,
  type PortalSession,
} from './portal-routes'
import {
  PortalValidationError,
  makeD1PortalStore,
} from './portal-store'

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

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db
        .prepare(this.sql)
        .all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0315_portal_engagements_and_content_items.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

const makeRuntime = () => {
  let n = 0
  return {
    makeId: (prefix: string) => `${prefix}_${(n += 1)}`,
    nowIso: () => '2026-07-10T12:00:00.000Z',
  }
}

type TestEnv = Readonly<{ label: string }>

const testEnv: TestEnv = { label: 'portal-test' }

const makeRoutes = (
  db: D1Database,
  options: Readonly<{
    admin?: boolean
    session?: PortalSession | undefined
  }> = {},
) =>
  makePortalRoutes<TestEnv>({
    database: () => db,
    requireAdminApiToken: () => Promise.resolve(options.admin ?? false),
    requireBrowserSession: () => Promise.resolve(options.session),
  })

const ctx = {} as ExecutionContext

const run = async (
  routes: ReturnType<typeof makeRoutes>,
  request: Request,
): Promise<Response> => {
  const effect = routes.routePortalRequest(request, testEnv, ctx)
  if (effect === undefined) {
    throw new Error('route did not match')
  }
  return Effect.runPromise(effect)
}

const jsonRequest = (
  method: string,
  path: string,
  body?: unknown,
): Request =>
  new Request(`https://openagents.com${path}`, {
    method,
    ...(body === undefined
      ? {}
      : {
          body: JSON.stringify(body),
          headers: { 'content-type': 'application/json' },
        }),
  })

const seedEngagementWithItems = async (db: D1Database) => {
  const store = makeD1PortalStore(db)
  const runtime = makeRuntime()
  const engagement = await store.createEngagement(
    { name: 'Strategic Consulting Demo', clientEmail: 'client-a@example.com' },
    runtime,
  )
  const items = await store.seedContentItems(
    engagement.id,
    [
      {
        channel: 'linkedin',
        variant: 'a',
        pairRef: 'pair-1',
        title: 'Post A',
        body: 'Variant A body.',
      },
      {
        channel: 'linkedin',
        variant: 'b',
        pairRef: 'pair-1',
        title: 'Post B',
        body: 'Variant B body.',
      },
    ],
    runtime,
  )
  return { store, runtime, engagement, items }
}

describe('portal store (PORTAL-1 #8652)', () => {
  test('creates an engagement, binds a client, and seeds A/B content items', async () => {
    const db = makeDb()
    const { store, engagement, items } = await seedEngagementWithItems(db)

    expect(engagement.status).toBe('preparing')
    expect(engagement.clientEmail).toBe('client-a@example.com')
    expect(engagement.clientUserId).toBeNull()
    expect(items).toHaveLength(2)
    expect(items.map(item => item.variant).sort()).toEqual(['a', 'b'])
    expect(items.every(item => item.state === 'draft')).toBe(true)
    expect(items.every(item => item.pairRef === 'pair-1')).toBe(true)

    const bound = await store.bindClient({
      engagementId: engagement.id,
      clientUserId: 'user_a',
    })
    expect(bound?.clientUserId).toBe('user_a')
    expect(bound?.clientEmail).toBe('client-a@example.com')
  })

  test('openagents_web.portal_decision_receipts.v1: decisions mint immutable receipts', async () => {
    const db = makeDb()
    const { store, runtime, items } = await seedEngagementWithItems(db)
    const item = items[0]!

    const approved = await store.decideContentItem(item.id, 'approve', runtime)
    expect(approved.item.state).toBe('approved')
    expect(approved.item.decidedAt).toBe('2026-07-10T12:00:00.000Z')
    expect(approved.receiptRef).toMatch(/^portal_content_decision:pcd_/)
    expect(approved.alreadyDecided).toBe(false)

    // Idempotent repeat of the same decision returns the SAME receipt.
    const repeat = await store.decideContentItem(item.id, 'approve', runtime)
    expect(repeat.alreadyDecided).toBe(true)
    expect(repeat.receiptRef).toBe(approved.receiptRef)

    // Decisions never flip after the receipt is minted.
    await expect(
      store.decideContentItem(item.id, 'reject', runtime),
    ).rejects.toBeInstanceOf(PortalValidationError)

    const rejected = await store.decideContentItem(
      items[1]!.id,
      'reject',
      runtime,
    )
    expect(rejected.item.state).toBe('rejected')
    expect(rejected.receiptRef).toMatch(/^portal_content_decision:/)
    expect(rejected.receiptRef).not.toBe(approved.receiptRef)
  })

  test('readEngagementForClient never matches by email once a user id is bound elsewhere', async () => {
    const db = makeDb()
    const { store, engagement } = await seedEngagementWithItems(db)
    await store.bindClient({
      engagementId: engagement.id,
      clientUserId: 'user_a',
    })

    // Another account presenting the SAME email must not match: the bound
    // user id is authoritative (owner-scoped fail-closed).
    const byEmailOnly = await store.readEngagementForClient({
      userId: 'user_b',
      email: 'client-a@example.com',
    })
    expect(byEmailOnly).toBeNull()

    const byUser = await store.readEngagementForClient({
      userId: 'user_a',
      email: null,
    })
    expect(byUser?.id).toBe(engagement.id)
  })
})

describe('portal client routes (PORTAL-1 #8652)', () => {
  test('GET /api/portal/engagement requires a browser session', async () => {
    const db = makeDb()
    const routes = makeRoutes(db, { session: undefined })
    const response = await run(
      routes,
      jsonRequest('GET', '/api/portal/engagement'),
    )
    expect(response.status).toBe(401)
    expect(await response.json()).toEqual({ error: 'unauthorized' })
  })

  test('logged-in client without an engagement gets the honest empty shape', async () => {
    const db = makeDb()
    const routes = makeRoutes(db, {
      session: { user: { userId: 'user_new', email: 'new@example.com' } },
    })
    const response = await run(
      routes,
      jsonRequest('GET', '/api/portal/engagement'),
    )
    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({ engagement: null })
  })

  test('client reads their own engagement with items and placeholder KPIs, and the email binding pins the user id', async () => {
    const db = makeDb()
    const { store, engagement } = await seedEngagementWithItems(db)
    const routes = makeRoutes(db, {
      session: { user: { userId: 'user_a', email: 'Client-A@example.com' } },
    })

    const response = await run(
      routes,
      jsonRequest('GET', '/api/portal/engagement'),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      engagement: { id: string; name: string; status: string }
      items: Array<{ id: string; variant: string; state: string }>
      kpis: Array<{ key: string; value: null; note: string }>
    }
    expect(body.engagement.id).toBe(engagement.id)
    expect(body.engagement.name).toBe('Strategic Consulting Demo')
    expect(body.items).toHaveLength(2)
    // Honest placeholders: values are null, never fabricated numbers.
    expect(body.kpis).toEqual(PORTAL_KPI_PLACEHOLDERS)
    expect(body.kpis.every(kpi => kpi.value === null)).toBe(true)

    const pinned = await store.readEngagementById(engagement.id)
    expect(pinned?.clientUserId).toBe('user_a')
  })

  test('an EMAIL-provider client (email:<address> user id) reads their email-bound engagement and pins it (#8652 reopen)', async () => {
    // Regression: the SAFE_REF-only guard rejected `email:*` user ids ("@"
    // not in the pattern), so email-login clients — the audience the email
    // binding exists for — could never read even their own engagement.
    const db = makeDb()
    const { store, engagement } = await seedEngagementWithItems(db)
    const routes = makeRoutes(db, {
      session: {
        user: {
          userId: 'email:client-a@example.com',
          email: 'client-a@example.com',
        },
      },
    })

    const response = await run(
      routes,
      jsonRequest('GET', '/api/portal/engagement'),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { engagement: { id: string } }
    expect(body.engagement.id).toBe(engagement.id)

    // The first visit pins the email-provider user id like any other id.
    const pinned = await store.readEngagementById(engagement.id)
    expect(pinned?.clientUserId).toBe('email:client-a@example.com')

    // And the pinned id keeps resolving on subsequent visits (by-user path).
    const again = await store.readEngagementForClient({
      userId: 'email:client-a@example.com',
      email: null,
    })
    expect(again?.id).toBe(engagement.id)
  })

  test('openagents_web.portal_owner_scoped_engagement.v1: a client can NEVER read another engagement', async () => {
    const db = makeDb()
    const { store, engagement, items } = await seedEngagementWithItems(db)
    await store.bindClient({
      engagementId: engagement.id,
      clientUserId: 'user_a',
    })

    // Client B (different user id, different email) sees no engagement...
    const otherRoutes = makeRoutes(db, {
      session: { user: { userId: 'user_b', email: 'client-b@example.com' } },
    })
    const readResponse = await run(
      otherRoutes,
      jsonRequest('GET', '/api/portal/engagement'),
    )
    expect(readResponse.status).toBe(200)
    expect(await readResponse.json()).toEqual({ engagement: null })

    // ...and cannot decide client A's item; existence is not leaked (404).
    const decisionResponse = await run(
      otherRoutes,
      jsonRequest('POST', `/api/portal/content/${items[0]!.id}/decision`, {
        decision: 'approve',
      }),
    )
    expect(decisionResponse.status).toBe(404)
    expect(await decisionResponse.json()).toEqual({ error: 'not_found' })

    // The item stayed undecided.
    const untouched = await store.readContentItemById(items[0]!.id)
    expect(untouched?.state).toBe('draft')
    expect(untouched?.decisionReceiptRef).toBeNull()

    // Even a stolen engagement id gives client B nothing: there is no
    // client-facing engagement-id lookup route at all.
    expect(
      portalEngagementOwnedBy(
        (await store.readEngagementById(engagement.id))!,
        { userId: 'user_b', email: 'client-a@example.com' },
      ),
    ).toBe(false)
  })

  test('openagents_web.portal_decision_receipts.v1: approve and reject write decision receipts through the route', async () => {
    const db = makeDb()
    const { engagement, store, items } = await seedEngagementWithItems(db)
    await store.bindClient({
      engagementId: engagement.id,
      clientUserId: 'user_a',
    })
    const routes = makeRoutes(db, {
      session: { user: { userId: 'user_a', email: 'client-a@example.com' } },
    })

    const approve = await run(
      routes,
      jsonRequest('POST', `/api/portal/content/${items[0]!.id}/decision`, {
        decision: 'approve',
      }),
    )
    expect(approve.status).toBe(200)
    const approveBody = (await approve.json()) as {
      ok: boolean
      item: { state: string; decidedAt: string | null }
      receiptRef: string
      alreadyDecided: boolean
    }
    expect(approveBody.ok).toBe(true)
    expect(approveBody.item.state).toBe('approved')
    expect(approveBody.item.decidedAt).not.toBeNull()
    expect(approveBody.receiptRef).toMatch(/^portal_content_decision:/)
    expect(approveBody.alreadyDecided).toBe(false)

    const reject = await run(
      routes,
      jsonRequest('POST', `/api/portal/content/${items[1]!.id}/decision`, {
        decision: 'reject',
      }),
    )
    expect(reject.status).toBe(200)
    const rejectBody = (await reject.json()) as {
      item: { state: string }
      receiptRef: string
    }
    expect(rejectBody.item.state).toBe('rejected')
    expect(rejectBody.receiptRef).toMatch(/^portal_content_decision:/)

    // Flipping a decided item is refused with a typed 422.
    const flip = await run(
      routes,
      jsonRequest('POST', `/api/portal/content/${items[0]!.id}/decision`, {
        decision: 'reject',
      }),
    )
    expect(flip.status).toBe(422)
    expect(((await flip.json()) as { error: string }).error).toBe(
      'portal_validation_error',
    )
  })

  test('decision route validates input and method', async () => {
    const db = makeDb()
    const { items } = await seedEngagementWithItems(db)
    const routes = makeRoutes(db, {
      session: { user: { userId: 'user_a', email: 'client-a@example.com' } },
    })

    const badDecision = await run(
      routes,
      jsonRequest('POST', `/api/portal/content/${items[0]!.id}/decision`, {
        decision: 'publish',
      }),
    )
    expect(badDecision.status).toBe(400)

    const wrongMethod = await run(
      routes,
      jsonRequest('GET', `/api/portal/content/${items[0]!.id}/decision`),
    )
    expect(wrongMethod.status).toBe(405)

    const missingItem = await run(
      routes,
      jsonRequest('POST', '/api/portal/content/nope_123/decision', {
        decision: 'approve',
      }),
    )
    expect(missingItem.status).toBe(404)
  })
})

describe('portal admin routes (PORTAL-1 #8652)', () => {
  test('admin routes are fail-closed without the admin bearer token', async () => {
    const db = makeDb()
    const routes = makeRoutes(db, { admin: false })

    for (const request of [
      jsonRequest('POST', '/api/portal/admin/engagements', { name: 'X' }),
      jsonRequest('POST', '/api/portal/admin/engagements/e1/bind', {
        clientEmail: 'x@example.com',
      }),
      jsonRequest('POST', '/api/portal/admin/engagements/e1/content-items', {
        items: [],
      }),
      jsonRequest('GET', '/api/portal/admin/engagements/e1'),
    ]) {
      const response = await run(routes, request)
      expect(response.status).toBe(401)
      expect(await response.json()).toEqual({ error: 'unauthorized' })
    }
  })

  test('admin creates an engagement, binds a client identity, and seeds content', async () => {
    const db = makeDb()
    const routes = makeRoutes(db, { admin: true })

    const created = await run(
      routes,
      jsonRequest('POST', '/api/portal/admin/engagements', {
        name: 'Business Formation Demo',
        clientEmail: 'Founder@Example.com',
      }),
    )
    expect(created.status).toBe(201)
    const createdBody = (await created.json()) as {
      engagement: { id: string; status: string; clientEmail: string }
    }
    expect(createdBody.engagement.status).toBe('preparing')
    expect(createdBody.engagement.clientEmail).toBe('founder@example.com')
    const engagementId = createdBody.engagement.id

    const bound = await run(
      routes,
      jsonRequest('POST', `/api/portal/admin/engagements/${engagementId}/bind`, {
        clientUserId: 'user_founder',
      }),
    )
    expect(bound.status).toBe(200)
    expect(
      ((await bound.json()) as { engagement: { clientUserId: string } })
        .engagement.clientUserId,
    ).toBe('user_founder')

    const seeded = await run(
      routes,
      jsonRequest(
        'POST',
        `/api/portal/admin/engagements/${engagementId}/content-items`,
        {
          items: [
            {
              channel: 'linkedin',
              variant: 'a',
              pairRef: 'week1-post1',
              title: 'Entity choice',
              body: 'LLC or C-corp?',
            },
            {
              channel: 'linkedin',
              variant: 'b',
              pairRef: 'week1-post1',
              title: 'Entity choice (alt)',
              body: 'C-corp or LLC?',
            },
          ],
        },
      ),
    )
    expect(seeded.status).toBe(201)
    expect(
      ((await seeded.json()) as { items: Array<unknown> }).items,
    ).toHaveLength(2)

    const read = await run(
      routes,
      jsonRequest('GET', `/api/portal/admin/engagements/${engagementId}`),
    )
    expect(read.status).toBe(200)
    const readBody = (await read.json()) as { items: Array<unknown> }
    expect(readBody.items).toHaveLength(2)

    const missing = await run(
      routes,
      jsonRequest('GET', '/api/portal/admin/engagements/does_not_exist'),
    )
    expect(missing.status).toBe(404)
  })

  test('admin seed validates items and engagement existence', async () => {
    const db = makeDb()
    const routes = makeRoutes(db, { admin: true })

    const emptyItems = await run(
      routes,
      jsonRequest('POST', '/api/portal/admin/engagements/e1/content-items', {
        items: [],
      }),
    )
    expect(emptyItems.status).toBe(422)

    const badItem = await run(
      routes,
      jsonRequest('POST', '/api/portal/admin/engagements/e1/content-items', {
        items: [{ channel: 'linkedin' }],
      }),
    )
    expect(badItem.status).toBe(400)
  })
})
