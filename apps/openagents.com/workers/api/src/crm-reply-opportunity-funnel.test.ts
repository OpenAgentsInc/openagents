// OB-5 (#8562): proves the reply route's best-effort opportunity-stage
// advance against a REAL schema (not the lightweight query-shape fake in
// crm-reply-routes.test.ts), using the same node:sqlite-backed D1 pattern as
// the OB-3 report store and OB-5's other new stores.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmReplyRoutes } from './crm-reply-routes'

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
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
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

const TENANT_REF = 'tenant.openagents'

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0218_crm_contacts.sql'))
  db.exec(migration('0310_crm_command_batches_and_replies.sql'))
  db.exec(migration('0311_crm_sarah_handoff_links.sql'))
  db.exec(
    `INSERT INTO crm_contacts (id, tenant_ref, primary_email, created_at, updated_at)
     VALUES ('crm_contact_1', '${TENANT_REF}', 'ada@example.com', '2026-07-08T00:00:00.000Z', '2026-07-08T00:00:00.000Z')`,
  )
  return new SqliteD1(db) as unknown as D1Database
}

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext
const base = 'https://openagents.com'

const routesFor = (db: D1Database) => {
  const routes = makeCrmReplyRoutes<Env>({ requireAdminApiToken: () => Promise.resolve(true) })
  return (request: Request): Promise<Response> => {
    const effect = routes.routeCrmReplyRequest(request, { OPENAGENTS_DB: db }, ctx)
    if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
    return Effect.runPromise(effect)
  }
}

const post = (path: string, body: unknown): Request =>
  new Request(`${base}${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('CRM reply route opportunity-funnel advance (OB-5, #8562)', () => {
  test('a matched reply opens a sales opportunity at the replied stage and returns it', async () => {
    const db = makeDb()
    const run = routesFor(db)

    const res = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Sounds great, tell me more.',
        fromEmail: 'ada@example.com',
        subject: 'Re: your report',
        sourceRef: 'apollo_agent_readiness_ecommerce',
      }),
    )
    expect(res.status).toBe(201)
    const body = (await res.json()) as {
      opportunity?: { id: string; stage: string }
      sarahHandoff?: { url: string }
    }
    expect(body.opportunity?.stage).toBe('replied')
    expect(body.sarahHandoff?.url).toContain('openagents.com/sarah/continue/')

    const stored = await db
      .prepare('SELECT * FROM crm_opportunities WHERE id = ?')
      .bind(body.opportunity?.id)
      .first<Row>()
    expect(stored?.stage).toBe('replied')
    expect(stored?.status).toBe('open')
    expect(JSON.parse(String(stored?.metadata_json ?? '{}'))).toMatchObject({
      sourceRef: 'apollo_agent_readiness_ecommerce',
    })

    const role = await db
      .prepare(
        'SELECT * FROM crm_opportunity_contact_roles WHERE opportunity_id = ? AND contact_id = ?',
      )
      .bind(body.opportunity?.id, 'crm_contact_1')
      .first<Row>()
    expect(role).not.toBeNull()
  })

  test('a second reply reuses the SAME open opportunity instead of minting a new one', async () => {
    const db = makeDb()
    const run = routesFor(db)

    const first = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Interested, tell me more.',
        fromEmail: 'ada@example.com',
        providerEventId: 'evt_1',
      }),
    )
    const firstBody = (await first.json()) as { opportunity?: { id: string } }

    const second = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Following up on my last note.',
        fromEmail: 'ada@example.com',
        providerEventId: 'evt_2',
      }),
    )
    const secondBody = (await second.json()) as { opportunity?: { id: string; stage: string } }

    expect(secondBody.opportunity?.id).toBe(firstBody.opportunity?.id)
    expect(secondBody.opportunity?.stage).toBe('replied')

    const count = await db
      .prepare('SELECT COUNT(*) as n FROM crm_opportunities')
      .first<{ n: number }>()
    expect(count?.n).toBe(1)
  })

  test('never regresses a deal that already advanced past replied', async () => {
    const db = makeDb()
    const run = routesFor(db)

    const first = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Yes, send me a quote.',
        fromEmail: 'ada@example.com',
      }),
    )
    const firstBody = (await first.json()) as { opportunity?: { id: string } }
    const opportunityId = firstBody.opportunity?.id as string

    // Simulate the deal having already progressed to `quoted` (e.g. via the
    // checkout-link route) before a late/duplicate reply webhook arrives.
    await db
      .prepare("UPDATE crm_opportunities SET stage = 'quoted' WHERE id = ?")
      .bind(opportunityId)
      .run()

    const second = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Just checking in again.',
        fromEmail: 'ada@example.com',
      }),
    )
    const secondBody = (await second.json()) as { opportunity?: { id: string; stage: string } }
    expect(secondBody.opportunity?.id).toBe(opportunityId)
    expect(secondBody.opportunity?.stage).toBe('quoted')
  })

  test('opt-out replies never open or advance an opportunity', async () => {
    const db = makeDb()
    const run = routesFor(db)

    const res = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Please unsubscribe me.',
        fromEmail: 'ada@example.com',
      }),
    )
    const body = (await res.json()) as { opportunity?: unknown }
    expect(body.opportunity).toBeUndefined()

    const count = await db
      .prepare('SELECT COUNT(*) as n FROM crm_opportunities')
      .first<{ n: number }>()
    expect(count?.n).toBe(0)
  })
})
